use crate::agent::AgentMode;
use crate::storage::Usage;
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::mpsc;

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
// computer-use-2025-01-24: enables computer_20250124 and bash_20250124 tools
// interleaved-thinking-2025-05-14: enables extended thinking with tool use for Claude 4 models
const BETA_HEADER: &str = "computer-use-2025-01-24,interleaved-thinking-2025-05-14";
const API_VERSION: &str = "2023-06-01";

/// display dimensions sent to claude for coordinate mapping.
/// matches the resolution we resize screenshots to in computer.rs
const DISPLAY_WIDTH: u32 = 1280;
const DISPLAY_HEIGHT: u32 = 800;

/// max output tokens for claude response. 16k allows for detailed responses
/// while staying within typical rate limits
const MAX_TOKENS: u32 = 16000;

/// thinking budget for extended thinking
const THINKING_BUDGET: u32 = 5000;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("API error: {0}")]
    Api(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { source: ImageSource },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: Vec<ToolResultContent>,
    },
    #[serde(rename = "thinking")]
    Thinking {
        thinking: String,
        signature: String,
    },
    #[serde(rename = "redacted_thinking")]
    RedactedThinking {
        data: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToolResultContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { source: ImageSource },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSource {
    #[serde(rename = "type")]
    pub source_type: String,
    pub media_type: String,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: Vec<ContentBlock>,
}

#[derive(Debug, Serialize)]
struct ThinkingConfig {
    #[serde(rename = "type")]
    config_type: String,
    budget_tokens: u32,
}

#[derive(Debug, Serialize)]
struct ApiRequest {
    model: String,
    max_tokens: u32,
    system: String,
    tools: Vec<serde_json::Value>,
    messages: Vec<Message>,
    stream: bool,
    thinking: ThinkingConfig,
}

#[derive(Debug, Deserialize)]
struct ApiErrorResponse {
    error: ApiErrorDetail,
}

#[derive(Debug, Deserialize)]
struct ApiErrorDetail {
    message: String,
}

// streaming event types
#[derive(Debug, Clone)]
pub enum StreamEvent {
    TextDelta { text: String },
    ThinkingDelta { thinking: String },
    ToolUseStart { name: String },
    MessageStop,
}

// api call result with content and usage
#[derive(Debug)]
pub struct ApiResult {
    pub content: Vec<ContentBlock>,
    pub usage: Usage,
}

pub struct AnthropicClient {
    client: Client,
    api_key: String,
    model: String,
}

impl AnthropicClient {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model,
        }
    }

    fn build_tools(&self, mode: AgentMode) -> Vec<serde_json::Value> {
        let mut tools = Vec::new();

        match mode {
            AgentMode::Computer => {
                // computer tool for screen control
                tools.push(serde_json::json!({
                    "type": "computer_20250124",
                    "name": "computer",
                    "display_width_px": DISPLAY_WIDTH,
                    "display_height_px": DISPLAY_HEIGHT,
                    "display_number": 1
                }));
            }
            AgentMode::Browser => {
                // browser tools via chromiumoxide CDP
                tools.extend(build_browser_tools());
            }
        }

        // bash available in both modes
        tools.push(serde_json::json!({
            "type": "bash_20250124",
            "name": "bash"
        }));

        tools
    }

    pub async fn send_message_streaming(
        &self,
        messages: Vec<Message>,
        event_tx: mpsc::UnboundedSender<StreamEvent>,
        mode: AgentMode,
    ) -> Result<ApiResult, ApiError> {
        let system = match mode {
            AgentMode::Computer => SYSTEM_PROMPT.to_string(),
            AgentMode::Browser => BROWSER_SYSTEM_PROMPT.to_string(),
        };

        let tools = self.build_tools(mode);
        println!("[api] Sending {} tools: {:?}", tools.len(), tools.iter().map(|t| t.get("name")).collect::<Vec<_>>());
        println!("[api] Tools JSON: {}", serde_json::to_string_pretty(&tools).unwrap_or_default());

        let request = ApiRequest {
            model: self.model.clone(),
            max_tokens: MAX_TOKENS,
            system,
            tools,
            messages,
            stream: true,
            thinking: ThinkingConfig {
                config_type: "enabled".to_string(),
                budget_tokens: THINKING_BUDGET,
            },
        };

        let response = self
            .client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", API_VERSION)
            .header("anthropic-beta", BETA_HEADER)
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await?;
            if let Ok(err) = serde_json::from_str::<ApiErrorResponse>(&body) {
                return Err(ApiError::Api(err.error.message));
            }
            return Err(ApiError::Api(format!("HTTP {}: {}", status, body)));
        }

        // parse SSE stream incrementally
        let mut content_blocks: Vec<ContentBlock> = Vec::new();
        let mut current_text: Vec<String> = Vec::new();
        let mut current_thinking: Vec<String> = Vec::new();
        let mut thinking_signature: Vec<String> = Vec::new();
        let mut current_tool_json: Vec<String> = Vec::new();
        let mut tool_info: Vec<(String, String)> = Vec::new(); // (id, name)
        let mut block_types: Vec<String> = Vec::new(); // track block type per index
        let mut buffer = String::new();

        // track usage from SSE events
        let mut usage = Usage::default();

        let mut stream = response.bytes_stream();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            // process complete lines
            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer[..newline_pos].trim().to_string();
                buffer = buffer[newline_pos + 1..].to_string();

                if !line.starts_with("data: ") {
                    continue;
                }

                let data = &line[6..];
                if let Ok(event) = serde_json::from_str::<serde_json::Value>(data) {
                    let event_type = event.get("type").and_then(|t| t.as_str()).unwrap_or("");

                    match event_type {
                        "message_start" => {
                            // capture input token usage from message_start
                            if let Some(message) = event.get("message") {
                                if let Some(u) = message.get("usage") {
                                    usage.input_tokens = u.get("input_tokens")
                                        .and_then(|v| v.as_u64())
                                        .unwrap_or(0) as u32;
                                    usage.cache_creation_input_tokens = u.get("cache_creation_input_tokens")
                                        .and_then(|v| v.as_u64())
                                        .unwrap_or(0) as u32;
                                    usage.cache_read_input_tokens = u.get("cache_read_input_tokens")
                                        .and_then(|v| v.as_u64())
                                        .unwrap_or(0) as u32;
                                }
                            }
                        }

                        "message_delta" => {
                            // capture output token usage from message_delta (cumulative)
                            if let Some(u) = event.get("usage") {
                                usage.output_tokens = u.get("output_tokens")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0) as u32;
                            }
                        }

                        "content_block_start" => {
                            let index = event.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
                            if let Some(block) = event.get("content_block") {
                                let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");

                                // ensure vectors are big enough
                                while current_text.len() <= index {
                                    current_text.push(String::new());
                                }
                                while current_thinking.len() <= index {
                                    current_thinking.push(String::new());
                                }
                                while thinking_signature.len() <= index {
                                    thinking_signature.push(String::new());
                                }
                                while current_tool_json.len() <= index {
                                    current_tool_json.push(String::new());
                                }
                                while tool_info.len() <= index {
                                    tool_info.push((String::new(), String::new()));
                                }
                                while block_types.len() <= index {
                                    block_types.push(String::new());
                                }
                                block_types[index] = block_type.to_string();

                                if block_type == "tool_use" {
                                    let id = block.get("id").and_then(|i| i.as_str()).unwrap_or("").to_string();
                                    let name = block.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
                                    tool_info[index] = (id.clone(), name.clone());
                                    let _ = event_tx.send(StreamEvent::ToolUseStart { name });
                                }
                            }
                        }

                        "content_block_delta" => {
                            let index = event.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
                            if let Some(delta) = event.get("delta") {
                                let delta_type = delta.get("type").and_then(|t| t.as_str()).unwrap_or("");

                                match delta_type {
                                    "thinking_delta" => {
                                        if let Some(thinking) = delta.get("thinking").and_then(|t| t.as_str()) {
                                            while current_thinking.len() <= index {
                                                current_thinking.push(String::new());
                                            }
                                            current_thinking[index].push_str(thinking);
                                            let _ = event_tx.send(StreamEvent::ThinkingDelta {
                                                thinking: thinking.to_string(),
                                            });
                                        }
                                    }
                                    "signature_delta" => {
                                        if let Some(sig) = delta.get("signature").and_then(|s| s.as_str()) {
                                            while thinking_signature.len() <= index {
                                                thinking_signature.push(String::new());
                                            }
                                            thinking_signature[index].push_str(sig);
                                        }
                                    }
                                    "text_delta" => {
                                        if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                                            while current_text.len() <= index {
                                                current_text.push(String::new());
                                            }
                                            current_text[index].push_str(text);
                                            let _ = event_tx.send(StreamEvent::TextDelta {
                                                text: text.to_string(),
                                            });
                                        }
                                    }
                                    "input_json_delta" => {
                                        if let Some(json) = delta.get("partial_json").and_then(|j| j.as_str()) {
                                            while current_tool_json.len() <= index {
                                                current_tool_json.push(String::new());
                                            }
                                            current_tool_json[index].push_str(json);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }

                        "content_block_stop" => {
                            let index = event.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;

                            // finalize content block based on tracked type
                            let block_type = if index < block_types.len() {
                                block_types[index].as_str()
                            } else {
                                ""
                            };

                            match block_type {
                                "thinking" => {
                                    if index < current_thinking.len() && !current_thinking[index].is_empty() {
                                        let sig = if index < thinking_signature.len() {
                                            thinking_signature[index].clone()
                                        } else {
                                            String::new()
                                        };
                                        content_blocks.push(ContentBlock::Thinking {
                                            thinking: current_thinking[index].clone(),
                                            signature: sig,
                                        });
                                    }
                                }
                                "text" => {
                                    if index < current_text.len() && !current_text[index].is_empty() {
                                        content_blocks.push(ContentBlock::Text {
                                            text: current_text[index].clone(),
                                        });
                                    }
                                }
                                "tool_use" => {
                                    // tool_use may have empty input (e.g. take_snapshot with no args)
                                    // so we check tool_info instead of current_tool_json
                                    let (id, name) = if index < tool_info.len() {
                                        tool_info[index].clone()
                                    } else {
                                        (String::new(), String::new())
                                    };
                                    if !id.is_empty() {
                                        let json_str = if index < current_tool_json.len() {
                                            &current_tool_json[index]
                                        } else {
                                            ""
                                        };
                                        let input: serde_json::Value = if json_str.is_empty() {
                                            serde_json::json!({})
                                        } else {
                                            serde_json::from_str(json_str).unwrap_or(serde_json::json!({}))
                                        };
                                        content_blocks.push(ContentBlock::ToolUse { id, name, input });
                                    }
                                }
                                _ => {}
                            }
                        }

                        "message_stop" => {
                            let _ = event_tx.send(StreamEvent::MessageStop);
                        }

                        _ => {}
                    }
                }
            }
        }

        Ok(ApiResult {
            content: content_blocks,
            usage,
        })
    }
}

