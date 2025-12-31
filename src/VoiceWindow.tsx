import { useEffect, useState, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import VoiceOrb from "./components/VoiceOrb";
import { useAgent } from "./hooks/useAgent";
import { useAgentStore } from "./stores/agentStore";
import { motion } from "framer-motion";

// recording or retry - backend controls window visibility
type Mode = "recording" | "retry";

export default function VoiceWindow() {
  console.log("[VoiceWindow] render");
  const [mode, setMode] = useState<Mode>("recording");
  const [interim, setInterim] = useState("");
  const { submit } = useAgent();
  const setVoiceMode = useAgentStore((s) => s.setVoiceMode);

  const submitRef = useRef(submit);
  const pttDataRef = useRef<{ screenshot: string | null; mode: string } | null>(null);

  useEffect(() => {
    submitRef.current = submit;
  }, [submit]);

  useEffect(() => {
    console.log("[VoiceWindow] mounting event listeners...");
    invoke("debug_log", { message: "VoiceWindow mounting event listeners" }).catch(() => {});
    const listeners = [
      listen<{ recording: boolean; screenshot?: string; mode?: string }>(
        "ptt:recording",
        (e) => {
          console.log("[VoiceWindow] ptt:recording received:", e.payload);
          if (e.payload.recording) {
            pttDataRef.current = {
              screenshot: e.payload.screenshot || null,
              mode: e.payload.mode || "computer",
            };
            setMode("recording");
            setInterim("");
          }
        }
      ),

      listen<string>("ptt:interim", (e) => {
        console.log("[VoiceWindow] ptt:interim received:", e.payload);
        invoke("debug_log", { message: `VoiceWindow got interim: ${e.payload}` }).catch(() => {});
        setInterim(e.payload);
      }),

      listen<{ text: string; screenshot?: string; mode?: string }>(
        "ptt:result",
        async (e) => {
          console.log("[VoiceWindow] ptt:result received:", e.payload);
          const hasText = !!e.payload.text.trim();
          if (!hasText) {
            setMode("retry");
          } else {
            // backend hides window, submit with voice mode
            console.log("[VoiceWindow] submitting text:", e.payload.text);
            setVoiceMode(true);
            const data = pttDataRef.current;
            await submitRef.current(
              e.payload.text,
              data?.screenshot ?? undefined,
              data?.mode !== "current" ? data?.mode : undefined
            );
            console.log("[VoiceWindow] submit complete");
          }
          pttDataRef.current = null;
        }
      ),

      listen("ptt:error", () => {
        pttDataRef.current = null;
      }),
    ];

    return () => {
      listeners.forEach((p) => p.then((fn) => fn()));
    };
  }, [setVoiceMode]);

  const handleDismiss = () => {
    setMode("recording"); // reset for next time
    invoke("hide_voice_window").catch(console.error);
  };

  // always render - backend controls visibility
  if (mode === "recording") {
    return (
      <div className="h-full w-full flex flex-col items-center justify-center bg-transparent">
        <VoiceOrb isActive={true} volume={0.3} size={180} />
        <motion.div
          initial={{ opacity: 0, y: 5 }}
          animate={{ opacity: 1, y: 0 }}
          className="mt-4 px-4 py-2.5 bg-black/90 rounded-2xl max-w-[250px]"
        >
          <p className="text-white/90 text-sm text-center">
            {interim || "listening..."}
          </p>
        </motion.div>
      </div>
    );
  }

  // retry mode
  return (
    <div className="h-full w-full flex flex-col items-center justify-center bg-transparent">
      <VoiceOrb isActive={false} volume={0} size={180} />
      <div className="mt-4 px-4 py-3 bg-black/90 rounded-2xl text-center">
        <p className="text-white/90 text-sm font-medium">No speech detected</p>
        <p className="text-white/50 text-xs mt-1">Hold ^+Shift+C and speak</p>
        <button
          onClick={handleDismiss}
          className="mt-3 px-4 py-1.5 bg-white/10 hover:bg-white/20 rounded-full text-white/70 text-xs"
        >
          Dismiss
        </button>
      </div>
    </div>
  );
}
