use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::mpsc;

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const BETA_HEADER: &str = "computer-use-2025-01-24";
const API_VERSION: &str = "2023-06-01";
const DISPLAY_WIDTH: u32 = 1280;
const DISPLAY_HEIGHT: u32 = 800;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("API key not set")]
    MissingApiKey,
    #[error("API error: {0}")]
    Api(String),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Stream error: {0}")]
    Stream(String),
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
pub struct ApiResponse {
    pub content: Vec<ContentBlock>,
    pub stop_reason: Option<String>,
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
    TextDelta { index: usize, text: String },
    ThinkingDelta { index: usize, thinking: String },
    ToolUseStart { index: usize, id: String, name: String },
    InputJsonDelta { index: usize, partial_json: String },
    ContentBlockStop { index: usize },
    MessageStop,
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

    fn build_tools(&self, mcp_tools: &[serde_json::Value]) -> Vec<serde_json::Value> {
        let mut tools = vec![
            serde_json::json!({
                "type": "computer_20250124",
                "name": "computer",
                "display_width_px": DISPLAY_WIDTH,
                "display_height_px": DISPLAY_HEIGHT,
                "display_number": 1
            }),
            serde_json::json!({
                "type": "bash_20250124",
                "name": "bash"
            }),
        ];
        tools.extend(mcp_tools.iter().cloned());
        tools
    }

    pub async fn send_message_streaming(
        &self,
        messages: Vec<Message>,
        event_tx: mpsc::UnboundedSender<StreamEvent>,
        mcp_tools: &[serde_json::Value],
    ) -> Result<Vec<ContentBlock>, ApiError> {
        let request = ApiRequest {
            model: self.model.clone(),
            max_tokens: 16000,
            system: SYSTEM_PROMPT.to_string(),
            tools: self.build_tools(mcp_tools),
            messages,
            stream: true,
            thinking: ThinkingConfig {
                config_type: "enabled".to_string(),
                budget_tokens: 5000,
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
                                    let _ = event_tx.send(StreamEvent::ToolUseStart { index, id, name });
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
                                                index,
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
                                                index,
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
                                            let _ = event_tx.send(StreamEvent::InputJsonDelta {
                                                index,
                                                partial_json: json.to_string(),
                                            });
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }

                        "content_block_stop" => {
                            let index = event.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
                            let _ = event_tx.send(StreamEvent::ContentBlockStop { index });

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
                                    if index < current_tool_json.len() && !current_tool_json[index].is_empty() {
                                        let (id, name) = if index < tool_info.len() {
                                            tool_info[index].clone()
                                        } else {
                                            (String::new(), String::new())
                                        };
                                        let input: serde_json::Value = serde_json::from_str(&current_tool_json[index])
                                            .unwrap_or(serde_json::json!({}));
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

        Ok(content_blocks)
    }

    // keep non-streaming version for fallback
    pub async fn send_message(&self, messages: Vec<Message>, mcp_tools: &[serde_json::Value]) -> Result<ApiResponse, ApiError> {
        let request = ApiRequest {
            model: self.model.clone(),
            max_tokens: 16000,
            system: SYSTEM_PROMPT.to_string(),
            tools: self.build_tools(mcp_tools),
            messages,
            stream: false,
            thinking: ThinkingConfig {
                config_type: "enabled".to_string(),
                budget_tokens: 5000,
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
        let body = response.text().await?;

        if !status.is_success() {
            if let Ok(err) = serde_json::from_str::<ApiErrorResponse>(&body) {
                return Err(ApiError::Api(err.error.message));
            }
            return Err(ApiError::Api(format!("HTTP {}: {}", status, body)));
        }

        serde_json::from_str(&body).map_err(|e| ApiError::Parse(e.to_string()))
    }
}

const SYSTEM_PROMPT: &str = r#"You are taskhomie, a computer control agent in a macOS menubar app. You see the screen, move the mouse, click, type, and run bash commands.

Rules:
- Click to focus before typing
- Screenshot after actions to verify
- Use keyboard shortcuts (cmd+c, cmd+v, cmd+tab, cmd+w, cmd+q)
- If something fails, try another approach
- Always call a tool, never just text
- Keep responses concise

macOS CLI shortcuts (fast, use when applicable):
- open -a "App" (launch), open file.pdf (default app), open https://url (browser)
- pbpaste/pbcopy (clipboard), mdfind "query" (spotlight search)
- osascript -e 'tell app "X" to activate/quit'

Use computer tool for:
- Browser interactions (clicking links, filling forms, reading page content)
- Any visual/UI task requiring mouse clicks or reading the screen
- Tasks where you need to see what happened"#;