/// rewrite raw speech transcription into clean text using haiku
pub async fn rewrite_transcription(api_key: &str, raw_text: &str) -> Result<String, ApiError> {
    if raw_text.trim().is_empty() {
        return Ok(String::new());
    }

    let client = Client::new();

    let prompt = format!(
        r#"<context>
This is raw speech-to-text output from voice dictation. It may contain filler words, false starts, repeated phrases, incomplete thoughts, or trailing fragments from when the user released the push-to-talk key.
</context>

<instructions>
Rewrite this into clean, natural text that preserves the speaker's intent. Remove filler words (um, uh, like, you know), fix incomplete sentences, merge repeated phrases, and clean up any trailing fragments.

Output only the rewritten text. No explanations, no quotes, no prefixes.
</instructions>

<input>
{}
</input>"#,
        raw_text
    );

    let request_body = serde_json::json!({
        "model": "claude-haiku-4-5-20251001",
        "max_tokens": 1024,
        "messages": [{
            "role": "user",
            "content": prompt
        }]
    });

    let response = client
        .post(ANTHROPIC_API_URL)
        .header("x-api-key", api_key)
        .header("anthropic-version", API_VERSION)
        .header("content-type", "application/json")
        .json(&request_body)
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await?;
        if let Ok(err) = serde_json::from_str::<ApiErrorResponse>(&body) {
            return Err(ApiError::Api(err.error.message));
        }
        return Err(ApiError::Api(format!("HTTP {}: {}", status, body)));
    }

    let body: serde_json::Value = response.json().await?;

    // extract text from first content block
    if let Some(content) = body.get("content").and_then(|c| c.as_array()) {
        for block in content {
            if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                    return Ok(text.trim().to_string());
                }
            }
        }
    }

    // fallback to raw text if parsing fails
    Ok(raw_text.to_string())
}

