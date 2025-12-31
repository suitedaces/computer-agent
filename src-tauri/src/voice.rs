use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use bytes::{BufMut, BytesMut};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use thiserror::Error;

use deepgram::common::options::{Encoding, Model, Options};
use deepgram::common::stream_response::StreamResponse;
use deepgram::Deepgram;
use futures::StreamExt;

// ============================================================================
// ElevenLabs TTS (Text-to-Speech)
// ============================================================================

const ELEVENLABS_API_URL: &str = "https://api.elevenlabs.io/v1/text-to-speech";
const TTS_CACHE_MAX_SIZE: usize = 50;

#[derive(Error, Debug)]
pub enum TtsError {
    #[error("HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("API error: {0}")]
    Api(String),
}

pub struct TtsClient {
    client: reqwest::Client,
    api_key: String,
    voice_id: String,
    model_id: String,
    cache: Mutex<HashMap<String, String>>,
}

impl TtsClient {
    pub fn new(api_key: String, voice_id: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            voice_id,
            model_id: "eleven_flash_v2_5".to_string(),
            cache: Mutex::new(HashMap::new()),
        }
    }

    pub async fn synthesize(&self, text: &str) -> Result<String, TtsError> {
        // check cache
        {
            let cache = self.cache.lock().unwrap();
            if let Some(cached) = cache.get(text) {
                return Ok(cached.clone());
            }
        }

        let url = format!("{}/{}", ELEVENLABS_API_URL, self.voice_id);

        let response = self
            .client
            .post(&url)
            .header("xi-api-key", &self.api_key)
            .header("Content-Type", "application/json")
            .query(&[("output_format", "mp3_44100_128")])
            .json(&serde_json::json!({
                "text": text,
                "model_id": self.model_id,
                "voice_settings": {
                    "stability": 0.5,
                    "similarity_boost": 0.75
                }
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(TtsError::Api(format!("HTTP {}: {}", status, body)));
        }

        let bytes = response.bytes().await?;
        let base64_audio = BASE64.encode(&bytes);

        // cache it
        {
            let mut cache = self.cache.lock().unwrap();
            if cache.len() >= TTS_CACHE_MAX_SIZE {
                if let Some(key) = cache.keys().next().cloned() {
                    cache.remove(&key);
                }
            }
            cache.insert(text.to_string(), base64_audio.clone());
        }

        Ok(base64_audio)
    }
}

pub fn create_tts_client() -> Option<TtsClient> {
    let api_key = std::env::var("ELEVENLABS_API_KEY").ok()?;
    let voice_id = std::env::var("ELEVENLABS_VOICE_ID")
        .unwrap_or_else(|_| "21m00Tcm4TlvDq8ikWAM".to_string());
    Some(TtsClient::new(api_key, voice_id))
}

// ============================================================================
// Deepgram STT (Speech-to-Text)
// ============================================================================

#[derive(Clone, serde::Serialize)]
pub struct TranscriptionEvent {
    pub text: String,
    pub is_final: bool,
}

// shared audio capture config
struct AudioConfig {
    sample_rate: u32,
    channels: u16,
}

// starts audio capture thread, returns receiver for audio samples
fn start_audio_capture(
    is_running: Arc<AtomicBool>,
    log_prefix: &'static str,
) -> Result<(std::sync::mpsc::Receiver<Vec<f32>>, AudioConfig), String> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or("no input device found")?;
    println!("[{}] using device: {}", log_prefix, device.name().unwrap_or_default());

    let config = device
        .default_input_config()
        .map_err(|e| format!("no input config: {}", e))?;
    let sample_rate = config.sample_rate().0;
    let channels = config.channels();
    let sample_format = config.sample_format();
    println!("[{}] config: {}Hz, {} channels, {:?}", log_prefix, sample_rate, channels, sample_format);

    let (audio_tx, audio_rx) = std::sync::mpsc::channel::<Vec<f32>>();

    std::thread::spawn(move || {
        let stream = match sample_format {
            cpal::SampleFormat::F32 => {
                let tx = audio_tx.clone();
                let running = is_running.clone();
                device
                    .build_input_stream(
                        &config.into(),
                        move |data: &[f32], _: &_| {
                            if running.load(Ordering::SeqCst) {
                                let _ = tx.send(data.to_vec());
                            }
                        },
                        |err| println!("[audio] stream error: {}", err),
                        None,
                    )
                    .ok()
            }
            cpal::SampleFormat::I16 => {
                let tx = audio_tx.clone();
                let running = is_running.clone();
                device
                    .build_input_stream(
                        &config.into(),
                        move |data: &[i16], _: &_| {
                            if running.load(Ordering::SeqCst) {
                                let floats: Vec<f32> =
                                    data.iter().map(|&s| s as f32 / i16::MAX as f32).collect();
                                let _ = tx.send(floats);
                            }
                        },
                        |err| println!("[audio] stream error: {}", err),
                        None,
                    )
                    .ok()
            }
            _ => {
                println!("[audio] unsupported format: {:?}", sample_format);
                None
            }
        };

        if let Some(stream) = stream {
            if stream.play().is_ok() {
                println!("[{}] audio capture started", log_prefix);
                while is_running.load(Ordering::SeqCst) {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            }
            drop(stream);
        }
        println!("[{}] audio capture stopped", log_prefix);
    });

    Ok((audio_rx, AudioConfig { sample_rate, channels }))
}

// starts audio forwarder task that converts samples to linear16 and sends to websocket
fn start_audio_forwarder(
    audio_rx: std::sync::mpsc::Receiver<Vec<f32>>,
    channels: u16,
    is_running: Arc<AtomicBool>,
) -> futures::channel::mpsc::Receiver<Result<bytes::Bytes, std::io::Error>> {
    let (mut ws_tx, ws_rx) =
        futures::channel::mpsc::channel::<Result<bytes::Bytes, std::io::Error>>(100);

    tokio::task::spawn_blocking(move || {
        while is_running.load(Ordering::SeqCst) {
            match audio_rx.recv_timeout(std::time::Duration::from_millis(100)) {
                Ok(samples) => {
                    let mono: Vec<f32> = if channels > 1 {
                        samples
                            .chunks(channels as usize)
                            .map(|chunk| chunk.iter().sum::<f32>() / channels as f32)
                            .collect()
                    } else {
                        samples
                    };

                    let mut bytes = BytesMut::with_capacity(mono.len() * 2);
                    for sample in mono {
                        let s = (sample * i16::MAX as f32) as i16;
                        bytes.put_i16_le(s);
                    }

                    if ws_tx.try_send(Ok(bytes.freeze())).is_err() {
                        break;
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });

    ws_rx
}

// transcription callback type
type TranscriptCallback = Box<dyn Fn(&str, bool) + Send + 'static>;

// runs deepgram streaming session with callback for transcripts
async fn run_deepgram_streaming(
    api_key: String,
    sample_rate: u32,
    ws_rx: futures::channel::mpsc::Receiver<Result<bytes::Bytes, std::io::Error>>,
    is_running: Arc<AtomicBool>,
    on_transcript: TranscriptCallback,
) -> Result<(), String> {
    let dg = Deepgram::new(&api_key).map_err(|e| format!("deepgram init failed: {}", e))?;

    let options = Options::builder()
        .model(Model::Nova2)
        .smart_format(true)
        .build();

    let transcription = dg.transcription();
    let request = transcription
        .stream_request_with_options(options)
        .keep_alive()
        .channels(1)
        .sample_rate(sample_rate)
        .encoding(Encoding::Linear16);

    let mut results = request
        .stream(ws_rx)
        .await
        .map_err(|e| format!("stream failed: {}", e))?;

    while is_running.load(Ordering::SeqCst) {
        tokio::select! {
            result = results.next() => {
                match result {
                    Some(Ok(response)) => {
                        if let StreamResponse::TranscriptResponse { channel, is_final, .. } = response {
                            if let Some(alt) = channel.alternatives.first() {
                                let text = &alt.transcript;
                                if !text.is_empty() {
                                    on_transcript(text, is_final);
                                }
                            }
                        }
                    }
                    Some(Err(e)) => println!("[deepgram] error: {}", e),
                    None => break,
                }
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {}
        }
    }

    Ok(())
}

// ============================================================================
// VoiceSession - continuous transcription with events
// ============================================================================

pub struct VoiceSession {
    is_running: Arc<AtomicBool>,
}

impl VoiceSession {
    pub fn new() -> Self {
        Self {
            is_running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    pub fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
    }

    pub async fn start(&self, api_key: String, app_handle: AppHandle) -> Result<(), String> {
        if self.is_running.load(Ordering::SeqCst) {
            return Err("Voice session already running".to_string());
        }

        self.is_running.store(true, Ordering::SeqCst);
        let is_running = self.is_running.clone();

        let (audio_rx, config) = start_audio_capture(is_running.clone(), "voice")?;
        let ws_rx = start_audio_forwarder(audio_rx, config.channels, is_running.clone());

        let is_running_dg = is_running.clone();
        let app = app_handle.clone();
        tokio::spawn(async move {
            let app_cb = app.clone();
            let result = run_deepgram_streaming(
                api_key,
                config.sample_rate,
                ws_rx,
                is_running_dg.clone(),
                Box::new(move |text, is_final| {
                    println!("[voice] transcript: {} (final: {})", text, is_final);
                    let _ = app_cb.emit("voice:transcription", TranscriptionEvent {
                        text: text.to_string(),
                        is_final,
                    });
                }),
            ).await;

            if let Err(e) = result {
                println!("[voice] error: {}", e);
                let _ = app.emit("voice:error", e);
            }
            is_running_dg.store(false, Ordering::SeqCst);
            let _ = app.emit("voice:stopped", ());
        });

        let _ = app_handle.emit("voice:started", ());
        Ok(())
    }
}

// ============================================================================
// PushToTalkSession - accumulates transcription until stopped
// ============================================================================

pub struct PushToTalkSession {
    is_running: Arc<AtomicBool>,
    accumulated_text: Arc<Mutex<String>>,
}

impl PushToTalkSession {
    pub fn new() -> Self {
        Self {
            is_running: Arc::new(AtomicBool::new(false)),
            accumulated_text: Arc::new(Mutex::new(String::new())),
        }
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    pub async fn stop(&self) -> String {
        self.is_running.store(false, Ordering::SeqCst);
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let text = self.accumulated_text.lock().unwrap().clone();
        self.accumulated_text.lock().unwrap().clear();
        text
    }

    pub async fn start(&self, api_key: String, app_handle: AppHandle) -> Result<(), String> {
        if self.is_running.load(Ordering::SeqCst) {
            return Err("PTT session already running".to_string());
        }

        self.accumulated_text.lock().unwrap().clear();
        self.is_running.store(true, Ordering::SeqCst);
        let is_running = self.is_running.clone();
        let accumulated = self.accumulated_text.clone();

        let (audio_rx, config) = start_audio_capture(is_running.clone(), "ptt")?;
        let ws_rx = start_audio_forwarder(audio_rx, config.channels, is_running.clone());

        let is_running_dg = is_running.clone();
        let app = app_handle.clone();
        tokio::spawn(async move {
            let accumulated_cb = accumulated.clone();
            let app_cb = app.clone();
            let result = run_deepgram_streaming(
                api_key,
                config.sample_rate,
                ws_rx,
                is_running_dg.clone(),
                Box::new(move |text, is_final| {
                    println!("[ptt] transcript: {} (final: {})", text, is_final);
                    let _ = app_cb.emit("ptt:interim", text.to_string());

                    if is_final {
                        let mut acc = accumulated_cb.lock().unwrap();
                        if !acc.is_empty() {
                            acc.push(' ');
                        }
                        acc.push_str(text);
                    }
                }),
            ).await;

            if let Err(e) = result {
                println!("[ptt] error: {}", e);
                let _ = app.emit("ptt:error", e);
            }
            is_running_dg.store(false, Ordering::SeqCst);
        });

        let _ = app_handle.emit("ptt:started", ());
        Ok(())
    }
}
