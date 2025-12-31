import { useEffect, useState, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import VoiceOrb from "./components/VoiceOrb";
import { motion } from "framer-motion";

type Mode = "recording" | "retry";

export default function VoiceWindow() {
  const [mode, setMode] = useState<Mode>("recording");
  const [interim, setInterim] = useState("");

  const pttDataRef = useRef<{ screenshot: string | null; mode: string } | null>(null);

  useEffect(() => {
    const listeners = [
      listen<{ recording: boolean; screenshot?: string; mode?: string }>(
        "ptt:recording",
        (e) => {
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
        setInterim(e.payload);
      }),

      listen<{ text: string }>(
        "ptt:result",
        async (e) => {
          const hasText = !!e.payload.text.trim();
          if (!hasText) {
            setMode("retry");
          } else {
            // hide voice window, show main window in voice response mode
            await invoke("hide_voice_window").catch(() => {});

            const data = pttDataRef.current;
            // hand off to main window
            await invoke("show_main_voice_response", {
              text: e.payload.text,
              screenshot: data?.screenshot ?? null,
              mode: data?.mode !== "current" ? data?.mode : "computer",
            }).catch(console.error);
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
  }, []);

  const handleDismiss = () => {
    setMode("recording");
    invoke("hide_voice_window").catch(console.error);
  };

  // recording mode
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
        <p className="text-white/50 text-xs mt-1">Hold ^+Shift and speak</p>
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