const SYSTEM_PROMPT: &str = r#"You are taskhomie, a macOS computer control agent. You see the screen, control mouse/keyboard, and run bash.

Keep text responses very concise. Focus on doing, not explaining. Use tools on every turn.

Click to focus before typing. Screenshot after actions to verify. If something fails, try another approach.

Prefer bash for speed: open -a "App", open https://url, pbcopy/pbpaste, mdfind. Use `sleep N` when waiting.

Use computer tool for visual tasks: clicking UI, reading screen content, filling forms."#;

const BROWSER_SYSTEM_PROMPT: &str = r#"You are taskhomie in browser mode. You control Chrome via CDP.

Keep text responses very concise. Focus on doing, not explaining. Use tools on every turn.

Start every task with take_snapshot to see the page. Use uids from the latest snapshot only—stale uids fail. Take a new snapshot after any action that changes the page.

Use screenshot when:
- You're stuck or something isn't working as expected
- You need to verify a visual result after an action
- Dealing with CAPTCHAs, images, or visual elements not in the a11y tree
- Confirming the page looks correct before reporting success

Use bash for file operations.

If browser tools fail with connection errors, Chrome may have been closed. Run this bash command to relaunch it with debugging enabled:
open -a "Google Chrome" --args --remote-debugging-port=9222 --user-data-dir="$HOME/.taskhomie-chrome" --profile-directory=Default --no-first-run
Then wait a few seconds and retry the browser tool."#;

