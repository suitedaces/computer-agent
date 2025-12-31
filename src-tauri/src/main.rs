#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

mod agent;
mod api;
mod bash;
mod browser;
mod computer;
mod panels;
mod permissions;
mod storage;
mod voice;

use agent::{Agent, AgentMode, HistoryMessage};
use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager, PhysicalPosition, State,
};
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutState};

#[cfg(target_os = "macos")]
use tauri_nspanel::{
    tauri_panel, CollectionBehavior, ManagerExt, PanelLevel, StyleMask, WebviewWindowExt,
};

#[cfg(target_os = "macos")]
tauri_panel! {
    panel!(TaskhomiePanel {
        config: {
            can_become_key_window: true,
            is_floating_panel: true
        }
    })
}

struct AppState {
    agent: Arc<Mutex<Agent>>,
    running: Arc<std::sync::atomic::AtomicBool>,
}

// cached screen info for fast window positioning
#[cfg(target_os = "macos")]
struct ScreenInfo {
    width: f64,
    height: f64,
    menubar_height: f64,
    scale: f64,
}

#[cfg(target_os = "macos")]
static SCREEN_INFO: std::sync::OnceLock<ScreenInfo> = std::sync::OnceLock::new();

// re-export panel handles from shared module
#[cfg(target_os = "macos")]
use panels::{MAIN_PANEL, VOICE_PANEL, BORDER_PANEL};

#[cfg(target_os = "macos")]
fn get_screen_info() -> &'static ScreenInfo {
    SCREEN_INFO.get_or_init(|| {
        use objc2_app_kit::NSScreen;
        use objc2_foundation::MainThreadMarker;

        if let Some(mtm) = MainThreadMarker::new() {
            if let Some(screen) = NSScreen::mainScreen(mtm) {
                let frame = screen.frame();
                let visible = screen.visibleFrame();
                let menubar_height = frame.size.height - visible.size.height - visible.origin.y;
                let scale = screen.backingScaleFactor();
                return ScreenInfo {
                    width: frame.size.width,
                    height: frame.size.height,
                    menubar_height,
                    scale,
                };
            }
        }
        // fallback for retina mac
        ScreenInfo { width: 1440.0, height: 900.0, menubar_height: 25.0, scale: 2.0 }
    })
}

#[cfg(target_os = "macos")]
fn position_window_top_right(window: &tauri::WebviewWindow, width: f64, _height: f64) {
    let info = get_screen_info();
    let padding = 10.0;
    let x = (info.width - width - padding) * info.scale;
    let y = (info.menubar_height + padding) * info.scale;
    let _ = window.set_position(PhysicalPosition::new(x as i32, y as i32));
}

#[cfg(target_os = "macos")]
fn position_window_center(window: &tauri::WebviewWindow, width: f64, height: f64) {
    let info = get_screen_info();
    let x = ((info.width - width) / 2.0) * info.scale;
    let y = ((info.height - height) / 2.0) * info.scale;
    let _ = window.set_position(PhysicalPosition::new(x as i32, y as i32));
}

#[tauri::command]
async fn set_api_key(api_key: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut agent = state.agent.lock().await;
    agent.set_api_key(api_key);
    Ok(())
}

#[tauri::command]
async fn check_api_key(state: State<'_, AppState>) -> Result<bool, String> {
    let agent = state.agent.lock().await;
    Ok(agent.has_api_key())
}

