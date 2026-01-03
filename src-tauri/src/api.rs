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
// web-fetch-2025-09-10: enables web_fetch_20250910 server tool
// context-management-2025-06-27: enables context editing (clear_tool_uses, clear_thinking)
const BETA_HEADER: &str = "computer-use-2025-01-24,interleaved-thinking-2025-05-14,web-fetch-2025-09-10,context-management-2025-06-27";
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
    // server-side tool use (web_search, web_fetch) - anthropic executes these
    #[serde(rename = "server_tool_use")]
    ServerToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    // web search results from server
    #[serde(rename = "web_search_tool_result")]
    WebSearchToolResult {
        tool_use_id: String,
        content: serde_json::Value,
    },
    // web fetch results from server
    #[serde(rename = "web_fetch_tool_result")]
    WebFetchToolResult {
        tool_use_id: String,
        content: serde_json::Value,
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
struct ContextManagement {
    edits: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct SystemBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_control: Option<CacheControl>,
}

#[derive(Debug, Serialize)]
struct CacheControl {
    #[serde(rename = "type")]
    cache_type: String,
}

#[derive(Debug, Serialize)]
struct ApiRequest {
    model: String,
    max_tokens: u32,
    system: Vec<SystemBlock>,
    tools: Vec<serde_json::Value>,
    messages: Vec<Message>,
    stream: bool,
    thinking: ThinkingConfig,
    context_management: ContextManagement,
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

    fn build_tools(&self, mode: AgentMode, voice_mode: bool) -> Vec<serde_json::Value> {
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

        // web search tool - server-side, anthropic executes
        tools.push(serde_json::json!({
            "type": "web_search_20250305",
            "name": "web_search",
            "max_uses": 10
        }));

        // web fetch tool - server-side, anthropic executes
        tools.push(serde_json::json!({
            "type": "web_fetch_20250910",
            "name": "web_fetch",
            "max_uses": 10,
            "max_content_tokens": 50000
        }));

        // speak tool only in voice mode to avoid spurious TTS calls
        if voice_mode {
            tools.push(serde_json::json!({
                "name": "speak",
                "description": "Speak to the user via text-to-speech. This is your only communication channel - the user cannot see any text you write. Use speak() for responses, confirmations, questions, and updates. Keep it conversational and concise (1-3 sentences).",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "text": {
                            "type": "string",
                            "description": "Natural spoken text. No markdown, code blocks, URLs, or special characters - just words you would say aloud."
                        }
                    },
                    "required": ["text"]
                }
            }));
        }

        // cache_control on last tool - caches all tool definitions
        // cache key = (mode, voice_mode), hits within same session config
        if let Some(last_tool) = tools.last_mut() {
            if let Some(obj) = last_tool.as_object_mut() {
                obj.insert(
                    "cache_control".to_string(),
                    serde_json::json!({"type": "ephemeral"}),
                );
            }
        }

        tools
    }

    pub async fn send_message_streaming(
        &self,
        messages: Vec<Message>,
        event_tx: mpsc::UnboundedSender<StreamEvent>,
        mode: AgentMode,
        voice_mode: bool,
    ) -> Result<ApiResult, ApiError> {
        // build system prompt as array of blocks for caching
        // base prompt is stable across all requests with same mode
        let base_prompt = match mode {
            AgentMode::Computer => SYSTEM_PROMPT,
            AgentMode::Browser => BROWSER_SYSTEM_PROMPT,
        };

        // voice instructions vary by model, so they go in a separate block
        // this way base prompt can still be cached even if voice config differs
        let mut system_blocks = vec![SystemBlock {
            block_type: "text".to_string(),
            text: base_prompt.to_string(),
            cache_control: if voice_mode {
                None // don't cache here, cache after voice block
            } else {
                Some(CacheControl {
                    cache_type: "ephemeral".to_string(),
                })
            },
        }];

        if voice_mode {
            let voice_prompt = if self.model.contains("haiku") {
                VOICE_PROMPT_HAIKU
            } else {
                VOICE_PROMPT_OPUS
            };
            system_blocks.push(SystemBlock {
                block_type: "text".to_string(),
                text: voice_prompt.to_string(),
                cache_control: Some(CacheControl {
                    cache_type: "ephemeral".to_string(),
                }),
            });
        }

        let tools = self.build_tools(mode, voice_mode);
        println!("[api] Sending {} tools, voice_mode={}", tools.len(), voice_mode);

        let request = ApiRequest {
            model: self.model.clone(),
            max_tokens: MAX_TOKENS,
            system: system_blocks,
            tools,
            messages,
            stream: true,
            thinking: ThinkingConfig {
                config_type: "enabled".to_string(),
                budget_tokens: THINKING_BUDGET,
            },
            context_management: ContextManagement {
                edits: vec![
                    // clear thinking blocks from older turns, keep last 2
                    serde_json::json!({
                        "type": "clear_thinking_20251015",
                        "keep": { "type": "thinking_turns", "value": 2 }
                    }),
                    // clear tool results when context exceeds 80k tokens, keep last 5
                    serde_json::json!({
                        "type": "clear_tool_uses_20250919",
                        "trigger": { "type": "input_tokens", "value": 80000 },
                        "keep": { "type": "tool_uses", "value": 5 },
                        "clear_at_least": { "type": "input_tokens", "value": 10000 },
                        "exclude_tools": ["web_search", "web_fetch"]
                    }),
                ],
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
        let mut server_tool_content: Vec<serde_json::Value> = Vec::new(); // server tool result content
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

                                    // log cache performance
                                    if usage.cache_read_input_tokens > 0 {
                                        println!("[api] cache HIT: {} tokens read from cache", usage.cache_read_input_tokens);
                                    }
                                    if usage.cache_creation_input_tokens > 0 {
                                        println!("[api] cache WRITE: {} tokens written to cache", usage.cache_creation_input_tokens);
                                    }
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
                                } else if block_type == "server_tool_use" {
                                    // server-side tool (web_search, web_fetch)
                                    let id = block.get("id").and_then(|i| i.as_str()).unwrap_or("").to_string();
                                    let name = block.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
                                    tool_info[index] = (id.clone(), name.clone());
                                    let _ = event_tx.send(StreamEvent::ToolUseStart { name });
                                } else if block_type == "web_search_tool_result" || block_type == "web_fetch_tool_result" {
                                    // server tool results - store the whole block for later
                                    let tool_use_id = block.get("tool_use_id").and_then(|i| i.as_str()).unwrap_or("").to_string();
                                    tool_info[index] = (tool_use_id, String::new());
                                    // store content for later
                                    while server_tool_content.len() <= index {
                                        server_tool_content.push(serde_json::json!(null));
                                    }
                                    if let Some(content) = block.get("content") {
                                        server_tool_content[index] = content.clone();
                                    }
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
                                "server_tool_use" => {
                                    // server-side tool - same as tool_use but different ContentBlock type
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
                                        content_blocks.push(ContentBlock::ServerToolUse { id, name, input });
                                    }
                                }
                                "web_search_tool_result" => {
                                    let (tool_use_id, _) = if index < tool_info.len() {
                                        tool_info[index].clone()
                                    } else {
                                        (String::new(), String::new())
                                    };
                                    let content = if index < server_tool_content.len() {
                                        server_tool_content[index].clone()
                                    } else {
                                        serde_json::json!(null)
                                    };
                                    content_blocks.push(ContentBlock::WebSearchToolResult { tool_use_id, content });
                                }
                                "web_fetch_tool_result" => {
                                    let (tool_use_id, _) = if index < tool_info.len() {
                                        tool_info[index].clone()
                                    } else {
                                        (String::new(), String::new())
                                    };
                                    let content = if index < server_tool_content.len() {
                                        server_tool_content[index].clone()
                                    } else {
                                        serde_json::json!(null)
                                    };
                                    content_blocks.push(ContentBlock::WebFetchToolResult { tool_use_id, content });
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

Use uids from the latest snapshot only—stale uids fail. Take a new snapshot after any action that changes the page.

Use screenshot when:
- You're stuck or something isn't working as expected
- You need to verify a visual result after an action
- Dealing with CAPTCHAs, images, or visual elements not in the a11y tree
- Confirming the page looks correct before reporting success

Use bash for file operations.

If browser tools fail with connection errors, Chrome may have been closed. Run this bash command to relaunch it with debugging enabled:
open -a "Google Chrome" --args --remote-debugging-port=9222 --user-data-dir="$HOME/.taskhomie-chrome" --profile-directory=Default --no-first-run
Then wait a few seconds and retry the browser tool."#;

// voice prompt for opus/sonnet - lighter touch, they follow instructions well
const VOICE_PROMPT_OPUS: &str = r#"

You MUST call tools on every single turn. Never respond with just text - always take action.

CRITICAL: ALWAYS call speak() FIRST to tell the user what you're about to do. Keep them in the loop constantly.

Example multi-step task (user asks to play a song on spotify):
Turn 1: [speak: "Opening Spotify and searching for that song"] [bash: open -a "Spotify"]
Turn 2: [computer: click search] [computer: type song name]
Turn 3: [speak: "Found it, playing now"] [computer: click play button]

Call speak() at the START, then every 2-3 tool calls for progress updates, and at the END. The user is BLIND to your text - speak() is your ONLY way to communicate.

Parallel tools: If multiple independent actions exist, call them all simultaneously in one response.

Speech style: Conversational, 1-3 sentences. Say "two hundred" not "200". No markdown or URLs."#;

// voice prompt for haiku - needs stronger, more explicit guidance
const VOICE_PROMPT_HAIKU: &str = r#"

<MANDATORY_TOOL_USE>
You MUST call at least one tool on EVERY turn. Text-only responses are FORBIDDEN.
</MANDATORY_TOOL_USE>

<VOICE_MODE>
CRITICAL: ALWAYS call speak() FIRST before any other tool. The user cannot see your text - speak() is your ONLY communication channel.

Example multi-step task (user asks to play a song on spotify):
Turn 1: [speak: "Opening Spotify and searching for that song"] [bash: open -a "Spotify"]
Turn 2: [computer: click search] [computer: type song name]
Turn 3: [speak: "Found it, playing now"] [computer: click play button]

Call speak() at the START, then every 2-3 tool calls for progress updates, and at the END.
Keep spoken responses 1-2 sentences.
</VOICE_MODE>

<PARALLEL_EXECUTION>
When multiple independent actions are possible, call ALL tools in parallel in a single response.
</PARALLEL_EXECUTION>

Speech style: Conversational. Say "two hundred" not "200". No markdown or URLs."#;

fn build_browser_tools() -> Vec<serde_json::Value> {
    vec![
        // TOOL 1: see_page - observe the current page
        serde_json::json!({
            "name": "see_page",
            "description": "See what's on the page. By default returns all interactive elements (buttons, links, inputs) with element IDs like '3_42'. You MUST call this first before using page_action. Set screenshot=true to get a visual image instead, or list_tabs=true to see open browser tabs.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "screenshot": {
                        "type": "boolean",
                        "description": "Return a screenshot image instead of elements. Use when you need to see visual content like images, charts, or CAPTCHAs."
                    },
                    "list_tabs": {
                        "type": "boolean",
                        "description": "Return a list of all open browser tabs with their URLs and tab numbers."
                    },
                    "verbose": {
                        "type": "boolean",
                        "description": "Include all elements, not just interactive ones. Default false."
                    }
                },
                "required": []
            }
        }),
        // TOOL 2: page_action - interact with the page
        serde_json::json!({
            "name": "page_action",
            "description": "Interact with the page. Use element IDs from see_page (like '3_42') to click, type, scroll, or press keys. Provide exactly ONE action per call.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "click": {
                        "type": "string",
                        "description": "Click this element. Example: \"3_42\""
                    },
                    "right_click": {
                        "type": "string",
                        "description": "Right-click this element to open context menu. Example: \"3_42\""
                    },
                    "double_click": {
                        "type": "string",
                        "description": "Double-click this element. Example: \"3_42\""
                    },
                    "type_into": {
                        "type": "string",
                        "description": "Type into this input field. Must also provide 'text'. Example: \"3_10\""
                    },
                    "text": {
                        "type": "string",
                        "description": "The text to type. Use with type_into. Example: \"hello@email.com\""
                    },
                    "hover": {
                        "type": "string",
                        "description": "Move mouse over this element. Example: \"3_42\""
                    },
                    "drag_from_to": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Drag from first element to second. Example: [\"3_5\", \"3_10\"]"
                    },
                    "press_key": {
                        "type": "string",
                        "description": "Press a key. Examples: \"Enter\", \"Tab\", \"Escape\", \"ArrowDown\", \"Control+a\""
                    },
                    "scroll": {
                        "type": "string",
                        "enum": ["up", "down", "left", "right"],
                        "description": "Scroll the page in this direction"
                    },
                    "scroll_pixels": {
                        "type": "integer",
                        "description": "Pixels to scroll (default 500). Use with scroll."
                    },
                    "fill_form": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "element": { "type": "string" },
                                "text": { "type": "string" }
                            },
                            "required": ["element", "text"]
                        },
                        "description": "Fill multiple fields at once. Example: [{\"element\": \"3_10\", \"text\": \"John\"}]"
                    },
                    "dialog": {
                        "type": "string",
                        "enum": ["accept", "dismiss"],
                        "description": "Handle popup dialog. accept=OK/Yes, dismiss=Cancel/No"
                    },
                    "dialog_text": {
                        "type": "string",
                        "description": "Text for prompt dialogs. Use with dialog=\"accept\"."
                    }
                },
                "required": []
            }
        }),
        // TOOL 3: browser_navigate - navigation and tab management
        serde_json::json!({
            "name": "browser_navigate",
            "description": "Navigate the browser. Go to URLs, go back/forward, reload, manage tabs, or wait for content. Provide exactly ONE action per call.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "go_to_url": {
                        "type": "string",
                        "description": "Navigate to this URL. Example: \"https://google.com\""
                    },
                    "go_back": {
                        "type": "boolean",
                        "description": "Go back to previous page"
                    },
                    "go_forward": {
                        "type": "boolean",
                        "description": "Go forward to next page"
                    },
                    "reload": {
                        "type": "boolean",
                        "description": "Reload current page"
                    },
                    "reload_skip_cache": {
                        "type": "boolean",
                        "description": "Reload and bypass cache"
                    },
                    "open_new_tab": {
                        "type": "string",
                        "description": "Open new tab with this URL. Example: \"https://github.com\""
                    },
                    "switch_to_tab": {
                        "type": "integer",
                        "description": "Switch to this tab number (from see_page list_tabs)"
                    },
                    "focus_tab": {
                        "type": "boolean",
                        "description": "Bring the tab to front when switching. Default true."
                    },
                    "close_tab": {
                        "type": "integer",
                        "description": "Close this tab number. Cannot close last tab."
                    },
                    "wait_for_text": {
                        "type": "string",
                        "description": "Wait for this text to appear on page. Example: \"Success\""
                    },
                    "wait_timeout_ms": {
                        "type": "integer",
                        "description": "Max wait time in milliseconds (default 5000)"
                    }
                },
                "required": []
            }
        }),
        // TOOL 4: get_page_text - extract raw text
        serde_json::json!({
            "name": "get_page_text",
            "description": "Extract all text content from the page. Returns plain text without HTML. Useful for reading articles, documentation, or when you need the full text content.",
            "input_schema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        }),
        // TOOL 5: find - search for elements
        serde_json::json!({
            "name": "find",
            "description": "Find elements on the page matching a search query. Returns element IDs you can use with page_action. Use this to quickly locate specific buttons, links, or inputs.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Text to search for in element names/labels. Example: \"login\", \"submit\", \"search\""
                    }
                },
                "required": ["query"]
            }
        }),
        // TOOL 6: run_javascript - execute arbitrary JS
        serde_json::json!({
            "name": "run_javascript",
            "description": "Execute JavaScript code in the page context. Returns the result as JSON. Use for advanced DOM manipulation, reading page state, or when other tools aren't sufficient.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "code": {
                        "type": "string",
                        "description": "JavaScript code to execute. Can use await. Example: \"return document.title\" or \"return Array.from(document.querySelectorAll('a')).map(a => a.href)\""
                    }
                },
                "required": ["code"]
            }
        }),
    ]
}