fn build_browser_tools() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "name": "take_snapshot",
            "description": "Get page accessibility tree with element uids. Call first before interacting.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "verbose": { "type": "boolean", "description": "Include ignored nodes" }
                },
                "required": []
            }
        }),
        serde_json::json!({
            "name": "click",
            "description": "Click element by uid",
            "input_schema": {
                "type": "object",
                "properties": {
                    "uid": { "type": "string", "description": "Element uid from snapshot" },
                    "dblClick": { "type": "boolean", "description": "Double click" }
                },
                "required": ["uid"]
            }
        }),
        serde_json::json!({
            "name": "hover",
            "description": "Hover over element by uid",
            "input_schema": {
                "type": "object",
                "properties": {
                    "uid": { "type": "string", "description": "Element uid from snapshot" }
                },
                "required": ["uid"]
            }
        }),
        serde_json::json!({
            "name": "fill",
            "description": "Type text into input element",
            "input_schema": {
                "type": "object",
                "properties": {
                    "uid": { "type": "string", "description": "Input element uid" },
                    "value": { "type": "string", "description": "Text to type" }
                },
                "required": ["uid", "value"]
            }
        }),
        serde_json::json!({
            "name": "fill_form",
            "description": "Fill multiple form inputs at once",
            "input_schema": {
                "type": "object",
                "properties": {
                    "elements": {
                        "type": "array",
                        "description": "Array of {uid, value} pairs",
                        "items": {
                            "type": "object",
                            "properties": { "uid": { "type": "string" }, "value": { "type": "string" } },
                            "required": ["uid", "value"]
                        }
                    }
                },
                "required": ["elements"]
            }
        }),
        serde_json::json!({
            "name": "drag",
            "description": "Drag element to another element",
            "input_schema": {
                "type": "object",
                "properties": {
                    "from_uid": { "type": "string", "description": "Source element uid" },
                    "to_uid": { "type": "string", "description": "Target element uid" }
                },
                "required": ["from_uid", "to_uid"]
            }
        }),
        serde_json::json!({
            "name": "press_key",
            "description": "Press key or combo (Enter, Control+A, Shift+Tab)",
            "input_schema": {
                "type": "object",
                "properties": {
                    "key": { "type": "string", "description": "Key or combo" }
                },
                "required": ["key"]
            }
        }),
        serde_json::json!({
            "name": "navigate_page",
            "description": "Navigate: url, back, forward, or reload",
            "input_schema": {
                "type": "object",
                "properties": {
                    "type": { "type": "string", "enum": ["url", "back", "forward", "reload"] },
                    "url": { "type": "string", "description": "URL (for type=url)" },
                    "ignoreCache": { "type": "boolean", "description": "Bypass cache on reload" }
                },
                "required": ["type"]
            }
        }),
        serde_json::json!({
            "name": "wait_for",
            "description": "Wait for text to appear on page",
            "input_schema": {
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "Text to wait for" },
                    "timeout": { "type": "number", "description": "Timeout ms (default 5000)" }
                },
                "required": ["text"]
            }
        }),
        serde_json::json!({
            "name": "new_page",
            "description": "Open new tab with URL",
            "input_schema": {
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "URL to open" }
                },
                "required": ["url"]
            }
        }),
        serde_json::json!({
            "name": "list_pages",
            "description": "List open tabs",
            "input_schema": { "type": "object", "properties": {}, "required": [] }
        }),
        serde_json::json!({
            "name": "select_page",
            "description": "Switch to tab by index",
            "input_schema": {
                "type": "object",
                "properties": {
                    "pageIdx": { "type": "number", "description": "Tab index" },
                    "bringToFront": { "type": "boolean", "description": "Focus the tab" }
                },
                "required": ["pageIdx"]
            }
        }),
        serde_json::json!({
            "name": "close_page",
            "description": "Close tab by index (cannot close last tab)",
            "input_schema": {
                "type": "object",
                "properties": {
                    "pageIdx": { "type": "number", "description": "Tab index to close" }
                },
                "required": ["pageIdx"]
            }
        }),
        serde_json::json!({
            "name": "handle_dialog",
            "description": "Handle browser dialog (alert, confirm, prompt)",
            "input_schema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["accept", "dismiss"] },
                    "promptText": { "type": "string", "description": "Text for prompt dialogs" }
                },
                "required": ["action"]
            }
        }),
        serde_json::json!({
            "name": "screenshot",
            "description": "Capture a screenshot of the current page. Use when stuck, for visual verification, or to see CAPTCHAs and other visual elements not in the a11y tree.",
            "input_schema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        }),
    ]
}