#[tauri::command(rename_all = "camelCase")]
async fn run_agent(
    instructions: String,
    model: String,
    mode: AgentMode,
    voice_mode: Option<bool>,
    history: Vec<HistoryMessage>,
    context_screenshot: Option<String>,
    conversation_id: Option<String>,
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let voice = voice_mode.unwrap_or(false);
    println!("[taskhomie] run_agent called with: {} (model: {}, mode: {:?}, voice: {}, history: {} msgs, screenshot: {}, conv: {:?})",
        instructions, model, mode, voice, history.len(), context_screenshot.is_some(), conversation_id);

    let agent = state.agent.clone();

    {
        let agent_guard = agent.lock().await;
        if agent_guard.is_running() {
            return Err("Agent is already running".to_string());
        }
        if !agent_guard.has_api_key() {
            return Err("No API key set. Please add ANTHROPIC_API_KEY to .env".to_string());
        }
    }

    tokio::spawn(async move {
        let agent_guard = agent.lock().await;
        match agent_guard.run(instructions, model, mode, voice, history, context_screenshot, conversation_id, app_handle).await {
            Ok(_) => println!("[taskhomie] Agent finished"),
            Err(e) => println!("[taskhomie] Agent error: {:?}", e),
        }
    });

    Ok(())
}

#[tauri::command]
fn stop_agent(state: State<'_, AppState>) -> Result<(), String> {
    state.running.store(false, std::sync::atomic::Ordering::SeqCst);
    println!("[taskhomie] Stop requested");
    Ok(())
}

#[tauri::command]
fn is_agent_running(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.running.load(std::sync::atomic::Ordering::SeqCst))
}

#[tauri::command]
fn debug_log(message: String) {
    println!("[frontend] {}", message);
}

// unified window state command - frontend tells backend what size/position it needs
#[tauri::command]
fn set_window_state(app_handle: tauri::AppHandle, width: f64, height: f64, centered: bool) -> Result<(), String> {
    println!("[window] set_window_state: {}x{}, centered={}", width, height, centered);
    #[cfg(target_os = "macos")]
    {
        if let Some(window) = app_handle.get_webview_window("main") {
            let _ = window.set_size(tauri::LogicalSize::new(width, height));
            if centered {
                position_window_center(&window, width, height);
            } else {
                position_window_top_right(&window, width, height);
            }
            if let Some(panel) = MAIN_PANEL.get() {
                panel.show();
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        if let Some(window) = app_handle.get_webview_window("main") {
            let _ = window.set_size(tauri::LogicalSize::new(width, height));
            let _ = window.show();
        }
    }
    Ok(())
}

// voice window controls
#[tauri::command]
fn show_voice_window(app_handle: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        if let Some(window) = app_handle.get_webview_window("voice") {
            position_window_center(&window, 300.0, 300.0);
        }
        if let Some(panel) = VOICE_PANEL.get() {
            panel.show();
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        if let Some(window) = app_handle.get_webview_window("voice") {
            let _ = window.center();
            let _ = window.show();
        }
    }
    Ok(())
}

#[tauri::command]
fn hide_voice_window(_app_handle: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    if let Some(panel) = VOICE_PANEL.get() {
        panel.hide();
    }
    #[cfg(not(target_os = "macos"))]
    if let Some(window) = _app_handle.get_webview_window("voice") {
        let _ = window.hide();
    }
    Ok(())
}

#[tauri::command]
fn hide_main_window(_app_handle: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    if let Some(panel) = MAIN_PANEL.get() {
        panel.hide();
    }
    #[cfg(not(target_os = "macos"))]
    if let Some(window) = _app_handle.get_webview_window("main") {
        let _ = window.hide();
    }
    Ok(())
}

// show main window in voice response mode and emit event
#[tauri::command]
fn show_main_voice_response(app_handle: tauri::AppHandle, text: String, screenshot: Option<String>, mode: String) -> Result<(), String> {
    // emit event to main window so it can switch to voice response mode
    let _ = app_handle.emit("voice:response", serde_json::json!({
        "text": text,
        "screenshot": screenshot,
        "mode": mode,
    }));

    // show main panel (frontend will handle sizing via set_window_state)
    #[cfg(target_os = "macos")]
    if let Some(panel) = MAIN_PANEL.get() {
        panel.show();
    }
    #[cfg(not(target_os = "macos"))]
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.show();
    }

    Ok(())
}

// set main panel click-through (ignores mouse events)
#[tauri::command]
fn set_main_click_through(ignore: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    if let Some(panel) = MAIN_PANEL.get() {
        panel.set_ignores_mouse_events(ignore);
    }
    Ok(())
}

#[tauri::command]
fn show_border_overlay() {
    #[cfg(target_os = "macos")]
    if let Some(panel) = BORDER_PANEL.get() {
        panel.show();
    }
}

#[tauri::command]
fn hide_border_overlay() {
    #[cfg(target_os = "macos")]
    if let Some(panel) = BORDER_PANEL.get() {
        panel.hide();
    }
}

// take screenshot excluding our app windows - uses shared panels module
#[tauri::command]
fn take_screenshot_excluding_app() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        panels::take_screenshot_excluding_app()
    }

    #[cfg(not(target_os = "macos"))]
    {
        let control = computer::ComputerControl::new().map_err(|e| e.to_string())?;
        control.take_screenshot().map_err(|e| e.to_string())
    }
}

