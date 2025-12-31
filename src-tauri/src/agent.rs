use crate::api::{AnthropicClient, ApiError, ContentBlock, ImageSource, Message, StreamEvent, ToolResultContent};
use crate::storage::{self, Conversation};
use crate::bash::BashExecutor;
use crate::browser::{BrowserClient, SharedBrowserClient};
use crate::computer::{ComputerAction, ComputerControl, ComputerError};
use crate::voice::{create_tts_client, TtsClient};
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
    #[error("Browser error: {0}")]
    Browser(#[from] anyhow::Error),
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
    pub tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_input: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<serde_json::Value>, // deprecated, use tool_input
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bash_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
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
    browser_client: SharedBrowserClient,
}

impl Agent {
    pub fn new(running: Arc<AtomicBool>) -> Self {
        Self {
            api_key: None,
            running,
            computer: Mutex::new(None),
            bash: Mutex::new(BashExecutor::new()),
            browser_client: crate::browser::create_shared_browser_client(),
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

    pub async fn run(
        &self,
        instructions: String,
        model: String,
        mode: AgentMode,
        voice_mode: bool,
        history: Vec<HistoryMessage>,
        context_screenshot: Option<String>,
        conversation_id: Option<String>,
        app_handle: AppHandle,
    ) -> Result<(), AgentError> {
        println!("[agent] run() starting with: {} (model: {}, mode: {:?}, history: {} msgs, screenshot: {}, conv: {:?})",
            instructions, model, mode, history.len(), context_screenshot.is_some(), conversation_id);

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

        // connect browser client in browser mode
        if mode == AgentMode::Browser {
            let mut browser_guard = self.browser_client.lock().await;
            if browser_guard.is_none() {
                println!("[agent] Connecting to browser...");
                match BrowserClient::connect().await {
                    Ok(client) => {
                        println!("[agent] Browser connected");
                        *browser_guard = Some(client);
                    }
                    Err(e) => {
                        let err_str = e.to_string();
                        if err_str.contains("CHROME_NEEDS_RESTART") {
                            // emit event to ask user if they want to restart chrome
                            println!("[agent] Chrome needs restart, asking user...");
                            let _ = app_handle.emit("browser:needs-restart", ());

                            // wait for user response via a oneshot channel
                            // for now, just try to restart automatically
                            match crate::browser::restart_chrome_with_debugging().await {
                                Ok(client) => {
                                    println!("[agent] Chrome restarted and connected");
                                    *browser_guard = Some(client);
                                }
                                Err(restart_err) => {
                                    println!("[agent] Chrome restart failed: {}", restart_err);
                                    self.emit(&app_handle, "error", "Chrome restart failed. Please manually quit Chrome and restart with: open -a 'Google Chrome' --args --remote-debugging-port=9222", None, None);
                                    self.running.store(false, Ordering::SeqCst);
                                    return Err(AgentError::Browser(restart_err));
                                }
                            }
                        } else {
                            println!("[agent] Browser connection failed: {}", e);
                            self.emit(&app_handle, "error", &format!("Browser connection failed: {}", e), None, None);
                            self.running.store(false, Ordering::SeqCst);
                            return Err(AgentError::Browser(e));
                        }
                    }
                }
            }
        }

        let client = AnthropicClient::new(api_key, model.clone());
        let mut messages: Vec<Message> = Vec::new();

        // load existing conversation or create new one
        let mode_str = match mode {
            AgentMode::Computer => "computer",
            AgentMode::Browser => "browser",
        };
        let mut conversation = if let Some(ref conv_id) = conversation_id {
            // try to load existing conversation
            match storage::load_conversation(conv_id) {
                Ok(Some(conv)) => {
                    println!("[agent] Loaded existing conversation: {}", conv_id);
                    conv
                }
                Ok(None) => {
                    println!("[agent] Conversation {} not found, creating new", conv_id);
                    Conversation::new(
                        uuid::Uuid::new_v4().to_string(),
                        "New Conversation".to_string(),
                        model.clone(),
                        mode_str.to_string(),
                    )
                }
                Err(e) => {
                    println!("[agent] Failed to load conversation {}: {}, creating new", conv_id, e);
                    Conversation::new(
                        uuid::Uuid::new_v4().to_string(),
                        "New Conversation".to_string(),
                        model.clone(),
                        mode_str.to_string(),
                    )
                }
            }
        } else {
            Conversation::new(
                uuid::Uuid::new_v4().to_string(),
                "New Conversation".to_string(),
                model.clone(),
                mode_str.to_string(),
            )
        };

        // effective voice_mode: use frontend value OR persisted conversation value
        let effective_voice_mode = voice_mode || conversation.voice_mode;
        // update conversation if voice mode changed
        if effective_voice_mode != conversation.voice_mode {
            conversation.voice_mode = effective_voice_mode;
        }

        // emit conversation id and voice_mode to frontend
        let _ = app_handle.emit("agent:conversation_id", &conversation.id);
        let _ = app_handle.emit("agent:voice_mode", effective_voice_mode);

        // init TTS client for voice mode
        let tts_client: Option<TtsClient> = if effective_voice_mode {
            match create_tts_client() {
                Some(tts) => {
                    println!("[agent] TTS client initialized for voice mode");
                    Some(tts)
                }
                None => {
                    println!("[agent] Voice mode requested but ELEVENLABS_API_KEY not set");
                    None
                }
            }
        } else {
            None
        };

        // emit started to all windows with mode
        self.emit_full(&app_handle, "started", "Agent started", None, None, None, Some(mode_str.to_string()));
        let _ = app_handle.emit("agent:started", ());

        // emit border show for frontend to call IPC command
        let _ = app_handle.emit("border:show", ());

        // small delay to ensure spotlight window event listeners are ready
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // emit user message so all windows can display it
        let _ = app_handle.emit("agent-update", AgentUpdate {
            update_type: "user_message".to_string(),
            message: instructions.clone(),
            tool_name: None,
            tool_input: None,
            action: None,
            screenshot: context_screenshot.clone(),
            bash_command: None,
            exit_code: None,
            mode: None,
        });
        println!("[agent] Emitted started + user_message events");

        // load history: prefer DB conversation (has full tool_use/tool_result),
        // fall back to frontend history for new conversations
        if !conversation.messages.is_empty() {
            // resuming existing conversation - use DB messages which include tool blocks
            println!("[agent] Using {} messages from DB conversation", conversation.messages.len());
            messages = conversation.messages.clone();
        } else {
            // new conversation - use frontend history (lossy but ok for first message)
            for msg in history {
                messages.push(Message {
                    role: msg.role,
                    content: vec![ContentBlock::Text { text: msg.content }],
                });
            }
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

        // add text instructions - wrap in voice_input tags if voice mode
        let text_content = if effective_voice_mode {
            format!("<voice_input>{}</voice_input>", instructions)
        } else {
            instructions.clone()
        };
        user_content.push(ContentBlock::Text {
            text: text_content,
        });

        let user_message = Message {
            role: "user".to_string(),
            content: user_content,
        };
        messages.push(user_message.clone());
        conversation.add_message(user_message);

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

            let api_result = match client.send_message_streaming(messages.clone(), event_tx, mode, effective_voice_mode).await {
                Ok(result) => {
                    println!("[agent] API streaming response complete, {} blocks, usage: {:?}", result.content.len(), result.usage);
                    result
                }
                Err(e) => {
                    println!("[agent] API error: {:?}", e);
                    self.emit(&app_handle, "error", &e.to_string(), None, None);
                    break;
                }
            };
            let response_content = api_result.content;

            // wait for stream consumer to finish
            let _ = stream_task.await;

            // add assistant response to history
            let assistant_message = Message {
                role: "assistant".to_string(),
                content: response_content.clone(),
            };
            messages.push(assistant_message.clone());
            conversation.add_message(assistant_message);
            conversation.add_usage(api_result.usage.clone(), &model);

            let mut tool_results: Vec<ContentBlock> = Vec::new();

            // debug: print all block types received
            let block_types: Vec<&str> = response_content.iter().map(|b| match b {
                ContentBlock::Text { .. } => "text",
                ContentBlock::Thinking { .. } => "thinking",
                ContentBlock::ToolUse { name, .. } => name.as_str(),
                ContentBlock::ToolResult { .. } => "tool_result",
                ContentBlock::Image { .. } => "image",
                ContentBlock::RedactedThinking { .. } => "redacted_thinking",
            }).collect();
            println!("[agent] Response blocks: {:?}", block_types);

            for block in &response_content {
                if !self.running.load(Ordering::SeqCst) {
                    break;
                }

                println!("[agent] Processing block: {:?}", block);

                match block {
                    ContentBlock::Thinking { thinking, .. } => {
                        println!("[agent] Thinking ({} chars): {}...", thinking.len(), &thinking[..thinking.len().min(300)]);
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

                            // emit tool for TS-side formatting
                            self.emit_tool(&app_handle, "computer", input.clone());
                            // emit globally for mini
                            match app_handle.emit("agent:action", serde_json::json!({
                                "action": action.action,
                                "text": action.text
                            })) {
                                Ok(_) => println!("[agent] agent:action emitted OK"),
                                Err(e) => println!("[agent] agent:action emit FAILED: {:?}", e),
                            }

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
                                    // zoom action returns screenshot directly, others need post-screenshot
                                    let screenshot = if action.action == "zoom" {
                                        // zoom returns the region screenshot, use panel exclusion
                                        if let Some(region) = action.region {
                                            #[cfg(target_os = "macos")]
                                            {
                                                crate::panels::take_screenshot_region_excluding_app(region)
                                                    .map_err(|e| AgentError::Computer(ComputerError::Screenshot(e)))?
                                            }
                                            #[cfg(not(target_os = "macos"))]
                                            {
                                                action_result.unwrap_or_else(|| {
                                                    let computer = ComputerControl::with_dimensions(screen_w, screen_h);
                                                    computer.take_screenshot_region(region).unwrap_or_default()
                                                })
                                            }
                                        } else {
                                            // no region, fallback to full screenshot
                                            #[cfg(target_os = "macos")]
                                            {
                                                crate::panels::take_screenshot_excluding_app()
                                                    .map_err(|e| AgentError::Computer(ComputerError::Screenshot(e)))?
                                            }
                                            #[cfg(not(target_os = "macos"))]
                                            {
                                                let computer_guard = self.computer.lock().await;
                                                let computer = computer_guard.as_ref().unwrap();
                                                computer.take_screenshot()?
                                            }
                                        }
                                    } else {
                                        // take screenshot excluding app windows
                                        // must run on main thread for Panel access on macOS
                                        #[cfg(target_os = "macos")]
                                        {
                                            crate::panels::take_screenshot_excluding_app()
                                                .map_err(|e| AgentError::Computer(ComputerError::Screenshot(e)))?
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
                        } else if name == "bash" {
                            let command = input.get("command").and_then(|v| v.as_str());
                            let restart = input.get("restart").and_then(|v| v.as_bool()).unwrap_or(false);

                            if restart {
                                let mut bash = self.bash.lock().await;
                                bash.restart();
                                self.emit_tool(&app_handle, "bash", serde_json::json!({"restart": true}));
                                tool_results.push(ContentBlock::ToolResult {
                                    tool_use_id: id.clone(),
                                    content: vec![ToolResultContent::Text {
                                        text: "Bash session restarted".to_string(),
                                    }],
                                });
                            } else if let Some(cmd) = command {
                                // emit tool for TS-side formatting
                                self.emit_tool(&app_handle, "bash", input.clone());
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
                        } else if is_browser_tool(name) && mode == AgentMode::Browser {
                            // handle browser tools
                            println!("[agent] Calling browser tool: {}", name);
                            // emit tool for TS-side formatting
                            self.emit_tool(&app_handle, name, input.clone());
                            let _ = app_handle.emit("agent:browser_tool", serde_json::json!({ "name": name }));

                            let mut browser_guard = self.browser_client.lock().await;
                            if let Some(ref mut browser) = *browser_guard {
                                // screenshot returns image, other tools return text
                                if name == "screenshot" {
                                    match browser.screenshot().await {
                                        Ok(base64_data) => {
                                            println!("[agent] Browser screenshot captured ({} bytes)", base64_data.len());
                                            self.emit(&app_handle, "screenshot", "Browser screenshot", None, Some(base64_data.clone()));
                                            tool_results.push(ContentBlock::ToolResult {
                                                tool_use_id: id.clone(),
                                                content: vec![ToolResultContent::Image {
                                                    source: ImageSource {
                                                        source_type: "base64".to_string(),
                                                        media_type: "image/jpeg".to_string(),
                                                        data: base64_data,
                                                    },
                                                }],
                                            });
                                        }
                                        Err(e) => {
                                            let err_msg = format!("Screenshot error: {}", e);
                                            println!("[agent] Browser screenshot failed: {}", err_msg);
                                            self.emit(&app_handle, "browser_result", &err_msg, None, None);
                                            tool_results.push(ContentBlock::ToolResult {
                                                tool_use_id: id.clone(),
                                                content: vec![ToolResultContent::Text { text: err_msg }],
                                            });
                                        }
                                    }
                                } else {
                                    let result = execute_browser_tool(browser, name, input).await;
                                    match result {
                                        Ok(output) => {
                                            println!("[agent] Browser tool success ({} chars): {}...", output.len(), &output[..output.len().min(200)]);
                                            self.emit(&app_handle, "browser_result", &output, None, None);
                                            tool_results.push(ContentBlock::ToolResult {
                                                tool_use_id: id.clone(),
                                                content: vec![ToolResultContent::Text { text: output }],
                                            });
                                        }
                                        Err(e) => {
                                            let err_msg = format!("Browser error: {}", e);
                                            println!("[agent] Browser tool failed: {}", err_msg);
                                            // emit browser_result not error - tool failures are expected
                                            // (e.g. wait_for timeouts) and the model handles them gracefully
                                            self.emit(&app_handle, "browser_result", &err_msg, None, None);
                                            tool_results.push(ContentBlock::ToolResult {
                                                tool_use_id: id.clone(),
                                                content: vec![ToolResultContent::Text { text: err_msg }],
                                            });
                                        }
                                    }
                                }
                            } else {
                                let err_msg = "Browser not connected".to_string();
                                tool_results.push(ContentBlock::ToolResult {
                                    tool_use_id: id.clone(),
                                    content: vec![ToolResultContent::Text { text: err_msg }],
                                });
                            }
                        } else if name == "speak" {
                            // handle speak tool for voice mode
                            if let Some(text) = input.get("text").and_then(|t| t.as_str()) {
                                if let Some(ref tts) = tts_client {
                                    match tts.synthesize(text).await {
                                        Ok(audio_base64) => {
                                            println!("[agent] TTS synthesized {} bytes", audio_base64.len());
                                            // emit audio to frontend for playback
                                            let _ = app_handle.emit("agent:speak", serde_json::json!({
                                                "audio": audio_base64,
                                                "text": text,
                                            }));

                                            tool_results.push(ContentBlock::ToolResult {
                                                tool_use_id: id.clone(),
                                                content: vec![ToolResultContent::Text {
                                                    text: "Speech delivered.".to_string(),
                                                }],
                                            });
                                        }
                                        Err(e) => {
                                            let err_msg = format!("TTS error: {}", e);
                                            println!("[agent] TTS failed: {}", err_msg);
                                            tool_results.push(ContentBlock::ToolResult {
                                                tool_use_id: id.clone(),
                                                content: vec![ToolResultContent::Text { text: err_msg }],
                                            });
                                        }
                                    }
                                } else {
                                    tool_results.push(ContentBlock::ToolResult {
                                        tool_use_id: id.clone(),
                                        content: vec![ToolResultContent::Text {
                                            text: "TTS not available - ELEVENLABS_API_KEY not set".to_string(),
                                        }],
                                    });
                                }
                            }
                        } else {
                            // unknown tool - return error so API contract is satisfied
                            println!("[agent] Unknown tool called: {}", name);
                            tool_results.push(ContentBlock::ToolResult {
                                tool_use_id: id.clone(),
                                content: vec![ToolResultContent::Text {
                                    text: format!("Error: Unknown tool '{}'. Use the available tools: computer, bash, speak.", name),
                                }],
                            });
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

            // check if we're adding a new snapshot - if so, summarize old ones
            let has_new_snapshot = tool_results.iter().any(|r| {
                if let ContentBlock::ToolResult { content, .. } = r {
                    content.iter().any(|c| {
                        if let ToolResultContent::Text { text } = c {
                            text.starts_with("uid=")
                        } else {
                            false
                        }
                    })
                } else {
                    false
                }
            });

            if has_new_snapshot {
                summarize_old_snapshots(&mut messages);
            }

            let tool_result_message = Message {
                role: "user".to_string(),
                content: tool_results,
            };
            messages.push(tool_result_message.clone());
            conversation.add_message(tool_result_message);

            // save after each round so we don't lose progress on crash/stop
            conversation.auto_title();
            if let Err(e) = storage::save_conversation(&conversation) {
                println!("[agent] Failed to save conversation: {}", e);
            }
        }

        self.running.store(false, Ordering::SeqCst);

        // final save
        if !conversation.messages.is_empty() {
            if let Err(e) = storage::save_conversation(&conversation) {
                println!("[agent] Failed to save conversation: {}", e);
            } else {
                println!("[agent] Saved conversation {} ({} msgs, {} input, {} output tokens)",
                    conversation.id,
                    conversation.messages.len(),
                    conversation.total_input_tokens,
                    conversation.total_output_tokens
                );
            }
        }
        let _ = app_handle.emit("agent:stopped", ());

        // emit border hide for frontend to call IPC command
        let _ = app_handle.emit("border:hide", ());

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
        self.emit_full(app_handle, update_type, message, action, screenshot, exit_code, None);
    }

    fn emit_full(
        &self,
        app_handle: &AppHandle,
        update_type: &str,
        message: &str,
        action: Option<serde_json::Value>,
        screenshot: Option<String>,
        exit_code: Option<i32>,
        mode: Option<String>,
    ) {
        let payload = AgentUpdate {
            update_type: update_type.to_string(),
            message: message.to_string(),
            tool_name: None,
            tool_input: None,
            action,
            screenshot,
            bash_command: None,
            exit_code,
            mode,
        };
        // emit globally so both main and spotlight windows receive events
        match app_handle.emit("agent-update", payload) {
            Ok(_) => println!("[agent] Emit success: {}", update_type),
            Err(e) => println!("[agent] Emit FAILED: {} - {:?}", update_type, e),
        }
    }

    // emit tool action with tool name and input for TS-side formatting
    fn emit_tool(
        &self,
        app_handle: &AppHandle,
        tool_name: &str,
        tool_input: serde_json::Value,
    ) {
        let payload = AgentUpdate {
            update_type: "tool".to_string(),
            message: String::new(),
            tool_name: Some(tool_name.to_string()),
            tool_input: Some(tool_input.clone()),
            action: Some(tool_input), // backwards compat
            screenshot: None,
            bash_command: None,
            exit_code: None,
            mode: None,
        };
        match app_handle.emit("agent-update", payload) {
            Ok(_) => println!("[agent] Emit tool: {}", tool_name),
            Err(e) => println!("[agent] Emit tool FAILED: {} - {:?}", tool_name, e),
        }
    }
}

const BROWSER_TOOLS: &[&str] = &[
    "take_snapshot",
    "click",
    "hover",
    "fill",
    "fill_form",
    "drag",
    "press_key",
    "navigate_page",
    "wait_for",
    "new_page",
    "list_pages",
    "select_page",
    "close_page",
    "handle_dialog",
    "screenshot",
];

fn is_browser_tool(name: &str) -> bool {
    BROWSER_TOOLS.contains(&name)
}

async fn execute_browser_tool(
    browser: &mut BrowserClient,
    name: &str,
    input: &serde_json::Value,
) -> anyhow::Result<String> {
    match name {
        "take_snapshot" => {
            let verbose = input.get("verbose").and_then(|v| v.as_bool()).unwrap_or(false);
            browser.take_snapshot(verbose).await
        }
        "click" => {
            let uid = input.get("uid").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("uid required"))?;
            let dbl_click = input.get("dblClick").and_then(|v| v.as_bool()).unwrap_or(false);
            browser.click(uid, dbl_click).await
        }
        "hover" => {
            let uid = input.get("uid").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("uid required"))?;
            browser.hover(uid).await
        }
        "fill" => {
            let uid = input.get("uid").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("uid required"))?;
            let value = input.get("value").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("value required"))?;
            browser.fill(uid, value).await
        }
        "press_key" => {
            let key = input.get("key").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("key required"))?;
            browser.press_key(key).await
        }
        "navigate_page" => {
            let nav_type = input.get("type").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("type required"))?;
            let url = input.get("url").and_then(|v| v.as_str());
            let ignore_cache = input.get("ignoreCache").and_then(|v| v.as_bool()).unwrap_or(false);
            browser.navigate_page(nav_type, url, ignore_cache).await
        }
        "wait_for" => {
            let text = input.get("text").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("text required"))?;
            let timeout = input.get("timeout").and_then(|v| v.as_u64()).unwrap_or(5000);
            browser.wait_for(text, timeout).await
        }
        "new_page" => {
            let url = input.get("url").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("url required"))?;
            browser.new_page(url).await
        }
        "list_pages" => {
            browser.list_pages().await
        }
        "select_page" => {
            let page_idx = input.get("pageIdx").and_then(|v| v.as_u64())
                .ok_or_else(|| anyhow::anyhow!("pageIdx required"))? as usize;
            let bring_to_front = input.get("bringToFront").and_then(|v| v.as_bool()).unwrap_or(false);
            browser.select_page(page_idx, bring_to_front).await
        }
        "close_page" => {
            let page_idx = input.get("pageIdx").and_then(|v| v.as_u64())
                .ok_or_else(|| anyhow::anyhow!("pageIdx required"))? as usize;
            browser.close_page(page_idx).await
        }
        "drag" => {
            let from_uid = input.get("from_uid").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("from_uid required"))?;
            let to_uid = input.get("to_uid").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("to_uid required"))?;
            browser.drag(from_uid, to_uid).await
        }
        "fill_form" => {
            let elements = input.get("elements").and_then(|v| v.as_array())
                .ok_or_else(|| anyhow::anyhow!("elements array required"))?;
            let pairs: Vec<(String, String)> = elements.iter().filter_map(|el| {
                let uid = el.get("uid").and_then(|v| v.as_str())?;
                let value = el.get("value").and_then(|v| v.as_str())?;
                Some((uid.to_string(), value.to_string()))
            }).collect();
            browser.fill_form(&pairs).await
        }
        "handle_dialog" => {
            let action = input.get("action").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("action required (accept/dismiss)"))?;
            let accept = action == "accept";
            let prompt_text = input.get("promptText").and_then(|v| v.as_str());
            browser.handle_dialog(accept, prompt_text).await
        }
        _ => Err(anyhow::anyhow!("unknown browser tool: {}", name)),
    }
}

