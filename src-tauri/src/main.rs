#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

mod agent;
mod api;
mod bash;
mod computer;
mod mcp;

use agent::{Agent, HistoryMessage};
use mcp::{create_shared_client, SharedMcpClient};
use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{
    tray::TrayIconBuilder,
    Manager, State,
};
use tauri::PhysicalPosition;

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
    mcp_client: SharedMcpClient,
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

#[tauri::command]
async fn run_agent(
    instructions: String,
    model: String,
    history: Vec<HistoryMessage>,
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    println!("[taskhomie] run_agent called with: {} (model: {}, history: {} msgs)", instructions, model, history.len());

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
        match agent_guard.run(instructions, model, history, app_handle).await {
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

#[tauri::command]
async fn connect_mcp(
    command: String,
    args: Vec<String>,
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let mut client = state.mcp_client.write().await;
    client
        .connect(&command, &args_refs)
        .await
        .map_err(|e| e.to_string())?;
    Ok(client.get_tool_names())
}

#[tauri::command]
async fn disconnect_mcp(state: State<'_, AppState>) -> Result<(), String> {
    let mut client = state.mcp_client.write().await;
    client.disconnect().await;
    Ok(())
}

#[tauri::command]
async fn is_mcp_connected(state: State<'_, AppState>) -> Result<bool, String> {
    let client = state.mcp_client.read().await;
    Ok(client.is_connected())
}

#[tauri::command]
async fn get_mcp_tools(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let client = state.mcp_client.read().await;
    Ok(client.get_tool_names())
}

#[tauri::command]
fn show_mini_window(app_handle: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    if let Ok(panel) = app_handle.get_webview_panel("mini") {
        panel.show();
    }
    #[cfg(not(target_os = "macos"))]
    if let Some(window) = app_handle.get_webview_window("mini") {
        let _ = window.show();
    }
    Ok(())
}

#[tauri::command]
fn hide_mini_window(app_handle: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    if let Ok(panel) = app_handle.get_webview_panel("mini") {
        panel.hide();
    }
    #[cfg(not(target_os = "macos"))]
    if let Some(window) = app_handle.get_webview_window("mini") {
        let _ = window.hide();
    }
    Ok(())
}

fn main() {
    // load .env
    if dotenvy::dotenv().is_err() {
        let _ = dotenvy::from_filename("../.env");
    }

    let running = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mcp_client = create_shared_client();
    let mut agent = Agent::new(running.clone());
    agent.set_mcp_client(mcp_client.clone());

    if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
        println!("[taskhomie] API key loaded");
        agent.set_api_key(key);
    }

    // auto-connect to chrome-devtools-mcp on startup
    let mcp_client_clone = mcp_client.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let mut client = mcp_client_clone.write().await;
            match client.connect("npx", &["-y", "chrome-devtools-mcp@latest"]).await {
                Ok(()) => println!("[taskhomie] chrome-devtools-mcp connected"),
                Err(e) => println!("[taskhomie] chrome-devtools-mcp failed to connect: {}", e),
            }
        });
    });

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_positioner::init());

    #[cfg(target_os = "macos")]
    {
        builder = builder.plugin(tauri_nspanel::init());
    }

    builder
        .manage(AppState {
            agent: Arc::new(Mutex::new(agent)),
            running,
            mcp_client,
        })
        .setup(|app| {
            // hide from dock - menubar app only
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // convert windows to panels for fullscreen support
            #[cfg(target_os = "macos")]
            {
                // main panel
                if let Some(window) = app.get_webview_window("main") {
                    // pre-position offscreen to avoid flicker on first show
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
                    }
                }

                // mini panel - same setup as main for fullscreen support
                if let Some(window) = app.get_webview_window("mini") {
                    println!("[setup] mini window found");

                    // pre-position offscreen to avoid flicker on first show
                    let _ = window.set_position(PhysicalPosition::new(-1000, -1000));

                    // ensure mini window has ?mini=true in URL (for dev mode)
                    if let Ok(url) = window.url() {
                        println!("[setup] mini window url: {}", url);
                        if !url.to_string().contains("mini") {
                            let new_url = format!("{}?mini=true", url);
                            println!("[setup] navigating mini to: {}", new_url);
                            let _ = window.eval(&format!("window.location.href = '{}'", new_url));
                        }
                    }

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
                        println!("[setup] mini panel created");
                    }
                }
            }

            // create tray icon
            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .icon_as_template(false)
                .on_tray_icon_event(|tray, event| {
                    // update tray position for positioner plugin
                    tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);

                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Up,
                        rect,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        let state = app.state::<AppState>();
                        let is_running = state.running.load(std::sync::atomic::Ordering::SeqCst);

                        // calculate position centered below tray icon
                        let (tray_x, tray_bottom) = match (rect.position, rect.size) {
                            (tauri::Position::Physical(pos), tauri::Size::Physical(size)) => {
                                (pos.x, pos.y + size.height as i32)
                            }
                            (tauri::Position::Logical(pos), tauri::Size::Logical(size)) => {
                                (pos.x as i32, (pos.y + size.height) as i32)
                            }
                            _ => (0, 0),
                        };

                        #[cfg(target_os = "macos")]
                        {
                            println!("[tray] click, is_running={}", is_running);
                            if let Ok(main_panel) = app.get_webview_panel("main") {
                                if main_panel.is_visible() {
                                    println!("[tray] hiding main panel");
                                    main_panel.hide();
                                    // show mini if agent is running
                                    if is_running {
                                        println!("[tray] showing mini panel");
                                        if let Ok(mini_panel) = app.get_webview_panel("mini") {
                                            if let Some(mini_window) = app.get_webview_window("mini") {
                                                if let Ok(size) = mini_window.outer_size() {
                                                    let x = tray_x - (size.width as i32 / 2);
                                                    let _ = mini_window.set_position(PhysicalPosition::new(x, tray_bottom));
                                                }
                                            }
                                            mini_panel.show();
                                        }
                                    }
                                } else {
                                    // hide mini, show main
                                    println!("[tray] hiding mini, showing main");
                                    if let Ok(mini_panel) = app.get_webview_panel("mini") {
                                        mini_panel.hide();
                                    }
                                    if let Some(window) = app.get_webview_window("main") {
                                        if let Ok(size) = window.outer_size() {
                                            let x = tray_x - (size.width as i32 / 2);
                                            let _ = window.set_position(PhysicalPosition::new(x, tray_bottom));
                                        }
                                    }
                                    main_panel.show_and_make_key();
                                }
                            }
                        }
                        #[cfg(not(target_os = "macos"))]
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                                if is_running {
                                    if let Some(mini) = app.get_webview_window("mini") {
                                        let _ = mini.show();
                                    }
                                }
                            } else {
                                if let Some(mini) = app.get_webview_window("mini") {
                                    let _ = mini.hide();
                                }
                                if let Ok(size) = window.outer_size() {
                                    let x = tray_x - (size.width as i32 / 2);
                                    let _ = window.set_position(PhysicalPosition::new(x, tray_bottom));
                                }
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
            // disabled auto-hide for now - interferes with agent running
            // if let tauri::WindowEvent::Focused(false) = event {
            //     let _ = window.hide();
            // }
        })
        .invoke_handler(tauri::generate_handler![
            set_api_key,
            check_api_key,
            run_agent,
            stop_agent,
            is_agent_running,
            debug_log,
            connect_mcp,
            disconnect_mcp,
            is_mcp_connected,
            get_mcp_tools,
            show_mini_window,
            hide_mini_window,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
