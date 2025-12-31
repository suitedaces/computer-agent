// shared audio playback utility

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