// summarize old snapshots to reduce context size
// keeps only interactive elements (links, buttons, inputs, headings)
fn summarize_old_snapshots(messages: &mut Vec<Message>) {
    for message in messages.iter_mut() {
        if message.role != "user" {
            continue;
        }

        for block in message.content.iter_mut() {
            if let ContentBlock::ToolResult { content, .. } = block {
                for item in content.iter_mut() {
                    if let ToolResultContent::Text { text } = item {
                        // check if it's a snapshot (starts with uid=)
                        if text.starts_with("uid=") && text.len() > 5000 {
                            *text = summarize_snapshot(text);
                        }
                    }
                }
            }
        }
    }
}

fn summarize_snapshot(snapshot: &str) -> String {
    // keep only lines with interactive roles
    let interactive_roles = [
        "link", "button", "textbox", "checkbox", "radio", "combobox",
        "searchbox", "slider", "switch", "menuitem", "tab", "heading",
        "WebArea", // keep the root
    ];

    let mut summary_lines: Vec<&str> = Vec::new();
    let mut kept_count = 0;
    let mut total_count = 0;

    for line in snapshot.lines() {
        total_count += 1;
        let trimmed = line.trim();

        // keep line if it contains any interactive role
        let should_keep = interactive_roles.iter().any(|role| {
            // match "uid=X_Y role" pattern
            trimmed.contains(&format!(" {} ", role)) ||
            trimmed.contains(&format!(" {} \"", role)) ||
            trimmed.ends_with(&format!(" {}", role))
        });

        if should_keep {
            summary_lines.push(line);
            kept_count += 1;
        }
    }

    let header = format!(
        "[snapshot summarized: {} interactive elements from {} total]\n",
        kept_count, total_count
    );

    header + &summary_lines.join("\n")
}