// trigger screen flash effect - plays sound as feedback
#[cfg(target_os = "macos")]
fn trigger_screen_flash() {
    std::process::Command::new("afplay")
        .arg("/System/Library/Components/CoreAudio.component/Contents/SharedSupport/SystemSounds/system/Grab.aif")
        .spawn()
        .ok();
}

// hotkey triggered - capture screenshot and return base64
#[tauri::command]
fn capture_screen_for_help() -> Result<String, String> {
    let control = computer::ComputerControl::new().map_err(|e| e.to_string())?;
    let screenshot = control.take_screenshot().map_err(|e| e.to_string())?;

    #[cfg(target_os = "macos")]
    trigger_screen_flash();

    Ok(screenshot)
}

// --- storage IPC commands ---

mod storage_cmd {
    use crate::storage::{self, Conversation, ConversationMeta};

    #[tauri::command]
    pub fn list_conversations(limit: usize, offset: usize) -> Result<Vec<ConversationMeta>, String> {
        storage::list_conversations(limit, offset)
    }

    #[tauri::command]
    pub fn load_conversation(id: String) -> Result<Option<Conversation>, String> {
        storage::load_conversation(&id)
    }

    #[tauri::command]
    pub fn create_conversation(title: String, model: String, mode: String) -> Result<String, String> {
        storage::create_conversation(title, model, mode)
    }

    #[tauri::command]
    pub fn save_conversation(conv: Conversation) -> Result<(), String> {
        storage::save_conversation(&conv)
    }

    #[tauri::command]
    pub fn delete_conversation(id: String) -> Result<(), String> {
        storage::delete_conversation(&id)
    }

    #[tauri::command]
    pub fn search_conversations(query: String, limit: usize) -> Result<Vec<ConversationMeta>, String> {
        storage::search_conversations(&query, limit)
    }

    #[tauri::command(rename_all = "camelCase")]
    pub fn set_conversation_voice_mode(conversation_id: String, voice_mode: bool) -> Result<(), String> {
        storage::set_conversation_voice_mode(&conversation_id, voice_mode)
    }
}

// --- voice IPC commands ---

mod voice_cmd {
    use crate::voice::{VoiceSession, PushToTalkSession};
    use std::sync::Arc;
    use tauri::State;

    pub struct VoiceState {
        pub session: Arc<VoiceSession>,
    }

    pub struct PttState {
        pub session: Arc<PushToTalkSession>,
        pub screenshot: std::sync::Mutex<Option<String>>,
        pub mode: std::sync::Mutex<Option<String>>,
        pub current_session_id: std::sync::Mutex<u64>,
    }

    #[tauri::command]
    pub async fn start_voice(
        app_handle: tauri::AppHandle,
        state: State<'_, VoiceState>,
    ) -> Result<(), String> {
        println!("[voice cmd] start_voice called");
        let api_key = match std::env::var("DEEPGRAM_API_KEY") {
            Ok(key) => {
                println!("[voice cmd] got API key (len={})", key.len());
                key
            }
            Err(e) => {
                println!("[voice cmd] DEEPGRAM_API_KEY not found: {:?}", e);
                return Err("DEEPGRAM_API_KEY not set in .env".to_string());
            }
        };
        println!("[voice cmd] starting session...");
        let result = state.session.start(api_key, app_handle).await;
        println!("[voice cmd] session.start returned: {:?}", result);
        result
    }

