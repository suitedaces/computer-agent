use crate::api::{AnthropicClient, ApiError, ContentBlock, ImageSource, Message, ToolResultContent};
use crate::bash::BashExecutor;
use crate::computer::{ComputerAction, ComputerControl, ComputerError};
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
    #[error("Stopped by user")]
    Stopped,
    #[error("No API key set")]
    NoApiKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentUpdate {
    pub update_type: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot: Option<String>,
}

pub struct Agent {
    api_key: Option<String>,
    running: Arc<AtomicBool>,
    computer: Mutex<Option<ComputerControl>>,
    bash: Mutex<BashExecutor>,
}

impl Agent {
    pub fn new(running: Arc<AtomicBool>) -> Self {
        Self {
            api_key: None,
            running,
            computer: Mutex::new(None),
            bash: Mutex::new(BashExecutor::new()),
        }
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

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    pub async fn run(
        &self,
        instructions: String,
        app_handle: AppHandle,
    ) -> Result<(), AgentError> {
        println!("[agent] run() starting with: {}", instructions);

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

        let client = AnthropicClient::new(api_key);
        let mut messages: Vec<Message> = Vec::new();

        // emit started
        self.emit(&app_handle, "started", "Agent started", None, None);
        println!("[agent] Emitted started event");

        // first message - just the user instructions
        // claude will call screenshot action first to see the screen
        messages.push(Message {
            role: "user".to_string(),
            content: vec![ContentBlock::Text {
                text: instructions.clone(),
            }],
        });

        // agent loop
        let max_iterations = 50;
        let mut iteration = 0;
        println!("[agent] Starting agent loop");

        while self.running.load(Ordering::SeqCst) && iteration < max_iterations {
            iteration += 1;
            println!("[agent] Iteration {}", iteration);

            // call API
            println!("[agent] Calling Anthropic API...");
            let response = match client.send_message(messages.clone()).await {
                Ok(r) => {
                    println!("[agent] API response received, {} blocks", r.content.len());
                    r
                }
                Err(e) => {
                    println!("[agent] API error: {:?}", e);
                    self.emit(&app_handle, "error", &e.to_string(), None, None);
                    break;
                }
            };

            // add assistant response to history
            messages.push(Message {
                role: "assistant".to_string(),
                content: response.content.clone(),
            });

            let mut tool_results: Vec<ContentBlock> = Vec::new();
            let mut should_finish = false;

            for block in &response.content {
                if !self.running.load(Ordering::SeqCst) {
                    break;
                }

                println!("[agent] Processing block: {:?}", block);

                match block {
                    ContentBlock::Text { text } => {
                        println!("[agent] Text: {}", text);
                        self.emit(&app_handle, "thinking", text, None, None);
                    }

                    ContentBlock::ToolUse { id, name, input } => {
                        if name == "finish_run" {
                            let message = input
                                .get("message")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Task completed");
                            self.emit(&app_handle, "finished", message, None, None);
                            should_finish = true;
                            break;
                        }

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

                            // execute action
                            let result = {
                                let computer_guard = self.computer.lock().await;
                                let computer = computer_guard.as_ref().unwrap();
                                computer.perform_action(&action)
                            };

                            match result {
                                Ok(action_screenshot) => {
                                    // if action was screenshot, use that result
                                    // otherwise take a new screenshot after the action
                                    let screenshot = if let Some(s) = action_screenshot {
                                        s
                                    } else {
                                        let computer_guard = self.computer.lock().await;
                                        let computer = computer_guard.as_ref().unwrap();
                                        computer.take_screenshot()?
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
                                                media_type: "image/png".to_string(),
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

                                // execute
                                let bash = self.bash.lock().await;
                                let result = bash.execute(cmd);

                                let output = match result {
                                    Ok(out) => {
                                        self.emit(&app_handle, "bash_result", &out.to_string(), None, None);
                                        out.to_string()
                                    }
                                    Err(e) => {
                                        let err_msg = format!("Error: {}", e);
                                        self.emit(&app_handle, "bash_result", &err_msg, None, None);
                                        err_msg
                                    }
                                };

                                tool_results.push(ContentBlock::ToolResult {
                                    tool_use_id: id.clone(),
                                    content: vec![ToolResultContent::Text { text: output }],
                                });
                            }
                        }
                    }

                    _ => {}
                }
            }

            if should_finish {
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
        use tauri::Manager;
        let payload = AgentUpdate {
            update_type: update_type.to_string(),
            message: message.to_string(),
            action,
            screenshot,
        };
        // try emitting to specific window
        if let Some(window) = app_handle.get_webview_window("main") {
            match window.emit("agent-update", payload) {
                Ok(_) => println!("[agent] Emit success: {}", update_type),
                Err(e) => println!("[agent] Emit FAILED: {} - {:?}", update_type, e),
            }
        } else {
            println!("[agent] Window 'main' not found!");
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
        _ => format!("Action: {}", action.action),
    }
}
