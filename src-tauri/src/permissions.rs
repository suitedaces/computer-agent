use serde::{Deserialize, Serialize};

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(
        options: core_foundation::dictionary::CFDictionaryRef,
    ) -> bool;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum PermissionStatus {
    Granted,
    Denied,
    NotAsked,
    NotNeeded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionsCheck {
    pub accessibility: PermissionStatus,
    pub screen_recording: PermissionStatus,
    pub microphone: PermissionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserProfileStatus {
    pub exists: bool,
    pub path: String,
    pub sessions: Vec<String>, // domains with cookies
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyStatus {
    pub anthropic: bool,
    pub deepgram: bool,
}

// check all permissions
#[tauri::command]
pub fn check_permissions() -> PermissionsCheck {
    #[cfg(target_os = "macos")]
    {
        PermissionsCheck {
            accessibility: check_accessibility(),
            screen_recording: check_screen_recording(),
            microphone: check_microphone(),
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        PermissionsCheck {
            accessibility: PermissionStatus::NotNeeded,
            screen_recording: PermissionStatus::NotNeeded,
            microphone: PermissionStatus::NotNeeded,
        }
    }
}

#[cfg(target_os = "macos")]
fn check_accessibility() -> PermissionStatus {
    if unsafe { AXIsProcessTrusted() } {
        PermissionStatus::Granted
    } else {
        PermissionStatus::Denied
    }
}

#[cfg(target_os = "macos")]
fn check_screen_recording() -> PermissionStatus {
    // try to capture a 1x1 region - if it fails, we don't have permission
    use core_graphics::display::{CGPoint, CGRect, CGSize};
    use core_graphics::window::{
        kCGWindowListOptionOnScreenOnly, CGWindowListCreateImage,
    };

    let rect = CGRect::new(&CGPoint::new(0.0, 0.0), &CGSize::new(1.0, 1.0));

    let image = unsafe {
        CGWindowListCreateImage(
            rect,
            kCGWindowListOptionOnScreenOnly,
            0,
            0,
        )
    };

    if image.is_null() {
        PermissionStatus::Denied
    } else {
        PermissionStatus::Granted
    }
}

#[cfg(target_os = "macos")]
fn check_microphone() -> PermissionStatus {
    // check AVCaptureDevice authorization status
    use std::process::Command;

    // use swift snippet to check - simpler than objc bindings
    let output = Command::new("swift")
        .args([
            "-e",
            r#"
            import AVFoundation
            let status = AVCaptureDevice.authorizationStatus(for: .audio)
            switch status {
            case .authorized: print("granted")
            case .denied, .restricted: print("denied")
            case .notDetermined: print("notasked")
            @unknown default: print("denied")
            }
            "#,
        ])
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if stdout.contains("granted") {
                PermissionStatus::Granted
            } else if stdout.contains("notasked") {
                PermissionStatus::NotAsked
            } else {
                PermissionStatus::Denied
            }
        }
        Err(_) => PermissionStatus::Denied,
    }
}

// request permission (triggers system prompt)
#[tauri::command]
pub fn request_permission(permission: String) {
    #[cfg(target_os = "macos")]
    {
        match permission.as_str() {
            "accessibility" => request_accessibility(),
            "screenRecording" => request_screen_recording(),
            "microphone" => request_microphone(),
            _ => {}
        }
    }
}

#[cfg(target_os = "macos")]
fn request_accessibility() {
    use core_foundation::base::TCFType;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;

    let prompt_key = CFString::new("AXTrustedCheckOptionPrompt");
    let prompt_value = CFBoolean::true_value();

    let options =
        CFDictionary::from_CFType_pairs(&[(prompt_key.as_CFType(), prompt_value.as_CFType())]);

    unsafe {
        AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef());
    }
}

#[cfg(target_os = "macos")]
fn request_screen_recording() {
    // trigger screen recording prompt by attempting capture
    use core_graphics::display::{CGPoint, CGRect, CGSize};
    use core_graphics::window::{
        kCGWindowListOptionOnScreenOnly, CGWindowListCreateImage,
    };

    let rect = CGRect::new(&CGPoint::new(0.0, 0.0), &CGSize::new(1.0, 1.0));
    unsafe {
        CGWindowListCreateImage(rect, kCGWindowListOptionOnScreenOnly, 0, 0);
    }
}

#[cfg(target_os = "macos")]
fn request_microphone() {
    // request mic access via swift
    let _ = std::process::Command::new("swift")
        .args([
            "-e",
            r#"
            import AVFoundation
            AVCaptureDevice.requestAccess(for: .audio) { _ in }
            "#,
        ])
        .spawn();
}

// open system settings to permission pane
#[tauri::command]
pub fn open_permission_settings(permission: String) {
    #[cfg(target_os = "macos")]
    {
        let url = match permission.as_str() {
            "accessibility" => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
            }
            "screenRecording" => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"
            }
            "microphone" => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"
            }
            _ => return,
        };

        let _ = std::process::Command::new("open").arg(url).spawn();
    }
}