    #[tauri::command]
    pub fn stop_voice(state: State<'_, VoiceState>) -> Result<(), String> {
        state.session.stop();
        Ok(())
    }

    #[tauri::command]
    pub fn is_voice_running(state: State<'_, VoiceState>) -> Result<bool, String> {
        Ok(state.session.is_running())
    }

    #[tauri::command]
    pub async fn start_ptt(
        app_handle: tauri::AppHandle,
        state: State<'_, PttState>,
        screenshot: Option<String>,
    ) -> Result<(), String> {
        println!("[ptt cmd] start_ptt called");

        if let Some(ss) = screenshot {
            *state.screenshot.lock().unwrap() = Some(ss);
        }

        let api_key = std::env::var("DEEPGRAM_API_KEY")
            .map_err(|_| "DEEPGRAM_API_KEY not set in .env".to_string())?;

        let session_id = state.session.start(api_key, app_handle).await?;
        *state.current_session_id.lock().unwrap() = session_id;
        Ok(())
    }

    #[tauri::command]
    pub async fn stop_ptt(
        state: State<'_, PttState>,
    ) -> Result<(String, Option<String>), String> {
        println!("[ptt cmd] stop_ptt called");
        let (text, _session_id) = state.session.stop().await;
        let screenshot = state.screenshot.lock().unwrap().take();
        println!("[ptt cmd] got text: '{}', screenshot: {}", text, screenshot.is_some());
        Ok((text, screenshot))
    }

    #[tauri::command]
    pub fn is_ptt_running(state: State<'_, PttState>) -> Result<bool, String> {
        Ok(state.session.is_running())
    }
}

