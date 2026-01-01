// shared audio playback utility

// Web Audio context for generated sounds
let audioContext: AudioContext | null = null;

function getAudioContext(): AudioContext {
  if (!audioContext) {
    audioContext = new AudioContext();
  }
  // resume if suspended (browser autoplay policy)
  if (audioContext.state === "suspended") {
    audioContext.resume();
  }
  return audioContext;
}

// iOS-style subtle pop/click - two layered tones for depth
export function playClickSound() {
  try {
    const ctx = getAudioContext();

    // layer 1: soft pop (higher freq, quick)
    const osc1 = ctx.createOscillator();
    const gain1 = ctx.createGain();
    osc1.connect(gain1);
    gain1.connect(ctx.destination);
    osc1.type = "sine";
    osc1.frequency.setValueAtTime(1800, ctx.currentTime);
    osc1.frequency.exponentialRampToValueAtTime(1200, ctx.currentTime + 0.03);
    gain1.gain.setValueAtTime(0.06, ctx.currentTime);
    gain1.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.04);
    osc1.start(ctx.currentTime);
    osc1.stop(ctx.currentTime + 0.04);

    // layer 2: body (lower, adds warmth)
    const osc2 = ctx.createOscillator();
    const gain2 = ctx.createGain();
    osc2.connect(gain2);
    gain2.connect(ctx.destination);
    osc2.type = "sine";
    osc2.frequency.setValueAtTime(600, ctx.currentTime);
    osc2.frequency.exponentialRampToValueAtTime(400, ctx.currentTime + 0.025);
    gain2.gain.setValueAtTime(0.04, ctx.currentTime);
    gain2.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.03);
    osc2.start(ctx.currentTime);
    osc2.stop(ctx.currentTime + 0.03);
  } catch (e) {
    console.error("[audio] click error:", e);
  }
}

// iOS keyboard-style soft tick
export function playTypeSound() {
  try {
    const ctx = getAudioContext();

    // soft high tick with slight randomness
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    const filter = ctx.createBiquadFilter();

    osc.connect(filter);
    filter.connect(gain);
    gain.connect(ctx.destination);

    filter.type = "lowpass";
    filter.frequency.setValueAtTime(3000, ctx.currentTime);

    osc.type = "sine";
    const baseFreq = 2200 + Math.random() * 200;
    osc.frequency.setValueAtTime(baseFreq, ctx.currentTime);
    osc.frequency.exponentialRampToValueAtTime(baseFreq * 0.7, ctx.currentTime + 0.02);

    gain.gain.setValueAtTime(0.035, ctx.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.025);

    osc.start(ctx.currentTime);
    osc.stop(ctx.currentTime + 0.025);
  } catch (e) {
    console.error("[audio] type error:", e);
  }
}

// soft completion chime
export function playDoneSound() {
  const ctx = getAudioContext();

  [523.25, 659.25, 783.99].forEach((freq, i) => {
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();

    osc.connect(gain);
    gain.connect(ctx.destination);

    osc.type = "sine";
    osc.frequency.setValueAtTime(freq, ctx.currentTime);

    const startTime = ctx.currentTime + i * 0.08;
    gain.gain.setValueAtTime(0, startTime);
    gain.gain.linearRampToValueAtTime(0.06, startTime + 0.02);
    gain.gain.exponentialRampToValueAtTime(0.001, startTime + 0.3);

    osc.start(startTime);
    osc.stop(startTime + 0.3);
  });
}

// messenger-style ambient blip - soft bubbly sound while agent works
let ambientInterval: ReturnType<typeof setInterval> | null = null;
let ambientPaused = false;

function playBloop() {
  if (ambientPaused) return;

  const ctx = getAudioContext();

  const osc = ctx.createOscillator();
  const gain = ctx.createGain();

  osc.connect(gain);
  gain.connect(ctx.destination);

  // rising pitch = bubbly/friendly feel
  osc.type = "sine";
  osc.frequency.setValueAtTime(600, ctx.currentTime);
  osc.frequency.exponentialRampToValueAtTime(900, ctx.currentTime + 0.06);

  gain.gain.setValueAtTime(0, ctx.currentTime);
  gain.gain.linearRampToValueAtTime(0.04, ctx.currentTime + 0.015);
  gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.12);

  osc.start(ctx.currentTime);
  osc.stop(ctx.currentTime + 0.12);
}

export function startAmbientSound() {
  if (ambientInterval) return;
  ambientPaused = false;

  // bloop every 1.2 seconds
  ambientInterval = setInterval(() => {
    playBloop();
  }, 1200);
}

