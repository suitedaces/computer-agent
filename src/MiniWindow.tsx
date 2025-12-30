import { useEffect, useState, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { motion, AnimatePresence } from "framer-motion";
import { ChevronRight, Send, X } from "lucide-react";
import ChatView from "./components/ChatView";
import { useAgent } from "./hooks/useAgent";
import VoiceOrb from "./components/VoiceOrb";

export default function MiniWindow() {
  const { submit } = useAgent();
  const [isRunning, setIsRunning] = useState(false);
  const [helpMode, setHelpMode] = useState(false);
  const [helpPrompt, setHelpPrompt] = useState("");
  const [helpScreenshot, setHelpScreenshot] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // PTT state
  const [pttRecording, setPttRecording] = useState(false);
  const [pttInterim, setPttInterim] = useState("");
  const [pttRetryMode, setPttRetryMode] = useState(false);

  // poll running state and resize window
  useEffect(() => {
    // don't poll/resize while in special modes - backend handles sizing
    if (helpMode || pttRecording || pttRetryMode) return;

    const checkRunning = () => {
      invoke<boolean>("is_agent_running").then((running) => {
        setIsRunning(running);
        const win = getCurrentWindow();
        if (!running) {
          // idle bar size
          win.setSize(new LogicalSize(280, 36));
        } else {
          // running feed size
          win.setSize(new LogicalSize(380, 320));
        }
      }).catch(() => {});
    };
    checkRunning();
    const interval = setInterval(checkRunning, 500);
    return () => clearInterval(interval);
  }, [helpMode, pttRecording, pttRetryMode]);

  useEffect(() => {
    const unlisten1 = listen("agent:started", () => {
      setIsRunning(true);
    });

    const unlisten2 = listen("agent:stopped", () => {
      setIsRunning(false);
    });

    // hotkey help mode - Cmd+Shift+H triggers this
    const unlisten3 = listen<{ screenshot: string | null }>("hotkey-help", async (e) => {
      if (e.payload.screenshot) {
        setHelpScreenshot(e.payload.screenshot);
        setHelpMode(true);
        setHelpPrompt("");
        setTimeout(() => inputRef.current?.focus(), 100);
      }
    });

    // PTT recording state
    const unlisten4 = listen<{ recording: boolean }>("ptt:recording", (e) => {
      console.log("[ptt] recording:", e.payload.recording);
      setPttRecording(e.payload.recording);
      if (e.payload.recording) {
        setPttInterim("");
        setPttRetryMode(false);
      }
    });

    // PTT interim transcription
    const unlisten5 = listen<string>("ptt:interim", (e) => {
      console.log("[ptt] interim:", e.payload);
      setPttInterim(e.payload);
    });

    // PTT result - auto-submit or show retry
    const unlisten6 = listen<{ text: string; screenshot: string | null }>("ptt:result", async (e) => {
      console.log("[ptt] result:", e.payload);
      setPttRecording(false);
      setPttInterim("");

      const { text, screenshot } = e.payload;

      if (!text.trim()) {
        // empty transcription - show retry mode
        console.log("[ptt] empty transcription, showing retry");
        setPttRetryMode(true);

        // show mini window in retry mode
        const win = getCurrentWindow();
        await win.setSize(new LogicalSize(300, 300));
        return;
      }

      // has transcription - auto-submit to spotlight
      try {
        await invoke("show_spotlight_window");
        await new Promise((r) => setTimeout(r, 150));
        await submit(text, screenshot ?? undefined);
        await invoke("hide_mini_window");
      } catch (err) {
        console.error("[ptt] submit failed:", err);
      }
    });

    // PTT error
    const unlisten7 = listen<string>("ptt:error", (e) => {
      console.error("[ptt] error:", e.payload);
      setPttRecording(false);
      setPttInterim("");
    });

    return () => {
      unlisten1.then((f) => f());
      unlisten2.then((f) => f());
      unlisten3.then((f) => f());
      unlisten4.then((f) => f());
      unlisten5.then((f) => f());
      unlisten6.then((f) => f());
      unlisten7.then((f) => f());
    };
  }, [submit]);

  const handleOpenMain = async () => {
    try {
      await invoke("show_main_window");
    } catch (e) {
      console.error(e);
    }
  };

  const handleHelpSubmit = async () => {
    if (!helpPrompt.trim() || !helpScreenshot) return;

    // capture values before any state changes
    const prompt = helpPrompt;
    const screenshot = helpScreenshot;

    try {
      // show spotlight first
      await invoke("show_spotlight_window");
      await new Promise((r) => setTimeout(r, 150));

      // use the shared useAgent submit which goes through the hook
      await submit(prompt, screenshot ?? undefined);

      // now hide mini and clear state
      setHelpMode(false);
      await invoke("hide_mini_window");

      setHelpScreenshot(null);
      setHelpPrompt("");
    } catch (e) {
      console.error("[mini] help submit failed:", e);
    }
  };

  const handleHelpCancel = async () => {
    setHelpMode(false);
    setHelpScreenshot(null);
    setHelpPrompt("");
    // show_mini_window handles resize + reposition to top right
    await invoke("show_mini_window");
  };

  // help mode UI - animated photo capture
  if (helpMode && !isRunning) {
    return (
      <div className="h-screen w-screen flex items-center justify-center p-4 bg-black/40">
        <motion.div
          initial={{ opacity: 0, scale: 0.8, rotateX: 15, y: 50 }}
          animate={{ opacity: 1, scale: 1, rotateX: 0, y: 0 }}
          transition={{ type: "spring", damping: 25, stiffness: 300, duration: 0.4 }}
          className="w-full max-w-[480px]"
          style={{ perspective: 1000 }}
        >
          {/* polaroid-style photo card */}
          <motion.div
            initial={{ boxShadow: "0 0 0 rgba(255,255,255,0)" }}
            animate={{ boxShadow: "0 25px 50px -12px rgba(0,0,0,0.5)" }}
            transition={{ delay: 0.1, duration: 0.3 }}
            className="bg-gradient-to-b from-zinc-900 to-black rounded-2xl border border-white/10 overflow-hidden"
          >
            {/* screenshot with flash overlay */}
            {helpScreenshot && (
              <div className="relative p-3 pb-2">
                <motion.div
                  initial={{ opacity: 0.8 }}
                  animate={{ opacity: 0 }}
                  transition={{ duration: 0.3 }}
                  className="absolute inset-0 bg-white pointer-events-none z-10"
                />
                <motion.img
                  initial={{ opacity: 0, scale: 1.02 }}
                  animate={{ opacity: 1, scale: 1 }}
                  transition={{ delay: 0.15, duration: 0.3 }}
                  src={`data:image/jpeg;base64,${helpScreenshot}`}
                  alt="Screenshot"
                  className="w-full rounded-xl border border-white/5"
                />
              </div>
            )}

            {/* prompt input */}
            <motion.div
              initial={{ opacity: 0, y: 10 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ delay: 0.2, duration: 0.2 }}
              className="px-3 pb-2"
            >
              <input
                ref={inputRef}
                type="text"
                value={helpPrompt}
                onChange={(e) => setHelpPrompt(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && handleHelpSubmit()}
                placeholder="What do you need help with?"
                className="w-full bg-white/5 border border-white/10 rounded-xl px-4 py-3 text-[13px] text-white/90 placeholder:text-white/30 focus:outline-none focus:border-white/20 focus:bg-white/10 transition-all"
                autoFocus
              />
            </motion.div>

            {/* action buttons */}
            <motion.div
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              transition={{ delay: 0.25, duration: 0.2 }}
              className="px-3 pb-3 flex gap-2"
            >
              <button
                onClick={handleHelpCancel}
                className="flex-1 flex items-center justify-center gap-1.5 py-2.5 rounded-xl bg-red-500/20 border border-red-400/20 text-red-300 hover:bg-red-500/30 transition-all text-[12px]"
              >
                <X size={14} />
                <span>Cancel</span>
              </button>
              <button
                onClick={handleHelpSubmit}
                disabled={!helpPrompt.trim()}
                className="flex-1 flex items-center justify-center gap-1.5 py-2.5 rounded-xl bg-blue-500/30 border border-blue-400/30 text-blue-200 hover:bg-blue-500/40 disabled:opacity-40 disabled:cursor-not-allowed transition-all text-[12px] font-medium"
              >
                <Send size={14} />
                <span>Send</span>
              </button>
            </motion.div>
          </motion.div>
        </motion.div>
      </div>
    );
  }

  // PTT recording - orb with streaming text, fully transparent
  if (pttRecording && !isRunning) {
    return (
      <div className="h-screen w-screen flex flex-col items-center justify-center">
        <motion.div
          initial={{ opacity: 0, scale: 0.8 }}
          animate={{ opacity: 1, scale: 1 }}
          className="flex flex-col items-center"
        >
          <VoiceOrb isActive={true} volume={0.3} size={200} />

          <AnimatePresence mode="wait">
            {pttInterim ? (
              <motion.div
                key="interim"
                initial={{ opacity: 0, y: 5 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0 }}
                className="mt-4 px-4 py-2.5 bg-black rounded-2xl"
              >
                <p className="text-white font-medium text-[14px] text-center max-w-[220px] leading-relaxed">
                  {pttInterim}
                </p>
              </motion.div>
            ) : (
              <motion.div
                key="hint"
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                className="mt-4 px-4 py-2 bg-black rounded-full"
              >
                <p className="text-white/90 text-[12px]">listening...</p>
              </motion.div>
            )}
          </AnimatePresence>
        </motion.div>
      </div>
    );
  }

  // PTT retry mode - empty transcription, transparent with orb
  if (pttRetryMode && !isRunning) {
    const handleRetryCancel = async () => {
      setPttRetryMode(false);
      await invoke("show_mini_window");
    };

    return (
      <div className="h-screen w-screen flex flex-col items-center justify-center">
        <motion.div
          initial={{ opacity: 0, scale: 0.8 }}
          animate={{ opacity: 1, scale: 1 }}
          className="flex flex-col items-center"
        >
          <VoiceOrb isActive={false} volume={0} size={200} />

          <motion.div
            initial={{ opacity: 0, y: 5 }}
            animate={{ opacity: 1, y: 0 }}
            className="mt-4 px-4 py-3 bg-black rounded-2xl text-center"
          >
            <p className="text-white/90 text-[13px] font-medium">No speech detected</p>
            <p className="text-white/50 text-[11px] mt-1">Hold ⌘⇧V and speak</p>

            <button
              onClick={handleRetryCancel}
              className="mt-3 px-4 py-1.5 bg-white/10 hover:bg-white/20 rounded-full text-white/70 text-[11px] transition-colors"
            >
              Dismiss
            </button>
          </motion.div>
        </motion.div>
      </div>
    );
  }

  // idle bar
  if (!isRunning) {
    return (
      <motion.div
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={{ duration: 0.12, ease: "easeOut" }}
        onClick={handleOpenMain}
        className="h-screen w-screen mini-idle flex items-center gap-2 px-3 cursor-pointer"
      >
        <img src="/windows-computer-icon.png" alt="" className="w-4 h-4 opacity-60" />
        <span className="text-[12px] text-white/60 flex items-center gap-1">summon an agent <ChevronRight size={12} /></span>
      </motion.div>
    );
  }

  // running - show mini ChatView
  return <ChatView variant="mini" />;
}