fn main() {
    // load .env
    if dotenvy::dotenv().is_err() {
        let _ = dotenvy::from_filename("../.env");
    }

    // init storage
    if let Err(e) = storage::init_db() {
        eprintln!("[taskhomie] storage init failed: {}", e);
    }

    let running = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut agent = Agent::new(running.clone());

    if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
        println!("[taskhomie] API key loaded");
        agent.set_api_key(key);
    }

    let running_for_shortcut = running.clone();
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_shortcut(Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyH))
                .unwrap()
                .with_shortcut(Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyS))
                .unwrap()
                .with_shortcut(Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyQ))
                .unwrap()
                .with_shortcut(Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::Space))
                .unwrap()
                .with_shortcut(Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyC))
                .unwrap()
                .with_shortcut(Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyB))
                .unwrap()
                .with_handler(move |app, shortcut, event| {
                    // PTT shortcuts - Ctrl+Shift+C (computer), Ctrl+Shift+B (browser)
                    let ptt_mode: Option<&str> = if shortcut.matches(Modifiers::CONTROL | Modifiers::SHIFT, Code::KeyC) {
                        Some("computer")
                    } else if shortcut.matches(Modifiers::CONTROL | Modifiers::SHIFT, Code::KeyB) {
                        Some("browser")
                    } else {
                        None
                    };

                    if let Some(mode) = ptt_mode {
                        match event.state {
                            ShortcutState::Pressed => {
                                println!("[ptt] pressed - starting recording (mode: {})", mode);

                                // capture screenshot only for computer mode
                                let screenshot = if mode == "computer" {
                                    panels::take_screenshot_excluding_app_sync().ok()
                                } else {
                                    None
                                };

                                // play recording start sound
                                #[cfg(target_os = "macos")]
                                {
                                    std::process::Command::new("afplay")
                                        .arg("/System/Library/Sounds/Tink.aiff")
                                        .spawn()
                                        .ok();
                                }

                                // show voice window centered at 300x300
                            #[cfg(target_os = "macos")]
                            {
                                if let Some(window) = app.get_webview_window("voice") {
                                    let _ = window.set_size(tauri::LogicalSize::new(300.0, 300.0));
                                    let info = get_screen_info();
                                    let x = ((info.width - 300.0) / 2.0) * info.scale;
                                    let y = ((info.height - 300.0) / 2.0) * info.scale;
                                    let _ = window.set_position(PhysicalPosition::new(x as i32, y as i32));
                                }
                                if let Some(panel) = VOICE_PANEL.get() {
                                    panel.show();
                                }
                            }

                            // emit recording event with screenshot and mode
                            let _ = app.emit("ptt:recording", serde_json::json!({
                                "recording": true,
                                "screenshot": screenshot,
                                "mode": mode,
                                "sessionId": 0
                            }));

                                // start PTT recording via command
                                let app_clone = app.clone();
                                let screenshot_clone = screenshot.clone();
                                let mode_str = mode.to_string();
                                tauri::async_runtime::spawn(async move {
                                    if let Some(ptt_state) = app_clone.try_state::<voice_cmd::PttState>() {
                                        let api_key = match std::env::var("DEEPGRAM_API_KEY") {
                                            Ok(k) => k,
                                            Err(_) => {
                                                let _ = app_clone.emit("ptt:error", "DEEPGRAM_API_KEY not set");
                                                return;
                                            }
                                        };

                                        // store screenshot and mode
                                        if let Some(ss) = screenshot_clone {
                                            *ptt_state.screenshot.lock().unwrap() = Some(ss);
                                        }
                                        *ptt_state.mode.lock().unwrap() = Some(mode_str);

                                        match ptt_state.session.start(api_key, app_clone.clone()).await {
                                            Ok(session_id) => {
                                                *ptt_state.current_session_id.lock().unwrap() = session_id;
                                                // session started - first ptt:recording already emitted with mode
                                            }
                                            Err(e) => {
                                                println!("[ptt] start error: {}", e);
                                                let _ = app_clone.emit("ptt:error", e);
                                            }
                                        }
                                    }
                                });
                            }
                            ShortcutState::Released => {
                                println!("[ptt] released - stopping recording");

                                // play recording stop sound
                                #[cfg(target_os = "macos")]
                                {
                                    std::process::Command::new("afplay")
                                        .arg("/System/Library/Sounds/Pop.aiff")
                                        .spawn()
                                        .ok();
                                }

                                // stop recording and get result
                                let app_clone = app.clone();
                                tauri::async_runtime::spawn(async move {
                                    if let Some(ptt_state) = app_clone.try_state::<voice_cmd::PttState>() {
                                        let expected_session_id = *ptt_state.current_session_id.lock().unwrap();
                                        let (raw_text, result_session_id) = ptt_state.session.stop().await;
                                        let screenshot = ptt_state.screenshot.lock().unwrap().take();
                                        let mode = ptt_state.mode.lock().unwrap().take();

                                        if result_session_id != expected_session_id {
                                            println!("[ptt] stale result ignored: got session {} but expected {}", result_session_id, expected_session_id);
                                            return;
                                        }

                                        // rewrite transcription to clean up speech artifacts
                                        let text = if !raw_text.trim().is_empty() {
                                            match std::env::var("ANTHROPIC_API_KEY") {
                                                Ok(api_key) => {
                                                    println!("[ptt] rewriting transcription...");
                                                    match crate::api::rewrite_transcription(&api_key, &raw_text).await {
                                                        Ok(rewritten) => {
                                                            println!("[ptt] rewritten: '{}' -> '{}'", raw_text, rewritten);
                                                            rewritten
                                                        }
                                                        Err(e) => {
                                                            println!("[ptt] rewrite failed: {}, using raw", e);
                                                            raw_text
                                                        }
                                                    }
                                                }
                                                Err(_) => {
                                                    println!("[ptt] no ANTHROPIC_API_KEY, skipping rewrite");
                                                    raw_text
                                                }
                                            }
                                        } else {
                                            raw_text
                                        };

                                        println!("[ptt] result: text='{}', screenshot={}, mode={:?}, session={}", text, screenshot.is_some(), mode, result_session_id);

                                        // frontend handles voice window visibility via responding mode

                                        let _ = app_clone.emit("ptt:recording", serde_json::json!({
                                            "recording": false,
                                            "sessionId": result_session_id
                                        }));

                                        let _ = app_clone.emit("ptt:result", serde_json::json!({
                                            "text": text,
                                            "screenshot": screenshot,
                                            "mode": mode,
                                            "sessionId": result_session_id
                                        }));
                                    }
                                });
                            }
                        }
                        return;
                    }

                    // other shortcuts only on press
                    if event.state != ShortcutState::Pressed {
                        return;
                    }

                    // Cmd+Shift+H - help mode (screenshot + prompt)
                    if shortcut.matches(Modifiers::SUPER | Modifiers::SHIFT, Code::KeyH) {
                        let screenshot = panels::take_screenshot_excluding_app_sync().ok();

                        #[cfg(target_os = "macos")]
                        trigger_screen_flash();

                        let _ = app.emit("hotkey-help", serde_json::json!({ "screenshot": screenshot }));
                    }

                    // Cmd+Shift+Space - spotlight mode (show centered input)
                    if shortcut.matches(Modifiers::SUPER | Modifiers::SHIFT, Code::Space) {
                        println!("[taskhomie] Spotlight mode triggered");
                        let _ = app.emit("hotkey-spotlight", ());

                        #[cfg(target_os = "macos")]
                        if let Some(panel) = MAIN_PANEL.get() {
                            panel.show();
                        }
                    }

                    // Cmd+Shift+S - stop agent
                    if shortcut.matches(Modifiers::SUPER | Modifiers::SHIFT, Code::KeyS) {
                        if running_for_shortcut.load(std::sync::atomic::Ordering::SeqCst) {
                            running_for_shortcut.store(false, std::sync::atomic::Ordering::SeqCst);
                            println!("[taskhomie] Stop requested via shortcut");
                        }
                    }

                    // Cmd+Shift+Q - quit app
                    if shortcut.matches(Modifiers::SUPER | Modifiers::SHIFT, Code::KeyQ) {
                        println!("[taskhomie] Quit requested via shortcut");
                        app.exit(0);
                    }
                })
                .build(),
        );

    #[cfg(target_os = "macos")]
    {
        builder = builder.plugin(tauri_nspanel::init());
    }

    builder
        .manage(AppState {
            agent: Arc::new(Mutex::new(agent)),
            running,
        })
        .manage(voice_cmd::VoiceState {
            session: Arc::new(voice::VoiceSession::new()),
        })
        .manage(voice_cmd::PttState {
            session: Arc::new(voice::PushToTalkSession::new()),
            screenshot: std::sync::Mutex::new(None),
            mode: std::sync::Mutex::new(None),
            current_session_id: std::sync::Mutex::new(0),
        })
        .setup(|app| {
            // hide from dock - menubar app only
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            #[cfg(target_os = "macos")]
            {
                // main panel
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.set_position(PhysicalPosition::new(-1000, -1000));

                    if let Ok(panel) = window.to_panel::<TaskhomiePanel>() {
                        panel.set_level(PanelLevel::Floating.value());
                        panel.set_style_mask(StyleMask::empty().nonactivating_panel().into());
                        panel.set_collection_behavior(
                            CollectionBehavior::new()
                                .full_screen_auxiliary()
                                .can_join_all_spaces()
                                .stationary()
                                .into(),
                        );
                        panel.set_hides_on_deactivate(false);
                        let _ = MAIN_PANEL.set(panel);
                    }
                }

                // voice panel
                if let Some(window) = app.get_webview_window("voice") {
                    let _ = window.set_position(PhysicalPosition::new(-1000, -1000));

                    if let Ok(panel) = window.to_panel::<TaskhomiePanel>() {
                        panel.set_level(PanelLevel::Floating.value());
                        panel.set_style_mask(StyleMask::empty().nonactivating_panel().into());
                        panel.set_collection_behavior(
                            CollectionBehavior::new()
                                .full_screen_auxiliary()
                                .can_join_all_spaces()
                                .stationary()
                                .into(),
                        );
                        panel.set_hides_on_deactivate(false);
                        let _ = VOICE_PANEL.set(panel);
                    }
                }

                // border panel
                if let Some(window) = app.get_webview_window("border") {
                    let info = get_screen_info();
                    let _ = window.set_size(tauri::LogicalSize::new(info.width, info.height));
                    let _ = window.set_position(PhysicalPosition::new(0, 0));
                    println!("[taskhomie] Border panel sized to {}x{}", info.width, info.height);

                    if let Ok(panel) = window.to_panel::<TaskhomiePanel>() {
                        panel.set_level(PanelLevel::Floating.value());
                        panel.set_style_mask(StyleMask::empty().nonactivating_panel().into());
                        panel.set_collection_behavior(
                            CollectionBehavior::new()
                                .full_screen_auxiliary()
                                .can_join_all_spaces()
                                .stationary()
                                .into(),
                        );
                        panel.set_hides_on_deactivate(false);
                        panel.set_ignores_mouse_events(true);
                        let _ = BORDER_PANEL.set(panel);
                    }
                }

                // show main window at startup (idle size)
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.set_size(tauri::LogicalSize::new(280.0, 40.0));
                    position_window_top_right(&window, 280.0, 40.0);
                    if let Some(panel) = MAIN_PANEL.get() {
                        panel.show();
                    }
                }
            }

            // tray menu with quit option
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let tray_menu = Menu::with_items(app, &[&quit])?;

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .icon_as_template(false)
                .menu(&tray_menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| {
                    if event.id.as_ref() == "quit" {
                        app.exit(0);
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();

                        #[cfg(target_os = "macos")]
                        {
                            let main_visible = MAIN_PANEL.get().map(|p| p.is_visible()).unwrap_or(false);

                            if main_visible {
                                // hide main
                                if let Some(panel) = MAIN_PANEL.get() {
                                    panel.hide();
                                }
                            } else {
                                // show main at idle size
                                if let Some(window) = app.get_webview_window("main") {
                                    let _ = window.set_size(tauri::LogicalSize::new(280.0, 40.0));
                                    position_window_top_right(&window, 280.0, 40.0);
                                }
                                if let Some(panel) = MAIN_PANEL.get() {
                                    panel.show();
                                }
                            }
                        }
                        #[cfg(not(target_os = "macos"))]
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|_window, _event| {
            // disabled auto-hide for now
        })
        .invoke_handler(tauri::generate_handler![
            set_api_key,
            check_api_key,
            run_agent,
            stop_agent,
            is_agent_running,
            debug_log,
            set_window_state,
            show_voice_window,
            hide_voice_window,
            hide_main_window,
            show_main_voice_response,
            set_main_click_through,
            show_border_overlay,
            hide_border_overlay,
            take_screenshot_excluding_app,
            capture_screen_for_help,
            storage_cmd::list_conversations,
            storage_cmd::load_conversation,
            storage_cmd::create_conversation,
            storage_cmd::save_conversation,
            storage_cmd::delete_conversation,
            storage_cmd::search_conversations,
            storage_cmd::set_conversation_voice_mode,
            voice_cmd::start_voice,
            voice_cmd::stop_voice,
            voice_cmd::is_voice_running,
            voice_cmd::start_ptt,
            voice_cmd::stop_ptt,
            voice_cmd::is_ptt_running,
            permissions::check_permissions,
            permissions::request_permission,
            permissions::open_permission_settings,
            permissions::get_browser_profile_status,
            permissions::open_browser_profile,
            permissions::open_browser_profile_url,
            permissions::clear_domain_cookies,
            permissions::reset_browser_profile,
            permissions::get_api_key_status,
            permissions::save_api_key,
            permissions::get_voice_settings,
            permissions::save_voice_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
