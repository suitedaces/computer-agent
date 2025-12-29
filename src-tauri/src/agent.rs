use crate::api::{AnthropicClient, ApiError, ContentBlock, ImageSource, Message, StreamEvent, ToolResultContent};
use crate::bash::BashExecutor;
use crate::computer::{ComputerAction, ComputerControl, ComputerError};
use crate::mcp::{McpClient, McpError, SharedMcpClient};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use thiserror::Error;
use tokio::sync::Mutex;

#[derive(Error, Debug)]
pub enum AgentError {
    #[error("API error: {0}")]
    Api(#[from] ApiError),
    #[error("Computer error: {0}")]
    Computer(#[from] ComputerError),
    #[error("MCP error: {0}")]
    Mcp(#[from] McpError),
    #[error("No API key set")]
    NoApiKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentMode {
    Computer,
    Browser,
}

impl Default for AgentMode {
    fn default() -> Self {
        Self::Computer
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentUpdate {
    pub update_type: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bash_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryMessage {
    pub role: String,
    pub content: String,
}

pub struct Agent {
    api_key: Option<String>,
    running: Arc<AtomicBool>,
    computer: Mutex<Option<ComputerControl>>,
    bash: Mutex<BashExecutor>,
    mcp_client: Option<SharedMcpClient>,
}

impl Agent {
    pub fn new(running: Arc<AtomicBool>) -> Self {
        Self {
            api_key: None,
            running,
            computer: Mutex::new(None),
            bash: Mutex::new(BashExecutor::new()),
            mcp_client: None,
        }
    }

    pub fn set_mcp_client(&mut self, client: SharedMcpClient) {
        self.mcp_client = Some(client);
    }

    pub fn set_api_key(&mut self, key: String) {
        self.api_key = Some(key);
    }

    pub fn has_api_key(&self) -> bool {
        self.api_key.is_some()
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub async fn run(
        &self,
        instructions: String,
        model: String,
        mode: AgentMode,
        history: Vec<HistoryMessage>,
        context_screenshot: Option<String>,
        app_handle: AppHandle,
    ) -> Result<(), AgentError> {
        println!("[agent] run() starting with: {} (model: {}, mode: {:?}, history: {} msgs, screenshot: {})",
            instructions, model, mode, history.len(), context_screenshot.is_some());

        let api_key = self.api_key.clone().ok_or(AgentError::NoApiKey)?;
        println!("[agent] API key present");

        // init computer control
        println!("[agent] Initializing computer control...");
        let computer = match ComputerControl::new() {
            Ok(c) => {
                println!("[agent] Computer control initialized");
                c
            }
            Err(e) => {
                println!("[agent] Computer control failed: {:?}", e);
                self.emit(&app_handle, "error", &format!("Computer init failed: {}", e), None, None);
                return Err(e.into());
            }
        };
        *self.computer.lock().await = Some(computer);

        self.running.store(true, Ordering::SeqCst);

        // get mcp tools only in browser mode
        let mcp_tools = if mode == AgentMode::Browser {
            if let Some(ref mcp_client) = self.mcp_client {
                let client: tokio::sync::RwLockReadGuard<'_, McpClient> = mcp_client.read().await;
                if client.is_connected() {
                    let tools = client.get_tools_for_claude();
                    println!("[agent] MCP tools available: {}", tools.len());
                    tools
                } else {
                    println!("[agent] MCP client not connected");
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        } else {
            println!("[agent] Computer mode - skipping MCP tools");
            Vec::new()
        };

        let client = AnthropicClient::new(api_key, model);
        let mut messages: Vec<Message> = Vec::new();

        // emit started to all windows
        self.emit(&app_handle, "started", "Agent started", None, None);
        let _ = app_handle.emit("agent:started", ());

        // emit user message so all windows can display it
        let _ = app_handle.emit("agent-update", AgentUpdate {
            update_type: "user_message".to_string(),
            message: instructions.clone(),
            action: None,
            screenshot: context_screenshot.clone(),
            bash_command: None,
            exit_code: None,
        });
        println!("[agent] Emitted started + user_message events");

        // add conversation history first
        for msg in history {
            messages.push(Message {
                role: msg.role,
                content: vec![ContentBlock::Text { text: msg.content }],
            });
        }

        // build user message content - include screenshot if provided
        let mut user_content: Vec<ContentBlock> = Vec::new();

        // add context screenshot first if provided (from hotkey help mode)
        if let Some(screenshot_data) = context_screenshot {
            user_content.push(ContentBlock::Image {
                source: ImageSource {
                    source_type: "base64".to_string(),
                    media_type: "image/jpeg".to_string(),
                    data: screenshot_data,
                },
            });
        }

        // add text instructions
        user_content.push(ContentBlock::Text {
            text: instructions.clone(),
        });

        messages.push(Message {
            role: "user".to_string(),
            content: user_content,
        });

        // agent loop - limit iterations to prevent runaway tasks.
        // 50 is enough for complex multi-step tasks while providing a safety bound
        const MAX_ITERATIONS: usize = 50;
        let mut iteration = 0;
        println!("[agent] Starting agent loop");

        while self.running.load(Ordering::SeqCst) && iteration < MAX_ITERATIONS {
            iteration += 1;
            println!("[agent] Iteration {}", iteration);

            // call API with streaming
            println!("[agent] Calling Anthropic API (streaming)...");
            let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<StreamEvent>();

            // spawn stream consumer to emit text deltas in real-time
            let app_handle_clone = app_handle.clone();
            let stream_task = tokio::spawn(async move {
                while let Some(event) = event_rx.recv().await {
                    match event {
                        StreamEvent::ThinkingDelta { thinking } => {
                            // emit globally so all windows receive it
                            let _ = app_handle_clone.emit("agent-stream", serde_json::json!({
                                "type": "thinking_delta",
                                "text": thinking
                            }));
                        }
                        StreamEvent::TextDelta { text } => {
                            // emit globally so all windows receive it
                            let _ = app_handle_clone.emit("agent-stream", serde_json::json!({
                                "type": "text_delta",
                                "text": text
                            }));
                        }
                        StreamEvent::ToolUseStart { name } => {
                            let _ = app_handle_clone.emit("agent-stream", serde_json::json!({
                                "type": "tool_start",
                                "name": name
                            }));
                        }
                        StreamEvent::MessageStop => {}
                    }
                }
            });

            let response_content = match client.send_message_streaming(messages.clone(), event_tx, &mcp_tools, mode).await {
                Ok(content) => {
                    println!("[agent] API streaming response complete, {} blocks", content.len());
                    content
                }
                Err(e) => {
                    println!("[agent] API error: {:?}", e);
                    self.emit(&app_handle, "error", &e.to_string(), None, None);
                    break;
                }
            };

            // wait for stream consumer to finish
            let _ = stream_task.await;

            // add assistant response to history
            messages.push(Message {
                role: "assistant".to_string(),
                content: response_content.clone(),
            });

            let mut tool_results: Vec<ContentBlock> = Vec::new();

            for block in &response_content {
                if !self.running.load(Ordering::SeqCst) {
                    break;
                }

                println!("[agent] Processing block: {:?}", block);

                match block {
                    ContentBlock::Thinking { thinking, .. } => {
                        println!("[agent] Thinking: {}...", &thinking[..thinking.len().min(100)]);
                        self.emit(&app_handle, "thinking", thinking, None, None);
                    }

                    ContentBlock::RedactedThinking { .. } => {
                        // keep in history but don't display
                    }

                    ContentBlock::Text { text } => {
                        println!("[agent] Text: {}", text);
                        self.emit(&app_handle, "response", text, None, None);
                    }

                    ContentBlock::ToolUse { id, name, input } => {
                        if name == "computer" {
                            // parse action
                            let action: ComputerAction = match serde_json::from_value(input.clone())
                            {
                                Ok(a) => a,
                                Err(e) => {
                                    self.emit(
                                        &app_handle,
                                        "error",
                                        &format!("Failed to parse action: {}", e),
                                        None,
                                        None,
                                    );
                                    continue;
                                }
                            };

                            // emit action
                            self.emit(
                                &app_handle,
                                "action",
                                &format_action(&action),
                                Some(input.clone()),
                                None,
                            );
                            // emit globally for mini
                            match app_handle.emit("agent:action", serde_json::json!({
                                "action": action.action,
                                "text": action.text
                            })) {
                                Ok(_) => println!("[agent] agent:action emitted OK"),
                                Err(e) => println!("[agent] agent:action emit FAILED: {:?}", e),
                            }

                            // get window id for native screenshot exclusion
                            #[cfg(target_os = "macos")]
                            let window_id: Option<u32> = {
                                use tauri_nspanel::ManagerExt;
                                app_handle.get_webview_panel("main").ok().map(|panel| {
                                    let ns_panel = panel.as_panel();
                                    // SAFETY: NSPanel inherits from NSWindow, windowNumber is a valid
                                    // selector that returns the window's unique identifier as NSInteger.
                                    // The cast to u32 is safe because window numbers are non-negative.
                                    unsafe {
                                        let num: isize = objc2::msg_send![ns_panel, windowNumber];
                                        num as u32
                                    }
                                })
                            };
                            #[cfg(not(target_os = "macos"))]
                            let window_id: Option<u32> = None;

                            // execute action on blocking thread (enigo requires main-thread-like context)
                            let action_clone = action.clone();
                            let screen_w = {
                                let computer_guard = self.computer.lock().await;
                                let computer = computer_guard.as_ref().unwrap();
                                computer.screen_width
                            };
                            let screen_h = {
                                let computer_guard = self.computer.lock().await;
                                let computer = computer_guard.as_ref().unwrap();
                                computer.screen_height
                            };
                            let result = tokio::task::spawn_blocking(move || {
                                let computer = ComputerControl::with_dimensions(screen_w, screen_h);
                                computer.perform_action(&action_clone)
                            }).await.map_err(|e| AgentError::Computer(ComputerError::Input(e.to_string())))?;

                            match result {
                                Ok(_action_result) => {
                                    // always take screenshot with window exclusion
                                    let screenshot = {
                                        #[cfg(target_os = "macos")]
                                        {
                                            if let Some(wid) = window_id {
                                                let computer_guard = self.computer.lock().await;
                                                let computer = computer_guard.as_ref().unwrap();
                                                computer.take_screenshot_excluding(wid)?
                                            } else {
                                                let computer_guard = self.computer.lock().await;
                                                let computer = computer_guard.as_ref().unwrap();
                                                computer.take_screenshot()?
                                            }
                                        }
                                        #[cfg(not(target_os = "macos"))]
                                        {
                                            let computer_guard = self.computer.lock().await;
                                            let computer = computer_guard.as_ref().unwrap();
                                            computer.take_screenshot()?
                                        }
                                    };

                                    self.emit(
                                        &app_handle,
                                        "screenshot",
                                        "Screenshot",
                                        None,
                                        Some(screenshot.clone()),
                                    );

                                    tool_results.push(ContentBlock::ToolResult {
                                        tool_use_id: id.clone(),
                                        content: vec![ToolResultContent::Image {
                                            source: ImageSource {
                                                source_type: "base64".to_string(),
                                                media_type: "image/jpeg".to_string(),
                                                data: screenshot,
                                            },
                                        }],
                                    });
                                }
                                Err(e) => {
                                    tool_results.push(ContentBlock::ToolResult {
                                        tool_use_id: id.clone(),
                                        content: vec![ToolResultContent::Text {
                                            text: format!("Error: {}", e),
                                        }],
                                    });
                                }
                            }
                        }

                        if name == "bash" {
                            let command = input.get("command").and_then(|v| v.as_str());
                            let restart = input.get("restart").and_then(|v| v.as_bool()).unwrap_or(false);

                            if restart {
                                let mut bash = self.bash.lock().await;
                                bash.restart();
                                self.emit(&app_handle, "action", "Restarting bash session", Some(input.clone()), None);
                                tool_results.push(ContentBlock::ToolResult {
                                    tool_use_id: id.clone(),
                                    content: vec![ToolResultContent::Text {
                                        text: "Bash session restarted".to_string(),
                                    }],
                                });
                            } else if let Some(cmd) = command {
                                // emit action
                                let preview = if cmd.len() > 50 {
                                    format!("$ {}...", &cmd[..50])
                                } else {
                                    format!("$ {}", cmd)
                                };
                                self.emit(&app_handle, "action", &preview, Some(input.clone()), None);
                                // emit globally for mini
                                let _ = app_handle.emit("agent:bash", serde_json::json!({ "command": cmd }));

                                // execute
                                let bash = self.bash.lock().await;
                                let result = bash.execute(cmd);

                                let output = match result {
                                    Ok(out) => {
                                        let code = out.exit_code;
                                        let text = out.to_string();
                                        self.emit_with_exit_code(&app_handle, "bash_result", &text, None, None, Some(code));
                                        text
                                    }
                                    Err(e) => {
                                        let err_msg = format!("Error: {}", e);
                                        self.emit_with_exit_code(&app_handle, "bash_result", &err_msg, None, None, Some(-1));
                                        err_msg
                                    }
                                };

                                tool_results.push(ContentBlock::ToolResult {
                                    tool_use_id: id.clone(),
                                    content: vec![ToolResultContent::Text { text: output }],
                                });
                            }
                        } else if let Some(ref mcp_client) = self.mcp_client {
                            // check if it's an mcp tool
                            let is_mcp_tool = {
                                let client: tokio::sync::RwLockReadGuard<'_, McpClient> = mcp_client.read().await;
                                client.has_tool(name)
                            };

                            if is_mcp_tool {
                                println!("[agent] Calling MCP tool: {}", name);
                                self.emit(
                                    &app_handle,
                                    "action",
                                    &format!("MCP: {}", name),
                                    Some(input.clone()),
                                    None,
                                );
                                // emit globally for mini
                                let _ = app_handle.emit("agent:mcp_tool", serde_json::json!({ "name": name }));

                                let arguments = input.as_object().cloned();
                                println!("[agent] MCP tool args: {:?}", arguments);
                                let client: tokio::sync::RwLockReadGuard<'_, McpClient> = mcp_client.read().await;
                                let result: Result<String, McpError> = client.call_tool(name, arguments).await;

                                match result {
                                    Ok(output) => {
                                        println!("[agent] MCP tool success: {}", &output[..output.len().min(200)]);
                                        self.emit(&app_handle, "mcp_result", &output, None, None);
                                        tool_results.push(ContentBlock::ToolResult {
                                            tool_use_id: id.clone(),
                                            content: vec![ToolResultContent::Text { text: output }],
                                        });
                                    }
                                    Err(e) => {
                                        let err_msg = format!("MCP error: {}", e);
                                        println!("[agent] MCP tool failed: {}", err_msg);
                                        self.emit(&app_handle, "error", &err_msg, None, None);
                                        tool_results.push(ContentBlock::ToolResult {
                                            tool_use_id: id.clone(),
                                            content: vec![ToolResultContent::Text { text: err_msg }],
                                        });
                                    }
                                }
                            }
                        }
                    }

                    _ => {}
                }
            }

            // clear streaming text in mini on each message complete
            let _ = app_handle.emit("agent:message", ());

            // check if stopped during tool execution
            if !self.running.load(Ordering::SeqCst) {
                println!("[agent] Stopped by user");
                self.emit(&app_handle, "finished", "Stopped", None, None);
                break;
            }

            // if no tools were used, the task is complete
            if tool_results.is_empty() {
                println!("[agent] No tool calls, task complete");
                self.emit(&app_handle, "finished", "Task completed", None, None);
                break;
            }

            messages.push(Message {
                role: "user".to_string(),
                content: tool_results,
            });
        }

        self.running.store(false, Ordering::SeqCst);
        let _ = app_handle.emit("agent:stopped", ());
        Ok(())
    }

    fn emit(
        &self,
        app_handle: &AppHandle,
        update_type: &str,
        message: &str,
        action: Option<serde_json::Value>,
        screenshot: Option<String>,
    ) {
        self.emit_with_exit_code(app_handle, update_type, message, action, screenshot, None);
    }

    fn emit_with_exit_code(
        &self,
        app_handle: &AppHandle,
        update_type: &str,
        message: &str,
        action: Option<serde_json::Value>,
        screenshot: Option<String>,
        exit_code: Option<i32>,
    ) {
        let payload = AgentUpdate {
            update_type: update_type.to_string(),
            message: message.to_string(),
            action,
            screenshot,
            bash_command: None,
            exit_code,
        };
        // emit globally so both main and spotlight windows receive events
        match app_handle.emit("agent-update", payload) {
            Ok(_) => println!("[agent] Emit success: {}", update_type),
            Err(e) => println!("[agent] Emit FAILED: {} - {:?}", update_type, e),
        }
    }
}

fn format_action(action: &ComputerAction) -> String {
    match action.action.as_str() {
        "screenshot" => "Taking screenshot".to_string(),
        "mouse_move" => {
            if let Some(coord) = action.coordinate {
                format!("Moving mouse to ({}, {})", coord[0], coord[1])
            } else {
                "Moving mouse".to_string()
            }
        }
        "left_click" => {
            if let Some(coord) = action.coordinate {
                format!("Clicking at ({}, {})", coord[0], coord[1])
            } else {
                "Left click".to_string()
            }
        }
        "right_click" => "Right click".to_string(),
        "double_click" => {
            if let Some(coord) = action.coordinate {
                format!("Double clicking at ({}, {})", coord[0], coord[1])
            } else {
                "Double click".to_string()
            }
        }
        "type" => {
            if let Some(text) = &action.text {
                let preview = if text.len() > 30 {
                    format!("{}...", &text[..30])
                } else {
                    text.clone()
                };
                format!("Typing: \"{}\"", preview)
            } else {
                "Typing".to_string()
            }
        }
        "key" => {
            if let Some(key) = &action.text {
                format!("Pressing key: {}", key)
            } else {
                "Key press".to_string()
            }
        }
        "scroll" => {
            let dir = action.scroll_direction.as_deref().unwrap_or("down");
            format!("Scrolling {}", dir)
        }
        "wait" => "Waiting".to_string(),
        _ => format!("Action: {}", action.action),
    }
}
