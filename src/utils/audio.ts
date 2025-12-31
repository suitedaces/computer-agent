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

// subtle click sound
export function playClickSound() {
  try {
    const ctx = getAudioContext();
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();

    osc.connect(gain);
    gain.connect(ctx.destination);

    osc.frequency.setValueAtTime(800, ctx.currentTime);
    osc.frequency.exponentialRampToValueAtTime(400, ctx.currentTime + 0.05);

    gain.gain.setValueAtTime(0.08, ctx.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.05);

    osc.start(ctx.currentTime);
    osc.stop(ctx.currentTime + 0.05);
  } catch (e) {
    console.error("[audio] click error:", e);
  }
}

// subtle typing sound
export function playTypeSound() {
  const ctx = getAudioContext();
  const osc = ctx.createOscillator();
  const gain = ctx.createGain();

  osc.connect(gain);
  gain.connect(ctx.destination);

  osc.type = "square";
  osc.frequency.setValueAtTime(1200 + Math.random() * 400, ctx.currentTime);

  gain.gain.setValueAtTime(0.03, ctx.currentTime);
  gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.03);

  osc.start(ctx.currentTime);
  osc.stop(ctx.currentTime + 0.03);
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

let currentAudio: HTMLAudioElement | null = null;
const audioQueue: string[] = [];
let isPlaying = false;

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

// create replayable audio element
export function createAudioElement(base64Audio: string): HTMLAudioElement {
  const blob = base64ToBlob(base64Audio);
  const url = URL.createObjectURL(blob);
  return new Audio(url);
}