export function stopAmbientSound() {
  if (ambientInterval) {
    clearInterval(ambientInterval);
    ambientInterval = null;
  }
  ambientPaused = true; // prevent any pending plays
}

export function pauseAmbientSound() {
  ambientPaused = true;
}

export function resumeAmbientSound() {
  // only resume if ambient is still active
  if (ambientInterval) {
    ambientPaused = false;
  }
}

// iOS-style camera shutter - crisp snap with tonal body
export function playScreenshotSound() {
  try {
    const ctx = getAudioContext();

    // layer 1: crisp noise snap
    const bufferSize = ctx.sampleRate * 0.05;
    const buffer = ctx.createBuffer(1, bufferSize, ctx.sampleRate);
    const data = buffer.getChannelData(0);
    for (let i = 0; i < bufferSize; i++) {
      const decay = Math.pow(1 - i / bufferSize, 3);
      data[i] = (Math.random() * 2 - 1) * decay;
    }
    const noise = ctx.createBufferSource();
    noise.buffer = buffer;
    const noiseGain = ctx.createGain();
    const noiseFilter = ctx.createBiquadFilter();
    noiseFilter.type = "highpass";
    noiseFilter.frequency.setValueAtTime(2500, ctx.currentTime);
    noise.connect(noiseFilter);
    noiseFilter.connect(noiseGain);
    noiseGain.connect(ctx.destination);
    noiseGain.gain.setValueAtTime(0.05, ctx.currentTime);
    noiseGain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.05);
    noise.start();
    noise.stop(ctx.currentTime + 0.05);

    // layer 2: tonal "click" body
    const osc = ctx.createOscillator();
    const oscGain = ctx.createGain();
    osc.connect(oscGain);
    oscGain.connect(ctx.destination);
    osc.type = "sine";
    osc.frequency.setValueAtTime(1400, ctx.currentTime);
    osc.frequency.exponentialRampToValueAtTime(800, ctx.currentTime + 0.04);
    oscGain.gain.setValueAtTime(0.05, ctx.currentTime);
    oscGain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.05);
    osc.start(ctx.currentTime);
    osc.stop(ctx.currentTime + 0.05);
  } catch (e) {
    console.error("[audio] screenshot error:", e);
  }
}

let currentAudio: HTMLAudioElement | null = null;
const audioQueue: string[] = [];
let isPlaying = false;
let onAudioEndCallback: (() => void) | null = null;

function base64ToBlob(base64: string): Blob {
  const binaryString = atob(base64);
  const bytes = new Uint8Array(binaryString.length);
  for (let i = 0; i < binaryString.length; i++) {
    bytes[i] = binaryString.charCodeAt(i);
  }
  return new Blob([bytes], { type: "audio/mpeg" });
}

async function playNext() {
  if (isPlaying || audioQueue.length === 0) return;
  isPlaying = true;

  const base64Audio = audioQueue.shift()!;
  try {
    const blob = base64ToBlob(base64Audio);
    const url = URL.createObjectURL(blob);
    currentAudio = new Audio(url);

    await new Promise<void>((resolve) => {
      currentAudio!.onended = () => {
        URL.revokeObjectURL(url);
        resolve();
      };
      currentAudio!.onerror = () => {
        URL.revokeObjectURL(url);
        resolve();
      };
      currentAudio!.play().catch(() => resolve());
    });
  } catch (e) {
    console.error("Audio playback failed:", e);
  }

  currentAudio = null;
  isPlaying = false;

  // if queue empty, call end callback (resume ambient)
  if (audioQueue.length === 0 && onAudioEndCallback) {
    onAudioEndCallback();
  }

  playNext();
}

// queue audio for sequential playback
export function queueAudio(base64Audio: string) {
  audioQueue.push(base64Audio);
  playNext();
}

// stop current playback and clear queue
export function stopAudio() {
  audioQueue.length = 0;
  if (currentAudio) {
    currentAudio.pause();
    currentAudio = null;
  }
  isPlaying = false;
}

// check if currently playing
export function isAudioPlaying(): boolean {
  return isPlaying;
}

// set callback for when audio queue empties
export function setAudioEndCallback(callback: (() => void) | null) {
  onAudioEndCallback = callback;
}

// create replayable audio element
export function createAudioElement(base64Audio: string): HTMLAudioElement {
  const blob = base64ToBlob(base64Audio);
  const url = URL.createObjectURL(blob);
  return new Audio(url);
}
