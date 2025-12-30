use bytes::{BufMut, BytesMut};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use tokio::sync::oneshot;

use deepgram::common::options::{Encoding, Model, Options};
use deepgram::common::stream_response::StreamResponse;
use deepgram::Deepgram;
use futures::StreamExt;

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
    result_tx: Arc<Mutex<Option<oneshot::Sender<String>>>>,
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
