use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;

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
    pub source_type: String,  // "base64"
    pub media_type: String,   // "image/png"
    pub data: String,         // base64 encoded image
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: Vec<ContentBlock>,
}

#[derive(Debug, Serialize)]
struct ApiRequest {
    model: String,
    max_tokens: u32,
    system: String,
    tools: Vec<serde_json::Value>,
    messages: Vec<Message>,
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

    pub async fn send_message(&self, messages: Vec<Message>) -> Result<ApiResponse, ApiError> {
        let tools = vec![
            // computer use tool
            serde_json::json!({
                "type": "computer_20250124",
                "name": "computer",
                "display_width_px": DISPLAY_WIDTH,
                "display_height_px": DISPLAY_HEIGHT,
                "display_number": 1
            }),
            // bash tool
            serde_json::json!({
                "type": "bash_20250124",
                "name": "bash"
            }),
            // finish_run tool
            serde_json::json!({
                "name": "finish_run",
                "description": "Call this tool when you have completed the user's task. Provide a brief summary of what was accomplished.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "success": {
                            "type": "boolean",
                            "description": "Whether the task was completed successfully"
                        },
                        "message": {
                            "type": "string",
                            "description": "Brief summary of what was accomplished"
                        }
                    },
                    "required": ["success", "message"]
                }
            }),
        ];

        let request = ApiRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            system: SYSTEM_PROMPT.to_string(),
            tools,
            messages,
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
            // try to parse error
            if let Ok(err) = serde_json::from_str::<ApiErrorResponse>(&body) {
                return Err(ApiError::Api(err.error.message));
            }
            return Err(ApiError::Api(format!("HTTP {}: {}", status, body)));
        }

        serde_json::from_str(&body).map_err(|e| ApiError::Parse(e.to_string()))
    }
}

const SYSTEM_PROMPT: &str = r#"You are an AI assistant that controls a computer to help users complete tasks.

You have access to:
1. computer tool - take screenshots, move mouse, click, type, keyboard shortcuts
2. bash tool - run shell commands (preferred for file ops, git, builds, scripts)
3. finish_run tool - call when task is complete

Guidelines:
- Prefer bash for: file operations, git, running scripts, installing packages, builds
- Use computer for: GUI interactions, clicking buttons, visual tasks
- After GUI actions, take a screenshot to verify before proceeding
- Be precise with mouse clicks
- Use keyboard shortcuts when possible (cmd+c, cmd+v, cmd+tab)
- Before typing in a text field, click to focus it first
- If something doesn't work, try an alternative approach
- When done, call finish_run with a summary

Always call a tool. Never respond with just text."#;
