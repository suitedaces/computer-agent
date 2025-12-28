use enigo::{Enigo, Key, Keyboard, Mouse, Settings, Coordinate, Button, Direction};
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::DynamicImage;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use xcap::Monitor;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

#[cfg(target_os = "macos")]
use core_graphics::geometry::{CGRect, CGPoint, CGSize};
#[cfg(target_os = "macos")]
use core_graphics::window::{
    create_image, kCGWindowListOptionOnScreenBelowWindow,
    kCGWindowImageDefault, CGWindowID,
};

// jpeg quality (1-100) - lower = faster + smaller, 60 is good for screenshots
const JPEG_QUALITY: u8 = 60;

const AI_WIDTH: u32 = 1280;
const AI_HEIGHT: u32 = 800;

#[derive(Error, Debug)]
pub enum ComputerError {
    #[error("Input error: {0}")]
    Input(String),
    #[error("Screenshot error: {0}")]
    Screenshot(String),
    #[error("Unknown action: {0}")]
    UnknownAction(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputerAction {
    pub action: String,
    #[serde(default)]
    pub coordinate: Option<[i32; 2]>,
    #[serde(default)]
    pub start_coordinate: Option<[i32; 2]>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub scroll_direction: Option<String>,
    #[serde(default)]
    pub scroll_amount: Option<i32>,
}

pub struct ComputerControl {
    pub screen_width: u32,
    pub screen_height: u32,
}

impl ComputerControl {
    pub fn new() -> Result<Self, ComputerError> {
        let monitor = Monitor::all()
            .map_err(|e| ComputerError::Screenshot(e.to_string()))?
            .into_iter()
            .next()
            .ok_or_else(|| ComputerError::Screenshot("No monitor found".to_string()))?;

        Ok(Self {
            screen_width: monitor.width().map_err(|e| ComputerError::Screenshot(e.to_string()))?,
            screen_height: monitor.height().map_err(|e| ComputerError::Screenshot(e.to_string()))?,
        })
    }

    pub fn with_dimensions(screen_width: u32, screen_height: u32) -> Self {
        Self { screen_width, screen_height }
    }

    pub fn take_screenshot(&self) -> Result<String, ComputerError> {
        let monitor = Monitor::all()
            .map_err(|e| ComputerError::Screenshot(e.to_string()))?
            .into_iter()
            .next()
            .ok_or_else(|| ComputerError::Screenshot("No monitor found".to_string()))?;

        let image = monitor
            .capture_image()
            .map_err(|e| ComputerError::Screenshot(e.to_string()))?;

        // resize with Nearest filter (fastest) - good enough for AI
        let resized = DynamicImage::ImageRgba8(image)
            .resize_exact(AI_WIDTH, AI_HEIGHT, FilterType::Nearest);

        // encode jpeg with explicit quality control
        let rgb = resized.to_rgb8();
        let mut buffer = Vec::with_capacity(200_000); // pre-alloc ~200kb
        let mut encoder = JpegEncoder::new_with_quality(&mut buffer, JPEG_QUALITY);
        encoder.encode_image(&rgb)
            .map_err(|e| ComputerError::Screenshot(e.to_string()))?;

        Ok(BASE64.encode(&buffer))
    }

    #[cfg(target_os = "macos")]
    pub fn take_screenshot_excluding(&self, window_id: u32) -> Result<String, ComputerError> {
        // capture full screen but exclude windows at and above window_id
        let bounds = CGRect::new(
            &CGPoint::new(0.0, 0.0),
            &CGSize::new(self.screen_width as f64, self.screen_height as f64),
        );

        let cg_image = create_image(
            bounds,
            kCGWindowListOptionOnScreenBelowWindow,
            window_id as CGWindowID,
            kCGWindowImageDefault,
        ).ok_or_else(|| ComputerError::Screenshot("Failed to create CGImage".to_string()))?;

        let width = cg_image.width();
        let height = cg_image.height();
        let bytes_per_row = cg_image.bytes_per_row();
        let data = cg_image.data();
        let raw_data = data.bytes();

        // fast BGRA -> RGB conversion (skip alpha, swap B/R) using chunks
        // output is RGB for jpeg (no alpha needed)
        let mut rgb_data = Vec::with_capacity((width * height * 3) as usize);
        for y in 0..height {
            let row_start = (y * bytes_per_row) as usize;
            for x in 0..width {
                let offset = row_start + (x * 4) as usize;
                // BGRA -> RGB (skip alpha)
                rgb_data.push(raw_data[offset + 2]); // R
                rgb_data.push(raw_data[offset + 1]); // G
                rgb_data.push(raw_data[offset]);     // B
            }
        }

        let img = image::RgbImage::from_raw(width as u32, height as u32, rgb_data)
            .ok_or_else(|| ComputerError::Screenshot("Failed to create image".to_string()))?;

        // resize with Nearest (fastest)
        let resized = DynamicImage::ImageRgb8(img)
            .resize_exact(AI_WIDTH, AI_HEIGHT, FilterType::Nearest);

        // encode jpeg with quality control
        let rgb = resized.to_rgb8();
        let mut buffer = Vec::with_capacity(200_000);
        let mut encoder = JpegEncoder::new_with_quality(&mut buffer, JPEG_QUALITY);
        encoder.encode_image(&rgb)
            .map_err(|e| ComputerError::Screenshot(e.to_string()))?;

        Ok(BASE64.encode(&buffer))
    }

    pub fn perform_action(&self, action: &ComputerAction) -> Result<Option<String>, ComputerError> {
        let mut enigo = Enigo::new(&Settings::default())
            .map_err(|e| ComputerError::Input(e.to_string()))?;

        match action.action.as_str() {
            "screenshot" => {
                let screenshot = self.take_screenshot()?;
                Ok(Some(screenshot))
            }

            "mouse_move" => {
                if let Some(coord) = action.coordinate {
                    let (x, y) = self.map_from_ai_space(coord[0], coord[1]);
                    enigo.move_mouse(x, y, Coordinate::Abs)
                        .map_err(|e| ComputerError::Input(e.to_string()))?;
                }
                Ok(None)
            }

            "left_click" => {
                if let Some(coord) = action.coordinate {
                    let (x, y) = self.map_from_ai_space(coord[0], coord[1]);
                    enigo.move_mouse(x, y, Coordinate::Abs)
                        .map_err(|e| ComputerError::Input(e.to_string()))?;
                }
                enigo.button(Button::Left, Direction::Click)
                    .map_err(|e| ComputerError::Input(e.to_string()))?;
                Ok(None)
            }

            "right_click" => {
                if let Some(coord) = action.coordinate {
                    let (x, y) = self.map_from_ai_space(coord[0], coord[1]);
                    enigo.move_mouse(x, y, Coordinate::Abs)
                        .map_err(|e| ComputerError::Input(e.to_string()))?;
                }
                enigo.button(Button::Right, Direction::Click)
                    .map_err(|e| ComputerError::Input(e.to_string()))?;
                Ok(None)
            }

            "middle_click" => {
                if let Some(coord) = action.coordinate {
                    let (x, y) = self.map_from_ai_space(coord[0], coord[1]);
                    enigo.move_mouse(x, y, Coordinate::Abs)
                        .map_err(|e| ComputerError::Input(e.to_string()))?;
                }
                enigo.button(Button::Middle, Direction::Click)
                    .map_err(|e| ComputerError::Input(e.to_string()))?;
                Ok(None)
            }

            "double_click" => {
                if let Some(coord) = action.coordinate {
                    let (x, y) = self.map_from_ai_space(coord[0], coord[1]);
                    enigo.move_mouse(x, y, Coordinate::Abs)
                        .map_err(|e| ComputerError::Input(e.to_string()))?;
                }
                enigo.button(Button::Left, Direction::Click)
                    .map_err(|e| ComputerError::Input(e.to_string()))?;
                enigo.button(Button::Left, Direction::Click)
                    .map_err(|e| ComputerError::Input(e.to_string()))?;
                Ok(None)
            }

            "triple_click" => {
                if let Some(coord) = action.coordinate {
                    let (x, y) = self.map_from_ai_space(coord[0], coord[1]);
                    enigo.move_mouse(x, y, Coordinate::Abs)
                        .map_err(|e| ComputerError::Input(e.to_string()))?;
                }
                for _ in 0..3 {
                    enigo.button(Button::Left, Direction::Click)
                        .map_err(|e| ComputerError::Input(e.to_string()))?;
                }
                Ok(None)
            }

            "left_click_drag" => {
                if let (Some(start), Some(end)) = (&action.start_coordinate, &action.coordinate) {
                    let (sx, sy) = self.map_from_ai_space(start[0], start[1]);
                    let (ex, ey) = self.map_from_ai_space(end[0], end[1]);

                    enigo.move_mouse(sx, sy, Coordinate::Abs)
                        .map_err(|e| ComputerError::Input(e.to_string()))?;
                    enigo.button(Button::Left, Direction::Press)
                        .map_err(|e| ComputerError::Input(e.to_string()))?;
                    enigo.move_mouse(ex, ey, Coordinate::Abs)
                        .map_err(|e| ComputerError::Input(e.to_string()))?;
                    enigo.button(Button::Left, Direction::Release)
                        .map_err(|e| ComputerError::Input(e.to_string()))?;
                }
                Ok(None)
            }

            "type" => {
                if let Some(text) = &action.text {
                    enigo.text(text)
                        .map_err(|e| ComputerError::Input(e.to_string()))?;
                }
                Ok(None)
            }

            "key" => {
                if let Some(key_str) = &action.text {
                    #[cfg(target_os = "macos")]
                    {
                        self.press_key_applescript(key_str)?;
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        self.press_key(&mut enigo, key_str)?;
                    }
                }
                Ok(None)
            }

            "scroll" => {
                if let Some(coord) = action.coordinate {
                    let (x, y) = self.map_from_ai_space(coord[0], coord[1]);
                    enigo.move_mouse(x, y, Coordinate::Abs)
                        .map_err(|e| ComputerError::Input(e.to_string()))?;
                }

                let amount = action.scroll_amount.unwrap_or(3);
                let direction = action.scroll_direction.as_deref().unwrap_or("down");

                let scroll_amount = match direction {
                    "up" => amount,
                    "down" => -amount,
                    "left" => -amount,
                    "right" => amount,
                    _ => -amount,
                };

                if direction == "up" || direction == "down" {
                    enigo.scroll(scroll_amount, enigo::Axis::Vertical)
                        .map_err(|e| ComputerError::Input(e.to_string()))?;
                } else {
                    enigo.scroll(scroll_amount, enigo::Axis::Horizontal)
                        .map_err(|e| ComputerError::Input(e.to_string()))?;
                }
                Ok(None)
            }

            "wait" => {
                std::thread::sleep(std::time::Duration::from_secs(1));
                Ok(None)
            }

            _ => Err(ComputerError::UnknownAction(action.action.clone())),
        }
    }

    fn map_from_ai_space(&self, x: i32, y: i32) -> (i32, i32) {
        let scaled_x = (x as f64 * self.screen_width as f64 / AI_WIDTH as f64) as i32;
        let scaled_y = (y as f64 * self.screen_height as f64 / AI_HEIGHT as f64) as i32;
        (scaled_x, scaled_y)
    }

    fn press_key(&self, enigo: &mut Enigo, key_str: &str) -> Result<(), ComputerError> {
        // handle key combinations like "cmd+c", "ctrl+shift+a"
        let parts: Vec<&str> = key_str.split('+').collect();

        let mut modifiers = Vec::new();
        let mut main_key = None;

        for part in parts {
            let part_lower = part.to_lowercase();
            match part_lower.as_str() {
                "cmd" | "command" | "super" | "meta" => modifiers.push(Key::Meta),
                "ctrl" | "control" => modifiers.push(Key::Control),
                "alt" | "option" => modifiers.push(Key::Alt),
                "shift" => modifiers.push(Key::Shift),
                _ => main_key = Some(self.parse_key(part)?),
            }
        }

        // press modifiers
        for m in &modifiers {
            enigo.key(*m, Direction::Press)
                .map_err(|e| ComputerError::Input(e.to_string()))?;
        }

        // press main key
        if let Some(key) = main_key {
            enigo.key(key, Direction::Click)
                .map_err(|e| ComputerError::Input(e.to_string()))?;
        }

        // release modifiers in reverse
        for m in modifiers.iter().rev() {
            enigo.key(*m, Direction::Release)
                .map_err(|e| ComputerError::Input(e.to_string()))?;
        }

        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn press_key_applescript(&self, key_str: &str) -> Result<(), ComputerError> {
        use std::process::Command;

        // parse the key combo (e.g. "cmd+t", "ctrl+shift+a")
        let parts: Vec<&str> = key_str.split('+').collect();
        let mut modifiers = Vec::new();
        let mut main_key = String::new();

        for part in parts {
            let part_lower = part.to_lowercase();
            match part_lower.as_str() {
                "cmd" | "command" | "super" | "meta" => modifiers.push("command down"),
                "ctrl" | "control" => modifiers.push("control down"),
                "alt" | "option" => modifiers.push("option down"),
                "shift" => modifiers.push("shift down"),
                _ => main_key = part_lower,
            }
        }

        // map key names to applescript key codes or key names
        let key_code = match main_key.as_str() {
            "return" | "enter" => "return",
            "tab" => "tab",
            "escape" | "esc" => "escape",
            "space" => "space",
            "delete" | "backspace" => "delete",
            "up" | "uparrow" => "up arrow",
            "down" | "downarrow" => "down arrow",
            "left" | "leftarrow" => "left arrow",
            "right" | "rightarrow" => "right arrow",
            k => k,
        };

        let modifier_str = if modifiers.is_empty() {
            String::new()
        } else {
            format!(" using {{{}}}", modifiers.join(", "))
        };

        let script = format!(
            r#"tell application "System Events" to keystroke "{}"{}
"#,
            key_code, modifier_str
        );

        Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
            .map_err(|e| ComputerError::Input(e.to_string()))?;

        Ok(())
    }

    fn parse_key(&self, key_str: &str) -> Result<Key, ComputerError> {
        let key_lower = key_str.to_lowercase();
        match key_lower.as_str() {
            "return" | "enter" => Ok(Key::Return),
            "tab" => Ok(Key::Tab),
            "escape" | "esc" => Ok(Key::Escape),
            "backspace" | "delete" => Ok(Key::Backspace),
            "space" => Ok(Key::Space),
            "up" | "uparrow" => Ok(Key::UpArrow),
            "down" | "downarrow" => Ok(Key::DownArrow),
            "left" | "leftarrow" => Ok(Key::LeftArrow),
            "right" | "rightarrow" => Ok(Key::RightArrow),
            "home" => Ok(Key::Home),
            "end" => Ok(Key::End),
            "pageup" => Ok(Key::PageUp),
            "pagedown" => Ok(Key::PageDown),
            "f1" => Ok(Key::F1),
            "f2" => Ok(Key::F2),
            "f3" => Ok(Key::F3),
            "f4" => Ok(Key::F4),
            "f5" => Ok(Key::F5),
            "f6" => Ok(Key::F6),
            "f7" => Ok(Key::F7),
            "f8" => Ok(Key::F8),
            "f9" => Ok(Key::F9),
            "f10" => Ok(Key::F10),
            "f11" => Ok(Key::F11),
            "f12" => Ok(Key::F12),
            _ => {
                // single character
                if let Some(c) = key_str.chars().next() {
                    Ok(Key::Unicode(c))
                } else {
                    Err(ComputerError::UnknownAction(format!("Unknown key: {}", key_str)))
                }
            }
        }
    }
}
