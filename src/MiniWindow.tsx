import { useEffect, useState, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { motion, AnimatePresence } from "framer-motion";
import { ChevronRight, Send, X, Volume2, Maximize2 } from "lucide-react";
import ChatView from "./components/ChatView";
import MessagesDisplay from "./components/MessagesDisplay";
import { useAgent } from "./hooks/useAgent";
import { useAgentStore } from "./stores/agentStore";
import VoiceOrb from "./components/VoiceOrb";

export default function MiniWindow() {
  const { submit } = useAgent();
  const setVoiceMode = useAgentStore((s) => s.setVoiceMode);
  const [isRunning, setIsRunning] = useState(false);
  const [helpMode, setHelpMode] = useState(false);
  const [helpPrompt, setHelpPrompt] = useState("");
  const [helpScreenshot, setHelpScreenshot] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // PTT state
  const [pttRecording, setPttRecording] = useState(false);
  const [pttInterim, setPttInterim] = useState("");
  const [pttRetryMode, setPttRetryMode] = useState(false);
  const submitRef = useRef(submit);
  const lastPttResultRef = useRef<{ text: string; at: number } | null>(null);
  const pttSessionIdRef = useRef(0);
  const pttHandledSessionRef = useRef<number | null>(null);
  const pttPhaseRef = useRef<"idle" | "recording" | "stoppedWaiting">("idle");
  const pttStoppedAtRef = useRef(0);

  // voice response mode - shows ChatView in mini window
  const [voiceResponseMode, setVoiceResponseMode] = useState(false);

  useEffect(() => {
    submitRef.current = submit;
  }, [submit]);

  // poll running state and resize window
  useEffect(() => {
    // don't poll/resize while in special modes - backend handles sizing
    if (helpMode || pttRecording || pttRetryMode || voiceResponseMode) return;

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
  }, [helpMode, pttRecording, pttRetryMode, voiceResponseMode]);

  useEffect(() => {
    const unlisten1 = listen("agent:started", () => {
      setIsRunning(true);
    });

    const unlisten2 = listen("agent:stopped", () => {
      setIsRunning(false);
      // keep voice response visible for a moment after agent stops
      // user can dismiss or expand to main
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
    const unlisten4 = listen<{ recording: boolean; sessionId?: number }>("ptt:recording", (e) => {
      console.log("[ptt] recording:", e.payload.recording, "session:", e.payload.sessionId);
      setPttRecording(e.payload.recording);
      if (e.payload.recording) {
        setPttInterim("");
        setPttRetryMode(false);
        // use backend session id for correlation
        if (e.payload.sessionId !== undefined) {
          pttSessionIdRef.current = e.payload.sessionId;
        }
        pttHandledSessionRef.current = null;
        pttPhaseRef.current = "recording";
      } else {
        pttPhaseRef.current = "stoppedWaiting";
        pttStoppedAtRef.current = Date.now();
      }
    });

    // PTT interim transcription
    const unlisten5 = listen<string>("ptt:interim", (e) => {
      console.log("[ptt] interim:", e.payload);
      setPttInterim(e.payload);
    });

    // PTT result - auto-submit or show retry
    const unlisten6 = listen<{ text: string; screenshot: string | null; mode: string | null; sessionId?: number }>("ptt:result", async (e) => {
      console.log("[ptt] result:", e.payload);
      setPttRecording(false);
      setPttInterim("");

      const { text, screenshot, mode, sessionId } = e.payload;
      // use backend session id for correlation if available
      const expectedSessionId = pttSessionIdRef.current;
      if (sessionId !== undefined && sessionId !== expectedSessionId) {
        console.log("[ptt] stale session result ignored: got", sessionId, "expected", expectedSessionId);
        return;
      }
      if (pttHandledSessionRef.current === expectedSessionId) {
        console.log("[ptt] duplicate session result ignored");
        return;
      }
      const now = Date.now();
      if (pttPhaseRef.current !== "stoppedWaiting" || now - pttStoppedAtRef.current > 3000) {
        console.log("[ptt] stale result ignored (phase or timeout)");
        return;
      }
      pttPhaseRef.current = "idle";
      pttHandledSessionRef.current = expectedSessionId;
      const last = lastPttResultRef.current;
      if (last && last.text === text && now - last.at < 1500) {
        console.log("[ptt] duplicate result ignored");
        return;
      }
      lastPttResultRef.current = { text, at: now };

      if (!text.trim()) {
        // empty transcription - show retry mode
        console.log("[ptt] empty transcription, showing retry");
        setPttRetryMode(true);

        // show mini window in retry mode
        const win = getCurrentWindow();
        await win.setSize(new LogicalSize(300, 300));
        return;
      }

      // has transcription - submit and show voice response in mini window
      try {
        // enable voice mode for TTS response when using voice input
        setVoiceMode(true);
        // enter voice response mode - stay in mini window
        setVoiceResponseMode(true);
        // resize and reposition to top right
        await invoke("position_mini_window", { width: 320, height: 280 });
        // pass mode override if not "current" (which means use UI selection)
        const modeOverride = mode && mode !== "current" ? mode : undefined;
        console.log("[ptt] submitting:", text, "mode:", modeOverride);
        await submitRef.current(text, screenshot ?? undefined, modeOverride);
      } catch (err) {
        console.error("[ptt] submit failed:", err);
      }
    });

    // listen for speak events - messages come through store via useAgent
    const unlisten8 = listen<{ audio: string; text: string }>("agent:speak", (e) => {
      console.log("[mini] speak:", e.payload.text.slice(0, 50) + "...");
      // audio is played by main window via useAgent, messages go to store
    });

    // PTT error
    const unlisten7 = listen<string>("ptt:error", (e) => {
      console.error("[ptt] error:", e.payload);
      setPttRecording(false);
      setPttInterim("");
      pttPhaseRef.current = "idle";
    });

    return () => {
      unlisten1.then((f) => f());
      unlisten2.then((f) => f());
      unlisten3.then((f) => f());
      unlisten4.then((f) => f());
      unlisten5.then((f) => f());
      unlisten6.then((f) => f());
      unlisten7.then((f) => f());
      unlisten8.then((f) => f());
    };
  }, []);

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

  // voice response mode - shows messages in mini window
  if (voiceResponseMode) {
    const handleExpandToMain = async () => {
      setVoiceResponseMode(false);
      await invoke("show_spotlight_window");
      await invoke("show_mini_window");
    };

    const handleDismiss = async () => {
      setVoiceResponseMode(false);
      await invoke("show_mini_window");
    };

    return (
      <motion.div
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        className="h-screen w-screen flex flex-col bg-black/90 backdrop-blur-xl rounded-2xl border border-white/10 overflow-hidden"
      >
        {/* mini header with actions */}
        <div className="flex items-center justify-between px-2 py-1.5 border-b border-white/5 shrink-0">
          <div className="flex items-center gap-1.5">
            <Volume2 size={12} className={`text-orange-300 ${isRunning ? "animate-pulse" : ""}`} />
            <span className="text-[10px] text-white/40">voice</span>
          </div>
          <div className="flex items-center gap-1">
            <button
              onClick={handleDismiss}
              className="px-2 py-0.5 rounded text-[10px] text-white/40 hover:text-white/60 hover:bg-white/5 transition-colors"
            >
              Dismiss
            </button>
            <button
              onClick={handleExpandToMain}
              className="px-2 py-0.5 rounded bg-white/10 text-[10px] text-white/60 hover:bg-white/15 hover:text-white/80 transition-colors flex items-center gap-1"
            >
              <Maximize2 size={10} />
            </button>
          </div>
        </div>
        {/* messages display */}
        <div className="flex-1 min-h-0 overflow-hidden p-2">
          <MessagesDisplay className="h-full" />
        </div>
      </motion.div>
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
