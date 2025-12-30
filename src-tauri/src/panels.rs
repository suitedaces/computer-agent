// shared panel management module - provides main-thread-safe access to app panels

#[cfg(target_os = "macos")]
use tauri_nspanel::PanelHandle;

#[cfg(target_os = "macos")]
pub static MAIN_PANEL: std::sync::OnceLock<PanelHandle<tauri::Wry>> = std::sync::OnceLock::new();
#[cfg(target_os = "macos")]
pub static MINI_PANEL: std::sync::OnceLock<PanelHandle<tauri::Wry>> = std::sync::OnceLock::new();
#[cfg(target_os = "macos")]
pub static SPOTLIGHT_PANEL: std::sync::OnceLock<PanelHandle<tauri::Wry>> = std::sync::OnceLock::new();
#[cfg(target_os = "macos")]
pub static BORDER_PANEL: std::sync::OnceLock<PanelHandle<tauri::Wry>> = std::sync::OnceLock::new();

// core screenshot logic - must be called on main thread
#[cfg(target_os = "macos")]
fn take_screenshot_excluding_impl() -> Result<String, String> {
    use crate::computer::ComputerControl;

    let control = ComputerControl::new().map_err(|e| e.to_string())?;

    // hide border if visible
    let border_was_visible = BORDER_PANEL.get()
        .map(|p| {
            let vis = p.is_visible();
            if vis { p.hide(); }
            vis
        })
        .unwrap_or(false);

    // get topmost visible panel window ID for BelowWindow exclusion
    let topmost_id: Option<u32> = [
        SPOTLIGHT_PANEL.get(),
        MAIN_PANEL.get(),
        MINI_PANEL.get(),
    ].iter().find_map(|p| {
        p.and_then(|panel| {
            if panel.is_visible() {
                let ns_panel = panel.as_panel();
                Some(unsafe {
                    let num: isize = objc2::msg_send![ns_panel, windowNumber];
                    num as u32
                })
            } else {
                None
            }
        })
    });

    // small delay for window server to process hide
    if border_was_visible {
        std::thread::sleep(std::time::Duration::from_millis(30));
    }

    // take screenshot
    let screenshot = if let Some(wid) = topmost_id {
        control.take_screenshot_excluding(wid).map_err(|e| e.to_string())?
    } else {
        control.take_screenshot().map_err(|e| e.to_string())?
    };

    // restore border
    if border_was_visible {
        if let Some(panel) = BORDER_PANEL.get() {
            panel.show();
        }
    }

    Ok(screenshot)
}

// take screenshot excluding app windows - dispatches to main thread
// use from async/tokio contexts (agent loop)
#[cfg(target_os = "macos")]
pub fn take_screenshot_excluding_app() -> Result<String, String> {
    use dispatch::Queue;
    Queue::main().exec_sync(take_screenshot_excluding_impl)
}

// take screenshot excluding app windows - no dispatch, call when already on main thread
// use from shortcut handlers
#[cfg(target_os = "macos")]
pub fn take_screenshot_excluding_app_sync() -> Result<String, String> {
    take_screenshot_excluding_impl()
}

// zoom screenshot of region excluding app windows - dispatches to main thread for Panel access
#[cfg(target_os = "macos")]
pub fn take_screenshot_region_excluding_app(region: [i32; 4]) -> Result<String, String> {
    use dispatch::Queue;
    use crate::computer::ComputerControl;

    // dispatch to main thread for Panel access
    Queue::main().exec_sync(|| {
        let control = ComputerControl::new().map_err(|e| e.to_string())?;

        // hide border if visible
        let border_was_visible = BORDER_PANEL.get()
            .map(|p| {
                let vis = p.is_visible();
                if vis { p.hide(); }
                vis
            })
            .unwrap_or(false);

        // get topmost visible panel window ID for BelowWindow exclusion
        let topmost_id: Option<u32> = [
            SPOTLIGHT_PANEL.get(),
            MAIN_PANEL.get(),
            MINI_PANEL.get(),
        ].iter().find_map(|p| {
            p.and_then(|panel| {
                if panel.is_visible() {
                    let ns_panel = panel.as_panel();
                    Some(unsafe {
                        let num: isize = objc2::msg_send![ns_panel, windowNumber];
                        num as u32
                    })
                } else {
                    None
                }
            })
        });

        // small delay for window server to process hide
        if border_was_visible {
            std::thread::sleep(std::time::Duration::from_millis(30));
        }

        // take region screenshot
        let screenshot = if let Some(wid) = topmost_id {
            control.take_screenshot_region_excluding(region, wid).map_err(|e| e.to_string())?
        } else {
            control.take_screenshot_region(region).map_err(|e| e.to_string())?
        };

        // restore border
        if border_was_visible {
            if let Some(panel) = BORDER_PANEL.get() {
                panel.show();
            }
        }

        Ok(screenshot)
    })
}
