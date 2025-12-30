import { useEffect, useState, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { motion } from "framer-motion";
import { ChevronRight, Send, X, Mic, RotateCcw } from "lucide-react";
import ChatView from "./components/ChatView";
import { useAgent } from "./hooks/useAgent";

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
  const [pttRetryScreenshot, setPttRetryScreenshot] = useState<string | null>(null);

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
        setPttRetryScreenshot(screenshot);

        // show mini window in retry mode
        const win = getCurrentWindow();
        await win.setSize(new LogicalSize(320, 180));
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

  // PTT recording indicator overlay
  if (pttRecording && !isRunning) {
    return (
      <div className="h-screen w-screen flex flex-col items-center justify-center p-4 bg-black/60">
        <motion.div
          initial={{ opacity: 0, scale: 0.9 }}
          animate={{ opacity: 1, scale: 1 }}
          className="w-full max-w-[280px] bg-zinc-900/95 rounded-xl border border-red-500/30 p-4"
        >
          <div className="flex items-center gap-3 mb-2">
            <motion.div
              animate={{ scale: [1, 1.15, 1] }}
              transition={{ repeat: Infinity, duration: 1.2 }}
              className="w-10 h-10 rounded-full bg-red-500/30 border border-red-400 flex items-center justify-center flex-shrink-0"
            >
              <Mic size={18} className="text-red-400" />
            </motion.div>
            <div className="flex-1 min-w-0">
              <span className="text-white/80 text-sm font-medium">Recording...</span>
              <div className="text-white/30 text-[10px]">Release to send</div>
            </div>
          </div>

          {pttInterim && (
            <div className="mt-2 p-2 bg-white/5 rounded-lg border border-white/10">
              <p className="text-white/70 text-sm leading-relaxed break-words">
                {pttInterim}
              </p>
            </div>
          )}
        </motion.div>
      </div>
    );
  }

  // PTT retry mode - empty transcription
  if (pttRetryMode && !isRunning) {
    const handleRetryCancel = async () => {
      setPttRetryMode(false);
      setPttRetryScreenshot(null);
      await invoke("show_mini_window");
    };

    return (
      <div className="h-screen w-screen flex items-center justify-center p-3 bg-black/40">
        <motion.div
          initial={{ opacity: 0, y: 10 }}
          animate={{ opacity: 1, y: 0 }}
          className="w-full bg-zinc-900/95 rounded-xl border border-white/10 p-4"
        >
          <div className="flex items-center gap-2 mb-3">
            <div className="w-8 h-8 rounded-full bg-orange-500/20 flex items-center justify-center">
              <Mic size={16} className="text-orange-400" />
            </div>
            <div>
              <div className="text-white/90 text-sm font-medium">No speech detected</div>
              <div className="text-white/40 text-[11px]">Hold Cmd+Shift+V and speak</div>
            </div>
          </div>

          <div className="flex gap-2">
            <button
              onClick={handleRetryCancel}
              className="flex-1 flex items-center justify-center gap-1.5 py-2 rounded-lg bg-white/5 border border-white/10 text-white/60 hover:bg-white/10 transition-all text-[11px]"
            >
              <X size={12} />
              <span>Cancel</span>
            </button>
            <div className="flex-1 flex items-center justify-center gap-1.5 py-2 rounded-lg bg-orange-500/20 border border-orange-400/30 text-orange-300 text-[11px]">
              <RotateCcw size={12} />
              <span>Try again</span>
            </div>
          </div>
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
