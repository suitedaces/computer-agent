#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agent;
mod api;
mod computer;

use agent::Agent;
use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{
    tray::TrayIconBuilder,
    Manager, State,
};
use tauri_plugin_positioner::{Position, WindowExt};

#[cfg(target_os = "macos")]
use tauri_nspanel::{
    tauri_panel, CollectionBehavior, ManagerExt, PanelLevel, StyleMask, WebviewWindowExt,
};

#[cfg(target_os = "macos")]
tauri_panel! {
    panel!(GruntyPanel {
        config: {
            can_become_key_window: true,
            is_floating_panel: true
        }
    })
}

struct AppState {
    agent: Arc<Mutex<Agent>>,
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
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    println!("[grunty] run_agent called with: {}", instructions);

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
        match agent_guard.run(instructions, app_handle).await {
            Ok(_) => println!("[grunty] Agent finished"),
            Err(e) => println!("[grunty] Agent error: {:?}", e),
        }
    });

    Ok(())
}

#[tauri::command]
async fn stop_agent(state: State<'_, AppState>) -> Result<(), String> {
    let agent = state.agent.lock().await;
    agent.stop();
    Ok(())
}

#[tauri::command]
async fn is_agent_running(state: State<'_, AppState>) -> Result<bool, String> {
    let agent = state.agent.lock().await;
    Ok(agent.is_running())
}

#[tauri::command]
fn debug_log(message: String) {
    println!("[frontend] {}", message);
}

fn main() {
    // load .env
    if dotenvy::dotenv().is_err() {
        let _ = dotenvy::from_filename("../.env");
    }

    let mut agent = Agent::new();

    if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
        println!("[grunty] API key loaded");
        agent.set_api_key(key);
    }

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
        })
        .setup(|app| {
            // hide from dock - menubar app only
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // convert window to panel for fullscreen support
            #[cfg(target_os = "macos")]
            if let Some(window) = app.get_webview_window("main") {
                if let Ok(panel) = window.to_panel::<GruntyPanel>() {
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

            // create tray icon
            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .icon_as_template(true)
                .on_tray_icon_event(|tray, event| {
                    // update tray position for positioner plugin
                    tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);

                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        #[cfg(target_os = "macos")]
                        if let Ok(panel) = app.get_webview_panel("main") {
                            if panel.is_visible() {
                                panel.hide();
                            } else {
                                if let Some(window) = app.get_webview_window("main") {
                                    let _ = window.move_window(Position::TrayBottomCenter);
                                }
                                panel.show_and_make_key();
                            }
                        }
                        #[cfg(not(target_os = "macos"))]
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.move_window(Position::TrayBottomCenter);
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
