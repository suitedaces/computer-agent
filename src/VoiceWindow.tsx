import { useEffect, useState, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import VoiceOrb from "./components/VoiceOrb";
import { useAgent } from "./hooks/useAgent";
import { useAgentStore } from "./stores/agentStore";
import { motion } from "framer-motion";

type State = "hidden" | "recording" | "retry";

export default function VoiceWindow() {
  const [state, setState] = useState<State>("hidden");
  const [interim, setInterim] = useState("");
  const { submit } = useAgent();
  const setVoiceMode = useAgentStore((s) => s.setVoiceMode);

  const submitRef = useRef(submit);
  const pttDataRef = useRef<{ screenshot: string | null; mode: string } | null>(null);

  useEffect(() => {
    submitRef.current = submit;
  }, [submit]);

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
            setState("recording");
            setInterim("");
            invoke("show_voice_window").catch(console.error);
          }
        }
      ),

      listen<string>("ptt:interim", (e) => {
        setInterim(e.payload);
      }),

      listen<{ text: string; screenshot?: string; mode?: string }>(
        "ptt:result",
        async (e) => {
          const hasText = !!e.payload.text.trim();
          if (!hasText) {
            setState("retry");
          } else {
            setState("hidden");
            invoke("hide_voice_window").catch(console.error);
            // submit with voice mode enabled
            setVoiceMode(true);
            const data = pttDataRef.current;
            await submitRef.current(
              e.payload.text,
              data?.screenshot ?? undefined,
              data?.mode !== "current" ? data?.mode : undefined
            );
          }
          pttDataRef.current = null;
        }
      ),

      listen("ptt:error", () => {
        setState("hidden");
        invoke("hide_voice_window").catch(console.error);
        pttDataRef.current = null;
      }),
    ];

    return () => {
      listeners.forEach((p) => p.then((fn) => fn()));
    };
  }, [setVoiceMode]);

  const handleDismiss = () => {
    setState("hidden");
    invoke("hide_voice_window").catch(console.error);
  };

  // hidden state - render nothing (window is hidden anyway)
  if (state === "hidden") {
    return <div className="h-full w-full" />;
  }

  // recording state
  if (state === "recording") {
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

  // retry state
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
