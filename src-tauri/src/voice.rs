use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use bytes::{BufMut, BytesMut};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use thiserror::Error;

use deepgram::common::options::{Encoding, Endpointing, Language, Model, Options};
use deepgram::common::stream_response::StreamResponse;
use deepgram::Deepgram;
use futures::StreamExt;

// ============================================================================
// ElevenLabs TTS
// ============================================================================

const ELEVENLABS_API_URL: &str = "https://api.elevenlabs.io/v1/text-to-speech";

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
        if let Some(cached) = self.cache.lock().unwrap().get(text) {
            return Ok(cached.clone());
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
                "voice_settings": { "stability": 0.5, "similarity_boost": 0.75 }
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(TtsError::Api(format!("HTTP {}", response.status())));
        }

        let base64_audio = BASE64.encode(&response.bytes().await?);

        let mut cache = self.cache.lock().unwrap();
        if cache.len() >= 50 {
            if let Some(key) = cache.keys().next().cloned() {
                cache.remove(&key);
            }
        }
        cache.insert(text.to_string(), base64_audio.clone());

        Ok(base64_audio)
    }
}

pub fn create_tts_client() -> Option<TtsClient> {
    let api_key = std::env::var("ELEVENLABS_API_KEY").ok()?;
    let voice_id = std::env::var("ELEVENLABS_VOICE_ID")
        .unwrap_or_else(|_| "NOpBlnGInO9m6vDvFkFC".to_string());
    Some(TtsClient::new(api_key, voice_id))
}

// ============================================================================
// Deepgram STT - simple approach
// ============================================================================

#[derive(Clone, serde::Serialize)]
pub struct TranscriptionEvent {
    pub text: String,
    pub is_final: bool,
}

