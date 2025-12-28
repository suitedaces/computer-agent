use enigo::{Enigo, Key, Keyboard, Mouse, Settings, Coordinate, Button, Direction};
use image::imageops::FilterType;
use image::DynamicImage;
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::thread;
use std::time::Duration;
use thiserror::Error;
use xcap::Monitor;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

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

        // resize to AI space - use Triangle filter (fast, good enough for screenshots)
        let resized = DynamicImage::ImageRgba8(image)
            .resize_exact(AI_WIDTH, AI_HEIGHT, FilterType::Triangle);

        // encode as JPEG (much faster than PNG, still good quality)
        let mut buffer = Vec::new();
        resized
            .write_to(&mut Cursor::new(&mut buffer), image::ImageFormat::Jpeg)
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
                thread::sleep(Duration::from_millis(50));
                Ok(None)
            }

            "left_click" => {
                if let Some(coord) = action.coordinate {
                    let (x, y) = self.map_from_ai_space(coord[0], coord[1]);
                    enigo.move_mouse(x, y, Coordinate::Abs)
                        .map_err(|e| ComputerError::Input(e.to_string()))?;
                    thread::sleep(Duration::from_millis(50));
                }
                enigo.button(Button::Left, Direction::Click)
                    .map_err(|e| ComputerError::Input(e.to_string()))?;
                thread::sleep(Duration::from_millis(100));
                Ok(None)
            }

            "right_click" => {
                if let Some(coord) = action.coordinate {
                    let (x, y) = self.map_from_ai_space(coord[0], coord[1]);
                    enigo.move_mouse(x, y, Coordinate::Abs)
                        .map_err(|e| ComputerError::Input(e.to_string()))?;
                    thread::sleep(Duration::from_millis(50));
                }
                enigo.button(Button::Right, Direction::Click)
                    .map_err(|e| ComputerError::Input(e.to_string()))?;
                thread::sleep(Duration::from_millis(100));
                Ok(None)
            }

            "middle_click" => {
                if let Some(coord) = action.coordinate {
                    let (x, y) = self.map_from_ai_space(coord[0], coord[1]);
                    enigo.move_mouse(x, y, Coordinate::Abs)
                        .map_err(|e| ComputerError::Input(e.to_string()))?;
                    thread::sleep(Duration::from_millis(50));
                }
                enigo.button(Button::Middle, Direction::Click)
                    .map_err(|e| ComputerError::Input(e.to_string()))?;
                thread::sleep(Duration::from_millis(100));
                Ok(None)
            }

            "double_click" => {
                if let Some(coord) = action.coordinate {
                    let (x, y) = self.map_from_ai_space(coord[0], coord[1]);
                    enigo.move_mouse(x, y, Coordinate::Abs)
                        .map_err(|e| ComputerError::Input(e.to_string()))?;
                    thread::sleep(Duration::from_millis(50));
                }
                enigo.button(Button::Left, Direction::Click)
                    .map_err(|e| ComputerError::Input(e.to_string()))?;
                thread::sleep(Duration::from_millis(50));
                enigo.button(Button::Left, Direction::Click)
                    .map_err(|e| ComputerError::Input(e.to_string()))?;
                thread::sleep(Duration::from_millis(100));
                Ok(None)
            }

            "triple_click" => {
                if let Some(coord) = action.coordinate {
                    let (x, y) = self.map_from_ai_space(coord[0], coord[1]);
                    enigo.move_mouse(x, y, Coordinate::Abs)
                        .map_err(|e| ComputerError::Input(e.to_string()))?;
                    thread::sleep(Duration::from_millis(50));
                }
                for _ in 0..3 {
                    enigo.button(Button::Left, Direction::Click)
                        .map_err(|e| ComputerError::Input(e.to_string()))?;
                    thread::sleep(Duration::from_millis(50));
                }
                thread::sleep(Duration::from_millis(100));
                Ok(None)
            }

            "left_click_drag" => {
                if let (Some(start), Some(end)) = (&action.start_coordinate, &action.coordinate) {
                    let (sx, sy) = self.map_from_ai_space(start[0], start[1]);
                    let (ex, ey) = self.map_from_ai_space(end[0], end[1]);

                    enigo.move_mouse(sx, sy, Coordinate::Abs)
                        .map_err(|e| ComputerError::Input(e.to_string()))?;
                    thread::sleep(Duration::from_millis(50));

                    enigo.button(Button::Left, Direction::Press)
                        .map_err(|e| ComputerError::Input(e.to_string()))?;
                    thread::sleep(Duration::from_millis(50));

                    enigo.move_mouse(ex, ey, Coordinate::Abs)
                        .map_err(|e| ComputerError::Input(e.to_string()))?;
                    thread::sleep(Duration::from_millis(50));

                    enigo.button(Button::Left, Direction::Release)
                        .map_err(|e| ComputerError::Input(e.to_string()))?;
                }
                thread::sleep(Duration::from_millis(100));
                Ok(None)
            }

            "type" => {
                if let Some(text) = &action.text {
                    enigo.text(text)
                        .map_err(|e| ComputerError::Input(e.to_string()))?;
                }
                thread::sleep(Duration::from_millis(50));
                Ok(None)
            }

            "key" => {
                if let Some(key_str) = &action.text {
                    self.press_key(&mut enigo, key_str)?;
                }
                thread::sleep(Duration::from_millis(100));
                Ok(None)
            }

            "scroll" => {
                if let Some(coord) = action.coordinate {
                    let (x, y) = self.map_from_ai_space(coord[0], coord[1]);
                    enigo.move_mouse(x, y, Coordinate::Abs)
                        .map_err(|e| ComputerError::Input(e.to_string()))?;
                    thread::sleep(Duration::from_millis(50));
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

                // vertical scroll
                if direction == "up" || direction == "down" {
                    enigo.scroll(scroll_amount, enigo::Axis::Vertical)
                        .map_err(|e| ComputerError::Input(e.to_string()))?;
                } else {
                    enigo.scroll(scroll_amount, enigo::Axis::Horizontal)
                        .map_err(|e| ComputerError::Input(e.to_string()))?;
                }
                thread::sleep(Duration::from_millis(100));
                Ok(None)
            }

            "wait" => {
                // wait action - just sleep
                thread::sleep(Duration::from_millis(500));
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
