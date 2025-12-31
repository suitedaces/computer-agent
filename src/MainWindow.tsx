import { useEffect, useReducer, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import ChatView from "./components/ChatView";
import { useAgent } from "./hooks/useAgent";
import { ChevronRight, X, Send } from "lucide-react";
import { motion } from "framer-motion";

// main window states (no voice/PTT - that's VoiceWindow)
type State =
  | { mode: "idle" }
  | { mode: "expanded" }
  | { mode: "running" }
  | { mode: "help"; screenshot: string };

type Action =
  | { type: "EXPAND" }
  | { type: "COLLAPSE" }
  | { type: "HELP"; screenshot: string }
  | { type: "HELP_CANCEL" }
  | { type: "HELP_SUBMIT" }
  | { type: "AGENT_START" }
  | { type: "AGENT_STOP" };

function reducer(state: State, action: Action): State {
  switch (action.type) {
    case "EXPAND":
      return { mode: "expanded" };
    case "COLLAPSE":
      return { mode: "idle" };
    case "HELP":
      return { mode: "help", screenshot: action.screenshot };
    case "HELP_CANCEL":
      return { mode: "idle" };
    case "HELP_SUBMIT":
      return { mode: "expanded" };
    case "AGENT_START":
      return { mode: "running" };
    case "AGENT_STOP":
      return state.mode === "running" ? { mode: "expanded" } : state;
    default:
      return state;
  }
}

// size configs
const SIZES: Record<string, { w: number; h: number }> = {
  idle: { w: 280, h: 40 },
  expanded: { w: 400, h: 520 },
  running: { w: 400, h: 520 },
  help: { w: 520, h: 420 },
};

export default function MainWindow() {
  const [state, dispatch] = useReducer(reducer, { mode: "idle" });
  const { submit } = useAgent();

  const helpPromptRef = useRef("");
  const submitRef = useRef(submit);

  useEffect(() => {
    submitRef.current = submit;
  }, [submit]);

  // sync window size/position with state
  useEffect(() => {
    const size = SIZES[state.mode];
    const centered = state.mode === "help";
    invoke("set_window_state", {
      width: size.w,
      height: size.h,
      centered,
    }).catch(console.error);
  }, [state.mode]);

  // event listeners
  useEffect(() => {
    const listeners = [
      listen("agent:started", () => dispatch({ type: "AGENT_START" })),
      listen("agent:stopped", () => dispatch({ type: "AGENT_STOP" })),

      // help mode (Cmd+Shift+H)
      listen<{ screenshot: string | null }>("hotkey-help", (e) => {
        if (e.payload.screenshot) {
          dispatch({ type: "HELP", screenshot: e.payload.screenshot });
        }
      }),
    ];

    return () => {
      listeners.forEach((p) => p.then((fn) => fn()));
    };
  }, []);

  // IDLE BAR
  if (state.mode === "idle") {
    return (
      <div
        data-tauri-drag-region
        onClick={() => dispatch({ type: "EXPAND" })}
        className="h-full w-full flex items-center gap-2 px-3 bg-black/85 backdrop-blur-xl rounded-xl border border-white/10 cursor-pointer hover:bg-black/90 transition-colors"
      >
        <img src="/windows-computer-icon.png" className="w-4 h-4 opacity-60" alt="" />
        <span className="text-xs text-white/60 flex-1">summon an agent</span>
        <ChevronRight size={12} className="text-white/40" />
      </div>
    );
  }

  // HELP MODE
  if (state.mode === "help") {
    const handleSubmit = async () => {
      const prompt = helpPromptRef.current;
      if (!prompt.trim()) return;
      dispatch({ type: "HELP_SUBMIT" });
      await submitRef.current(prompt, state.screenshot);
      helpPromptRef.current = "";
    };

    return (
      <div className="h-full w-full flex items-center justify-center p-4 bg-black/40">
        <motion.div
          initial={{ opacity: 0, scale: 0.9, y: 20 }}
          animate={{ opacity: 1, scale: 1, y: 0 }}
          className="w-full max-w-[480px] bg-gradient-to-b from-zinc-900 to-black rounded-2xl border border-white/10 overflow-hidden shadow-2xl"
        >
          {/* screenshot */}
          <div className="p-3 pb-2">
            <img
              src={`data:image/jpeg;base64,${state.screenshot}`}
              alt="Screenshot"
              className="w-full rounded-xl border border-white/5"
            />
          </div>

          {/* input */}
          <div className="px-3 pb-2">
            <input
              type="text"
              autoFocus
              onChange={(e) => (helpPromptRef.current = e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleSubmit()}
              placeholder="What do you need help with?"
              className="w-full bg-white/5 border border-white/10 rounded-xl px-4 py-3 text-sm text-white/90 placeholder:text-white/30 focus:outline-none focus:border-white/20"
            />
          </div>

          {/* buttons */}
          <div className="px-3 pb-3 flex gap-2">
            <button
              onClick={() => dispatch({ type: "HELP_CANCEL" })}
              className="flex-1 py-2.5 rounded-xl bg-red-500/20 border border-red-400/20 text-red-300 hover:bg-red-500/30 text-xs flex items-center justify-center gap-1.5"
            >
              <X size={14} /> Cancel
            </button>
            <button
              onClick={handleSubmit}
              className="flex-1 py-2.5 rounded-xl bg-blue-500/30 border border-blue-400/30 text-blue-200 hover:bg-blue-500/40 text-xs font-medium flex items-center justify-center gap-1.5"
            >
              <Send size={14} /> Send
            </button>
          </div>
        </motion.div>
      </div>
    );
  }

  // EXPANDED or RUNNING - ChatView handles everything including titlebar
  return (
    <div className="h-full w-full flex flex-col bg-black/90 backdrop-blur-xl rounded-2xl border border-white/10 overflow-hidden">
      <ChatView variant="compact" onCollapse={() => dispatch({ type: "COLLAPSE" })} />
    </div>
  );
}
