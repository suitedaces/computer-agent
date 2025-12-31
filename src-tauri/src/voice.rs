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
const TTS_CACHE_MAX_SIZE: usize = 50; // max cached entries

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
    cache: Mutex<HashMap<String, String>>, // text -> base64 audio
}

impl TtsClient {
    pub fn new(api_key: String, voice_id: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            voice_id,
            model_id: "eleven_flash_v2_5".to_string(), // 75ms latency
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// synthesize text to speech, returns base64-encoded mp3 audio (cached)
    pub async fn synthesize(&self, text: &str) -> Result<String, TtsError> {
        // check cache first
        {
            let cache = self.cache.lock().unwrap();
            if let Some(cached) = cache.get(text) {
                return Ok(cached.clone());
            }
        }

        let url = format!("{}/{}/stream", ELEVENLABS_API_URL, self.voice_id);

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
                    "similarity_boost": 0.75,
                    "speed": 1.0
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

        // store in cache
        {
            let mut cache = self.cache.lock().unwrap();
            // evict oldest if too large (simple FIFO-ish eviction)
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

/// create TTS client from environment variables
pub fn create_tts_client() -> Option<TtsClient> {
    let api_key = std::env::var("ELEVENLABS_API_KEY").ok()?;
    let voice_id = std::env::var("ELEVENLABS_VOICE_ID")
        .unwrap_or_else(|_| "21m00Tcm4TlvDq8ikWAM".to_string()); // default: Rachel

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

pub struct VoiceSession {
    is_running: Arc<AtomicBool>,
}

// push-to-talk session that accumulates transcription
pub struct PushToTalkSession {
    is_running: Arc<AtomicBool>,
    accumulated_text: Arc<Mutex<String>>,
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

        // get audio device info on current thread (before spawning)
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("no input device found")?;
        let device_name = device.name().unwrap_or_default();
        println!("[voice] using device: {}", device_name);

        let config = device
            .default_input_config()
            .map_err(|e| format!("no input config: {}", e))?;
        let sample_rate = config.sample_rate().0;
        let channels = config.channels();
        let sample_format = config.sample_format();
        println!(
            "[voice] config: {}Hz, {} channels, {:?}",
            sample_rate, channels, sample_format
        );

        // channel for audio data - use std::sync::mpsc for thread safety
        let (audio_tx, audio_rx) = std::sync::mpsc::channel::<Vec<f32>>();

        // spawn blocking thread for audio capture (cpal::Stream is not Send)
        let is_running_audio = is_running.clone();
        std::thread::spawn(move || {
            let stream = match sample_format {
                cpal::SampleFormat::F32 => {
                    let tx = audio_tx.clone();
                    let running = is_running_audio.clone();
                    device
                        .build_input_stream(
                            &config.into(),
                            move |data: &[f32], _: &_| {
                                if running.load(Ordering::SeqCst) {
                                    let _ = tx.send(data.to_vec());
                                }
                            },
                            |err| println!("[voice] stream error: {}", err),
                            None,
                        )
                        .ok()
                }
                cpal::SampleFormat::I16 => {
                    let tx = audio_tx.clone();
                    let running = is_running_audio.clone();
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
                            |err| println!("[voice] stream error: {}", err),
                            None,
                        )
                        .ok()
                }
                _ => {
                    println!("[voice] unsupported sample format: {:?}", sample_format);
                    None
                }
            };

            if let Some(stream) = stream {
                if stream.play().is_ok() {
                    println!("[voice] audio capture started");
                    // keep thread alive while running
                    while is_running_audio.load(Ordering::SeqCst) {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                }
                drop(stream);
            }
            println!("[voice] audio capture stopped");
        });

        // spawn async task for deepgram streaming
        let is_running_dg = is_running.clone();
        let app = app_handle.clone();
        tokio::spawn(async move {
            if let Err(e) =
                run_deepgram_session(api_key, sample_rate, channels, audio_rx, is_running_dg.clone(), app.clone())
                    .await
            {
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

async fn run_deepgram_session(
    api_key: String,
    sample_rate: u32,
    channels: u16,
    audio_rx: std::sync::mpsc::Receiver<Vec<f32>>,
    is_running: Arc<AtomicBool>,
    app_handle: AppHandle,
) -> Result<(), String> {
    println!("[voice] connecting to deepgram");

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

    // channel for websocket audio bytes
    let (mut ws_tx, ws_rx) =
        futures::channel::mpsc::channel::<Result<bytes::Bytes, std::io::Error>>(100);

    // spawn task to read from audio_rx and send to websocket
    let is_running_fwd = is_running.clone();
    tokio::task::spawn_blocking(move || {
        while is_running_fwd.load(Ordering::SeqCst) {
            match audio_rx.recv_timeout(std::time::Duration::from_millis(100)) {
                Ok(samples) => {
                    // convert to mono if stereo
                    let mono: Vec<f32> = if channels > 1 {
                        samples
                            .chunks(channels as usize)
                            .map(|chunk| chunk.iter().sum::<f32>() / channels as f32)
                            .collect()
                    } else {
                        samples
                    };

                    // convert f32 to i16 linear16
                    let mut bytes = BytesMut::with_capacity(mono.len() * 2);
                    for sample in mono {
                        let s = (sample * i16::MAX as f32) as i16;
                        bytes.put_i16_le(s);
                    }

                    // blocking send
                    if ws_tx.try_send(Ok(bytes.freeze())).is_err() {
                        break;
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        println!("[voice] audio forwarder stopped");
    });

    // start streaming transcription
    let mut results = request
        .stream(ws_rx)
        .await
        .map_err(|e| format!("stream failed: {}", e))?;

    println!("[voice] deepgram connected");

    // process transcription results
    while is_running.load(Ordering::SeqCst) {
        tokio::select! {
            result = results.next() => {
                match result {
                    Some(Ok(response)) => {
                        handle_transcription(response, &app_handle);
                    }
                    Some(Err(e)) => {
                        println!("[voice] transcription error: {}", e);
                    }
                    None => {
                        println!("[voice] stream ended");
                        break;
                    }
                }
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                // check is_running periodically
            }
        }
    }

    println!("[voice] deepgram session ended");
    Ok(())
}

fn handle_transcription(response: StreamResponse, app_handle: &AppHandle) {
    match response {
        StreamResponse::TranscriptResponse {
            channel, is_final, ..
        } => {
            if let Some(alt) = channel.alternatives.first() {
                let text = &alt.transcript;
                if !text.is_empty() {
                    println!("[voice] transcript: {} (final: {})", text, is_final);
                    let _ = app_handle.emit(
                        "voice:transcription",
                        TranscriptionEvent {
                            text: text.clone(),
                            is_final,
                        },
                    );
                }
            }
        }
        _ => {}
    }
}

// push-to-talk implementation
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

    /// stop recording and return accumulated transcription
    pub async fn stop(&self) -> String {
        self.is_running.store(false, Ordering::SeqCst);

        // wait a bit for final transcription to arrive
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let text = self.accumulated_text.lock().unwrap().clone();
        // clear for next session
        self.accumulated_text.lock().unwrap().clear();
        text
    }

    pub async fn start(&self, api_key: String, app_handle: AppHandle) -> Result<(), String> {
        if self.is_running.load(Ordering::SeqCst) {
            return Err("PTT session already running".to_string());
        }

        // clear previous accumulated text
        self.accumulated_text.lock().unwrap().clear();
        self.is_running.store(true, Ordering::SeqCst);
        let is_running = self.is_running.clone();
        let accumulated = self.accumulated_text.clone();

        // get audio device info
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("no input device found")?;
        let device_name = device.name().unwrap_or_default();
        println!("[ptt] using device: {}", device_name);

        let config = device
            .default_input_config()
            .map_err(|e| format!("no input config: {}", e))?;
        let sample_rate = config.sample_rate().0;
        let channels = config.channels();
        let sample_format = config.sample_format();
        println!("[ptt] config: {}Hz, {} channels, {:?}", sample_rate, channels, sample_format);

        let (audio_tx, audio_rx) = std::sync::mpsc::channel::<Vec<f32>>();

        // audio capture thread
        let is_running_audio = is_running.clone();
        std::thread::spawn(move || {
            let stream = match sample_format {
                cpal::SampleFormat::F32 => {
                    let tx = audio_tx.clone();
                    let running = is_running_audio.clone();
                    device
                        .build_input_stream(
                            &config.into(),
                            move |data: &[f32], _: &_| {
                                if running.load(Ordering::SeqCst) {
                                    let _ = tx.send(data.to_vec());
                                }
                            },
                            |err| println!("[ptt] stream error: {}", err),
                            None,
                        )
                        .ok()
                }
                cpal::SampleFormat::I16 => {
                    let tx = audio_tx.clone();
                    let running = is_running_audio.clone();
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
                            |err| println!("[ptt] stream error: {}", err),
                            None,
                        )
                        .ok()
                }
                _ => {
                    println!("[ptt] unsupported sample format: {:?}", sample_format);
                    None
                }
            };

            if let Some(stream) = stream {
                if stream.play().is_ok() {
                    println!("[ptt] audio capture started");
                    while is_running_audio.load(Ordering::SeqCst) {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                }
                drop(stream);
            }
            println!("[ptt] audio capture stopped");
        });

        // deepgram streaming task
        let is_running_dg = is_running.clone();
        let app = app_handle.clone();
        tokio::spawn(async move {
            if let Err(e) = run_ptt_deepgram_session(
                api_key,
                sample_rate,
                channels,
                audio_rx,
                is_running_dg.clone(),
                accumulated,
                app.clone(),
            )
            .await
            {
                println!("[ptt] error: {}", e);
                let _ = app.emit("ptt:error", e);
            }
            is_running_dg.store(false, Ordering::SeqCst);
        });

        let _ = app_handle.emit("ptt:started", ());
        Ok(())
    }
}

async fn run_ptt_deepgram_session(
    api_key: String,
    sample_rate: u32,
    channels: u16,
    audio_rx: std::sync::mpsc::Receiver<Vec<f32>>,
    is_running: Arc<AtomicBool>,
    accumulated_text: Arc<Mutex<String>>,
    app_handle: AppHandle,
) -> Result<(), String> {
    println!("[ptt] connecting to deepgram");

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

    let (mut ws_tx, ws_rx) =
        futures::channel::mpsc::channel::<Result<bytes::Bytes, std::io::Error>>(100);

    // audio forwarder
    let is_running_fwd = is_running.clone();
    tokio::task::spawn_blocking(move || {
        while is_running_fwd.load(Ordering::SeqCst) {
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
        println!("[ptt] audio forwarder stopped");
    });

    let mut results = request
        .stream(ws_rx)
        .await
        .map_err(|e| format!("stream failed: {}", e))?;

    println!("[ptt] deepgram connected");

    while is_running.load(Ordering::SeqCst) {
        tokio::select! {
            result = results.next() => {
                match result {
                    Some(Ok(response)) => {
                        handle_ptt_transcription(response, &accumulated_text, &app_handle);
                    }
                    Some(Err(e)) => {
                        println!("[ptt] transcription error: {}", e);
                    }
                    None => {
                        println!("[ptt] stream ended");
                        break;
                    }
                }
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {}
        }
    }

    println!("[ptt] deepgram session ended");
    Ok(())
}

fn handle_ptt_transcription(
    response: StreamResponse,
    accumulated_text: &Arc<Mutex<String>>,
    app_handle: &AppHandle,
) {
    match response {
        StreamResponse::TranscriptResponse {
            channel, is_final, ..
        } => {
            if let Some(alt) = channel.alternatives.first() {
                let text = &alt.transcript;
                if !text.is_empty() {
                    println!("[ptt] transcript: {} (final: {})", text, is_final);

                    // emit interim for visual feedback
                    let _ = app_handle.emit("ptt:interim", text.clone());

                    // only accumulate final transcriptions
                    if is_final {
                        let mut acc = accumulated_text.lock().unwrap();
                        if !acc.is_empty() {
                            acc.push(' ');
                        }
                        acc.push_str(text);
                    }
                }
            }
        }
        _ => {}
    }
}