// check browser profile status
#[tauri::command]
pub fn get_browser_profile_status() -> BrowserProfileStatus {
    let home = std::env::var("HOME").unwrap_or_else(|_| String::from("/Users"));
    let profile_path = format!("{}/.taskhomie-chrome", home);
    let path = std::path::Path::new(&profile_path);

    if !path.exists() {
        return BrowserProfileStatus {
            exists: false,
            path: profile_path,
            sessions: vec![],
        };
    }

    // read cookies db to get domains with sessions
    let cookies_db = path.join("Default/Cookies");
    let sessions = if cookies_db.exists() {
        read_cookie_domains(&cookies_db).unwrap_or_default()
    } else {
        vec![]
    };

    BrowserProfileStatus {
        exists: true,
        path: profile_path,
        sessions,
    }
}

fn read_cookie_domains(db_path: &std::path::Path) -> Result<Vec<String>, String> {
    // copy db to temp location (chrome locks it)
    let temp_path = std::env::temp_dir().join("taskhomie_cookies_copy.db");
    std::fs::copy(db_path, &temp_path).map_err(|e| e.to_string())?;

    let conn = rusqlite::Connection::open(&temp_path).map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare("SELECT DISTINCT host_key FROM cookies ORDER BY last_access_utc DESC")
        .map_err(|e| e.to_string())?;

    let domains: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .map(|d| d.trim_start_matches('.').to_string())
        .collect();

    // cleanup temp file
    let _ = std::fs::remove_file(&temp_path);

    // deduplicate
    let mut unique: Vec<String> = vec![];
    for d in domains {
        if !unique.contains(&d) {
            unique.push(d);
        }
    }

    Ok(unique)
}

// clear cookies for a specific domain
#[tauri::command]
pub fn clear_domain_cookies(domain: String) -> Result<(), String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| String::from("/Users"));
    let profile_path = format!("{}/.taskhomie-chrome", home);
    let cookies_db = std::path::Path::new(&profile_path).join("Default/Cookies");

    if !cookies_db.exists() {
        return Ok(());
    }

    // copy, modify, copy back
    let temp_path = std::env::temp_dir().join("taskhomie_cookies_edit.db");
    std::fs::copy(&cookies_db, &temp_path).map_err(|e| e.to_string())?;

    let conn = rusqlite::Connection::open(&temp_path).map_err(|e| e.to_string())?;

    // delete cookies matching domain (with or without leading dot)
    conn.execute(
        "DELETE FROM cookies WHERE host_key = ?1 OR host_key = ?2",
        [&domain, &format!(".{}", domain)],
    )
    .map_err(|e| e.to_string())?;

    drop(conn);

    // copy back
    std::fs::copy(&temp_path, &cookies_db).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&temp_path);

    Ok(())
}

// open browser profile in chrome for manual login
#[tauri::command]
pub fn open_browser_profile() -> Result<(), String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| String::from("/Users"));
    let profile_path = format!("{}/.taskhomie-chrome", home);

    // create profile dir if it doesn't exist
    let _ = std::fs::create_dir_all(&profile_path);

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args([
                "-a",
                "Google Chrome",
                "--args",
                &format!("--user-data-dir={}", profile_path),
                "--profile-directory=Default",
                "--no-first-run",
                "--no-default-browser-check",
            ])
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

// open browser profile with specific url
#[tauri::command]
pub fn open_browser_profile_url(url: String) -> Result<(), String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| String::from("/Users"));
    let profile_path = format!("{}/.taskhomie-chrome", home);

    let _ = std::fs::create_dir_all(&profile_path);

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args([
                "-a",
                "Google Chrome",
                "--args",
                &format!("--user-data-dir={}", profile_path),
                "--profile-directory=Default",
                "--no-first-run",
                "--no-default-browser-check",
                &url,
            ])
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

// reset browser profile (delete it)
#[tauri::command]
pub fn reset_browser_profile() -> Result<(), String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| String::from("/Users"));
    let profile_path = format!("{}/.taskhomie-chrome", home);

    if std::path::Path::new(&profile_path).exists() {
        std::fs::remove_dir_all(&profile_path).map_err(|e| e.to_string())?;
    }

    Ok(())
}

// check which api keys are configured
#[tauri::command]
pub fn get_api_key_status() -> ApiKeyStatus {
    ApiKeyStatus {
        anthropic: std::env::var("ANTHROPIC_API_KEY").is_ok(),
        deepgram: std::env::var("DEEPGRAM_API_KEY").is_ok(),
    }
}

// save api key to .env file
#[tauri::command]
pub fn save_api_key(service: String, key: String) -> Result<(), String> {
    let env_path = std::env::current_dir()
        .map(|p| p.join(".env"))
        .unwrap_or_else(|_| std::path::PathBuf::from(".env"));

    // read existing content
    let existing = std::fs::read_to_string(&env_path).unwrap_or_default();

    let var_name = match service.as_str() {
        "anthropic" => "ANTHROPIC_API_KEY",
        "deepgram" => "DEEPGRAM_API_KEY",
        _ => return Err("Unknown service".to_string()),
    };

    // update or add the key
    let mut lines: Vec<String> = existing.lines().map(String::from).collect();
    let mut found = false;

    for line in &mut lines {
        if line.starts_with(&format!("{}=", var_name)) {
            *line = format!("{}={}", var_name, key);
            found = true;
            break;
        }
    }

    if !found {
        lines.push(format!("{}={}", var_name, key));
    }

    std::fs::write(&env_path, lines.join("\n")).map_err(|e| e.to_string())?;

    // also set in current process
    std::env::set_var(var_name, &key);

    Ok(())
}