// mic -> mpsc channel -> deepgram websocket
fn start_mic_stream(
    is_running: Arc<AtomicBool>,
) -> Result<(futures::channel::mpsc::Receiver<Result<bytes::Bytes, std::io::Error>>, u32), String> {
    let host = cpal::default_host();
    let device = host.default_input_device().ok_or("no input device")?;
    let config = device.default_input_config().map_err(|e| e.to_string())?;
    let sample_rate = config.sample_rate().0;
    let channels = config.channels();

    println!("[mic] {}Hz, {} ch", sample_rate, channels);

    let (mut tx, rx) = futures::channel::mpsc::channel::<Result<bytes::Bytes, std::io::Error>>(100);

    let is_running_cb = is_running.clone();
    std::thread::spawn(move || {
        let stream = device.build_input_stream(
            &config.into(),
            move |data: &[f32], _: &_| {
                if !is_running_cb.load(Ordering::SeqCst) { return; }

                // convert to mono linear16
                let mono: Vec<f32> = if channels > 1 {
                    data.chunks(channels as usize)
                        .map(|c| c.iter().sum::<f32>() / channels as f32)
                        .collect()
                } else {
                    data.to_vec()
                };

                let mut bytes = BytesMut::with_capacity(mono.len() * 2);
                for s in mono {
                    bytes.put_i16_le((s * i16::MAX as f32) as i16);
                }

                let _ = tx.try_send(Ok(bytes.freeze()));
            },
            |e| println!("[mic] error: {}", e),
            None,
        ).ok();

        if let Some(s) = stream {
            let _ = s.play();
            println!("[mic] started");
            while is_running.load(Ordering::SeqCst) {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            println!("[mic] stopped");
        }
    });

    Ok((rx, sample_rate))
}

// ============================================================================
// PushToTalkSession - simple version
// ============================================================================

pub struct PushToTalkSession {
    is_running: Arc<AtomicBool>,
    accumulated_text: Arc<Mutex<String>>,
    session_id: Arc<AtomicU64>,
}

impl PushToTalkSession {
    pub fn new() -> Self {
        Self {
            is_running: Arc::new(AtomicBool::new(false)),
            accumulated_text: Arc::new(Mutex::new(String::new())),
            session_id: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    pub async fn stop(&self) -> (String, u64) {
        let session_id = self.session_id.load(Ordering::SeqCst);
        self.is_running.store(false, Ordering::SeqCst);

        // wait for final transcripts
        tokio::time::sleep(tokio::time::Duration::from_millis(800)).await;

        let text = self.accumulated_text.lock().unwrap().clone();
        self.accumulated_text.lock().unwrap().clear();
        (text, session_id)
    }

    pub async fn start(&self, api_key: String, app_handle: AppHandle) -> Result<u64, String> {
        if self.is_running.load(Ordering::SeqCst) {
            return Err("already running".to_string());
        }

        let session_id = self.session_id.fetch_add(1, Ordering::SeqCst) + 1;
        self.accumulated_text.lock().unwrap().clear();
        self.is_running.store(true, Ordering::SeqCst);

        let is_running = self.is_running.clone();
        let accumulated = self.accumulated_text.clone();
        let app = app_handle.clone();

        let (audio_rx, sample_rate) = start_mic_stream(is_running.clone())?;

        tokio::spawn(async move {
            let dg = match Deepgram::new(&api_key) {
                Ok(d) => d,
                Err(e) => {
                    println!("[ptt] deepgram init failed: {}", e);
                    return;
                }
            };

            let options = Options::builder()
                .model(Model::Nova3)
                .language(Language::multi)
                .smart_format(true)
                .build();

            let transcription = dg.transcription();
            let request = transcription
                .stream_request_with_options(options)
                .keep_alive()
                .encoding(Encoding::Linear16)
                .sample_rate(sample_rate)
                .channels(1)
                .endpointing(Endpointing::CustomDurationMs(300))
                .interim_results(true)
                .utterance_end_ms(1000)
                .vad_events(true)
                .no_delay(true);

            println!("[ptt] connecting to deepgram...");
            let mut results = match request.stream(audio_rx).await {
                Ok(r) => r,
                Err(e) => {
                    println!("[ptt] stream failed: {}", e);
                    return;
                }
            };
            println!("[ptt] connected");

            // process all results until stream ends
            while let Some(result) = results.next().await {
                match result {
                    Ok(StreamResponse::TranscriptResponse { channel, is_final, .. }) => {
                        if let Some(alt) = channel.alternatives.first() {
                            let text = &alt.transcript;
                            if !text.is_empty() {
                                println!("[ptt] {} (final={})", text, is_final);

                                if is_final {
                                    let mut acc = accumulated.lock().unwrap();
                                    if !acc.is_empty() { acc.push(' '); }
                                    acc.push_str(text);
                                    let _ = app.emit("ptt:interim", acc.clone());
                                } else {
                                    let acc = accumulated.lock().unwrap();
                                    let display = if acc.is_empty() {
                                        text.clone()
                                    } else {
                                        format!("{} {}", *acc, text)
                                    };
                                    let _ = app.emit("ptt:interim", display);
                                }
                            }
                        }
                    }
                    Ok(other) => println!("[ptt] {:?}", other),
                    Err(e) => println!("[ptt] error: {}", e),
                }
            }
            println!("[ptt] stream ended");
        });

        let _ = app_handle.emit("ptt:started", session_id);
        Ok(session_id)
    }
}

// ============================================================================
// VoiceSession - continuous mode (kept for compatibility)
// ============================================================================

pub struct VoiceSession {
    is_running: Arc<AtomicBool>,
}

impl VoiceSession {
    pub fn new() -> Self {
        Self { is_running: Arc::new(AtomicBool::new(false)) }
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    pub fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
    }

    pub async fn start(&self, api_key: String, app_handle: AppHandle) -> Result<(), String> {
        if self.is_running.load(Ordering::SeqCst) {
            return Err("already running".to_string());
        }

        self.is_running.store(true, Ordering::SeqCst);
        let is_running = self.is_running.clone();
        let app = app_handle.clone();

        let (audio_rx, sample_rate) = start_mic_stream(is_running.clone())?;

        tokio::spawn(async move {
            let dg = match Deepgram::new(&api_key) {
                Ok(d) => d,
                Err(e) => {
                    println!("[voice] deepgram init failed: {}", e);
                    is_running.store(false, Ordering::SeqCst);
                    return;
                }
            };

            let options = Options::builder()
                .model(Model::Nova3)
                .language(Language::multi)
                .smart_format(true)
                .build();

            let transcription = dg.transcription();
            let request = transcription
                .stream_request_with_options(options)
                .keep_alive()
                .encoding(Encoding::Linear16)
                .sample_rate(sample_rate)
                .channels(1)
                .interim_results(true);

            let mut results = match request.stream(audio_rx).await {
                Ok(r) => r,
                Err(e) => {
                    println!("[voice] stream failed: {}", e);
                    is_running.store(false, Ordering::SeqCst);
                    return;
                }
            };

            while let Some(result) = results.next().await {
                if let Ok(StreamResponse::TranscriptResponse { channel, is_final, .. }) = result {
                    if let Some(alt) = channel.alternatives.first() {
                        if !alt.transcript.is_empty() {
                            let _ = app.emit("voice:transcription", TranscriptionEvent {
                                text: alt.transcript.clone(),
                                is_final,
                            });
                        }
                    }
                }
            }

            is_running.store(false, Ordering::SeqCst);
            let _ = app.emit("voice:stopped", ());
        });

        let _ = app_handle.emit("voice:started", ());
        Ok(())
    }
}
