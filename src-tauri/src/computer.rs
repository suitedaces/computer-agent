#[cfg(not(target_os = "macos"))]
use enigo::{Enigo, Key, Mouse, Settings, Coordinate, Button, Direction, Keyboard};
#[cfg(target_os = "macos")]
use enigo::{Enigo, Mouse, Settings, Coordinate, Button, Direction};
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
use core_graphics::window::kCGWindowImageDefault;
#[cfg(target_os = "macos")]
use core_graphics::event::{CGEvent, CGEventFlags, CGKeyCode, CGEventTapLocation};
#[cfg(target_os = "macos")]
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
#[cfg(target_os = "macos")]
use foreign_types::ForeignType;

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

    /// take screenshot excluding our app windows - captures everything BELOW the given window
    #[cfg(target_os = "macos")]
    pub fn take_screenshot_excluding(&self, window_id: u32) -> Result<String, ComputerError> {
        use core_graphics::window::{
            kCGWindowListOptionOnScreenBelowWindow, kCGWindowListExcludeDesktopElements,
            CGWindowListCreateImage,
        };

        let bounds = CGRect::new(
            &CGPoint::new(0.0, 0.0),
            &CGSize::new(self.screen_width as f64, self.screen_height as f64),
        );

        // capture all windows BELOW our window (excludes our app and everything above it)
        let options = kCGWindowListOptionOnScreenBelowWindow | kCGWindowListExcludeDesktopElements;

        let cg_image = unsafe {
            let img_ptr = CGWindowListCreateImage(
                bounds,
                options,
                window_id,
                kCGWindowImageDefault,
            );
            if img_ptr.is_null() {
                return self.take_screenshot();
            }
            core_graphics::image::CGImage::from_ptr(img_ptr)
        };

        let width = cg_image.width();
        let height = cg_image.height();
        let bytes_per_row = cg_image.bytes_per_row();
        let data = cg_image.data();
        let raw_data = data.bytes();

        // fast BGRA -> RGB conversion
        let mut rgb_data = Vec::with_capacity((width * height * 3) as usize);
        for y in 0..height {
            let row_start = (y * bytes_per_row) as usize;
            for x in 0..width {
                let offset = row_start + (x * 4) as usize;
                rgb_data.push(raw_data[offset + 2]); // R
                rgb_data.push(raw_data[offset + 1]); // G
                rgb_data.push(raw_data[offset]);     // B
            }
        }

        let img = image::RgbImage::from_raw(width as u32, height as u32, rgb_data)
            .ok_or_else(|| ComputerError::Screenshot("Failed to create image".to_string()))?;

        let resized = DynamicImage::ImageRgb8(img)
            .resize_exact(AI_WIDTH, AI_HEIGHT, FilterType::Nearest);

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
                    #[cfg(target_os = "macos")]
                    {
                        self.type_text_applescript(text)?;
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        enigo.text(text)
                            .map_err(|e| ComputerError::Input(e.to_string()))?;
                    }
                }
                Ok(None)
            }

            "key" => {
                if let Some(key_str) = &action.text {
                    #[cfg(target_os = "macos")]
                    {
                        self.press_key_cgevent(key_str)?;
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

                // enigo scroll: positive = down, negative = up
                // horizontal: positive = right, negative = left
                let scroll_amount = match direction {
                    "up" => -amount,
                    "down" => amount,
                    "left" => -amount,
                    "right" => amount,
                    _ => amount,
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

    #[cfg(target_os = "macos")]
    fn type_text_applescript(&self, text: &str) -> Result<(), ComputerError> {
        use std::process::Command;

        // escape quotes and backslashes for applescript
        let escaped = text
            .replace('\\', "\\\\")
            .replace('"', "\\\"");

        let script = format!(
            r#"tell application "System Events" to keystroke "{}"
"#,
            escaped
        );

        Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
            .map_err(|e| ComputerError::Input(e.to_string()))?;

        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn press_key_cgevent(&self, key_str: &str) -> Result<(), ComputerError> {
        // parse the key combo (e.g. "cmd+t", "ctrl+shift+a")
        let parts: Vec<&str> = key_str.split('+').collect();
        let mut flags = CGEventFlags::empty();
        let mut main_key = String::new();

        for part in parts {
            let part_lower = part.to_lowercase();
            match part_lower.as_str() {
                "cmd" | "command" | "super" | "meta" => flags |= CGEventFlags::CGEventFlagCommand,
                "ctrl" | "control" => flags |= CGEventFlags::CGEventFlagControl,
                "alt" | "option" => flags |= CGEventFlags::CGEventFlagAlternate,
                "shift" => flags |= CGEventFlags::CGEventFlagShift,
                _ => main_key = part_lower,
            }
        }

        // map key names to CGKeyCode (from core_graphics::event::KeyCode)
        let key_code: CGKeyCode = match main_key.as_str() {
            "return" | "enter" => 0x24,
            "tab" => 0x30,
            "space" => 0x31,
            "delete" | "backspace" => 0x33,
            "escape" | "esc" => 0x35,
            "forwarddelete" => 0x75,
            "up" | "uparrow" => 0x7E,
            "down" | "downarrow" => 0x7D,
            "left" | "leftarrow" => 0x7B,
            "right" | "rightarrow" => 0x7C,
            "home" => 0x73,
            "end" => 0x77,
            "pageup" => 0x74,
            "pagedown" => 0x79,
            "f1" => 0x7A,
            "f2" => 0x78,
            "f3" => 0x63,
            "f4" => 0x76,
            "f5" => 0x60,
            "f6" => 0x61,
            "f7" => 0x62,
            "f8" => 0x64,
            "f9" => 0x65,
            "f10" => 0x6D,
            "f11" => 0x67,
            "f12" => 0x6F,
            // single character - map to key code
            _ => self.char_to_keycode(&main_key)?,
        };

        // create event source
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| ComputerError::Input("failed to create event source".to_string()))?;

        // key down
        let key_down = CGEvent::new_keyboard_event(source.clone(), key_code, true)
            .map_err(|_| ComputerError::Input("failed to create key down event".to_string()))?;
        if !flags.is_empty() {
            key_down.set_flags(flags);
        }
        key_down.post(CGEventTapLocation::HID);

        // small delay between down and up
        std::thread::sleep(std::time::Duration::from_millis(10));

        // key up
        let key_up = CGEvent::new_keyboard_event(source, key_code, false)
            .map_err(|_| ComputerError::Input("failed to create key up event".to_string()))?;
        if !flags.is_empty() {
            key_up.set_flags(flags);
        }
        key_up.post(CGEventTapLocation::HID);

        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn char_to_keycode(&self, c: &str) -> Result<CGKeyCode, ComputerError> {
        // map single characters to their key codes
        let code = match c.chars().next() {
            Some('a') => 0x00,
            Some('s') => 0x01,
            Some('d') => 0x02,
            Some('f') => 0x03,
            Some('h') => 0x04,
            Some('g') => 0x05,
            Some('z') => 0x06,
            Some('x') => 0x07,
            Some('c') => 0x08,
            Some('v') => 0x09,
            Some('b') => 0x0B,
            Some('q') => 0x0C,
            Some('w') => 0x0D,
            Some('e') => 0x0E,
            Some('r') => 0x0F,
            Some('y') => 0x10,
            Some('t') => 0x11,
            Some('1') => 0x12,
            Some('2') => 0x13,
            Some('3') => 0x14,
            Some('4') => 0x15,
            Some('6') => 0x16,
            Some('5') => 0x17,
            Some('9') => 0x19,
            Some('7') => 0x1A,
            Some('8') => 0x1C,
            Some('0') => 0x1D,
            Some('o') => 0x1F,
            Some('u') => 0x20,
            Some('i') => 0x22,
            Some('p') => 0x23,
            Some('l') => 0x25,
            Some('j') => 0x26,
            Some('k') => 0x28,
            Some('n') => 0x2D,
            Some('m') => 0x2E,
            Some('-') => 0x1B,
            Some('=') => 0x18,
            Some('[') => 0x21,
            Some(']') => 0x1E,
            Some('\\') => 0x2A,
            Some(';') => 0x29,
            Some('\'') => 0x27,
            Some(',') => 0x2B,
            Some('.') => 0x2F,
            Some('/') => 0x2C,
            Some('`') => 0x32,
            _ => return Err(ComputerError::Input(format!("unknown key: {}", c))),
        };
        Ok(code)
    }

    #[cfg(not(target_os = "macos"))]
    fn press_key(&self, enigo: &mut Enigo, key_str: &str) -> Result<(), ComputerError> {
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

        for m in &modifiers {
            enigo.key(*m, Direction::Press)
                .map_err(|e| ComputerError::Input(e.to_string()))?;
        }

        if let Some(key) = main_key {
            enigo.key(key, Direction::Click)
                .map_err(|e| ComputerError::Input(e.to_string()))?;
        }

        for m in modifiers.iter().rev() {
            enigo.key(*m, Direction::Release)
                .map_err(|e| ComputerError::Input(e.to_string()))?;
        }

        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
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
                if let Some(c) = key_str.chars().next() {
                    Ok(Key::Unicode(c))
                } else {
                    Err(ComputerError::UnknownAction(format!("Unknown key: {}", key_str)))
                }
            }
        }
    }
}
