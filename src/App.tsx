import { useRef, useEffect, useState, KeyboardEvent } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { useAgentStore } from "./stores/agentStore";
import { useAgent } from "./hooks/useAgent";
import { ChatMessage } from "./types";
import {
  Send,
  Square,
  MousePointer2,
  Keyboard,
  Camera,
  ScrollText,
  MessageSquare,
  AlertCircle,
  CheckCircle,
  Monitor,
  ChevronDown,
  ChevronUp,
} from "lucide-react";

function MessageBubble({ msg }: { msg: ChatMessage }) {
  const isUser = msg.role === "user";

  const getIcon = () => {
    if (isUser) return null;
    switch (msg.type) {
      case "action":
        const action = msg.action?.action;
        if (action?.includes("click") || action === "mouse_move")
          return <MousePointer2 size={12} />;
        if (action === "type") return <Keyboard size={12} />;
        if (action === "screenshot") return <Camera size={12} />;
        if (action === "scroll") return <ScrollText size={12} />;
        return <MousePointer2 size={12} />;
      case "error":
        return <AlertCircle size={12} />;
      case "info":
        return <CheckCircle size={12} />;
      default:
        return null;
    }
  };

  const getBubbleStyle = () => {
    if (isUser) {
      return "bg-blue-500/30 border-blue-400/30 ml-8 px-3 py-2 rounded-2xl border backdrop-blur-sm";
    }
    switch (msg.type) {
      case "error":
        return "bg-red-500/20 border-red-400/30 mr-8 px-3 py-2 rounded-2xl border backdrop-blur-sm";
      case "action":
        return "bg-purple-500/20 border-purple-400/30 mr-8 px-3 py-2 rounded-2xl border backdrop-blur-sm";
      default:
        return "mr-8";
    }
  };

  const icon = getIcon();

  return (
    <motion.div
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      className={`flex ${isUser ? "justify-end" : "justify-start"}`}
    >
      <div className={`max-w-[85%] ${getBubbleStyle()}`}>
        <div className="flex items-start gap-2">
          {icon && <span className="mt-0.5 text-white/50">{icon}</span>}
          <p className="text-[13px] text-white/90 leading-relaxed break-words">
            {msg.content}
          </p>
        </div>
      </div>
    </motion.div>
  );
}

function ScreenPreview() {
  const { screenshot, isRunning } = useAgentStore();
  const [collapsed, setCollapsed] = useState(false);

  if (!screenshot) return null;

  return (
    <motion.div
      initial={{ opacity: 0, height: 0 }}
      animate={{ opacity: 1, height: "auto" }}
      exit={{ opacity: 0, height: 0 }}
      className="mx-3 mb-2"
    >
      <div className="glass-card p-2 relative">
        <button
          onClick={() => setCollapsed(!collapsed)}
          className="flex items-center gap-1.5 w-full text-[10px] text-white/40 hover:text-white/60 transition-colors"
        >
          <Monitor size={10} />
          <span>live</span>
          {isRunning && <span className="status-dot running" />}
          <span className="ml-auto">
            {collapsed ? <ChevronDown size={10} /> : <ChevronUp size={10} />}
          </span>
        </button>
        <AnimatePresence>
          {!collapsed && (
            <motion.div
              initial={{ opacity: 0, height: 0 }}
              animate={{ opacity: 1, height: "auto" }}
              exit={{ opacity: 0, height: 0 }}
              className="mt-1.5"
            >
              <div className="rounded-lg overflow-hidden bg-black/40">
                <img
                  src={`data:image/png;base64,${screenshot}`}
                  alt="Screen"
                  className="w-full h-auto"
                />
              </div>
            </motion.div>
          )}
        </AnimatePresence>
      </div>
    </motion.div>
  );
}

export default function App() {
  const { messages, isRunning, inputText, setInputText } = useAgentStore();
  const { toggle, submit } = useAgent();
  const scrollRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  // auto-scroll
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages]);

  // focus input on mount
  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      if (isRunning) {
        toggle(); // stop
      } else if (inputText.trim()) {
        submit();
      }
    }
  };

  const canSubmit = inputText.trim().length > 0 || isRunning;

  return (
    <div className="h-screen flex flex-col bg-black/85 backdrop-blur-2xl overflow-hidden">
      {/* titlebar */}
      <div className="titlebar h-11 flex items-center justify-center border-b border-white/5 shrink-0">
        <span className="text-[11px] font-medium text-white/40 tracking-wide uppercase">
          Grunty
        </span>
      </div>

      {/* messages */}
      <div ref={scrollRef} className="flex-1 overflow-y-auto px-3 py-3">
        <div className="space-y-2">
          <AnimatePresence mode="popLayout">
            {messages.length === 0 ? (
              <motion.div
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                className="text-center py-16 text-white/25"
              >
                <Monitor size={28} className="mx-auto mb-3 opacity-40" />
                <p className="text-sm">what should I do?</p>
              </motion.div>
            ) : (
              messages.map((msg) => <MessageBubble key={msg.id} msg={msg} />)
            )}
          </AnimatePresence>
        </div>
      </div>

      {/* screen preview */}
      <AnimatePresence>
        <ScreenPreview />
      </AnimatePresence>

      {/* input */}
      <div className="p-3 pt-0 shrink-0">
        <div className="glass-card flex items-end gap-2 p-2">
          <textarea
            ref={inputRef}
            value={inputText}
            onChange={(e) => setInputText(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={isRunning ? "running..." : "what should I do?"}
            disabled={isRunning}
            rows={1}
            className="flex-1 bg-transparent text-white text-[13px] placeholder-white/30 resize-none focus:outline-none min-h-[24px] max-h-[100px] py-1 px-1"
            style={{ height: "24px" }}
            onInput={(e) => {
              const target = e.target as HTMLTextAreaElement;
              target.style.height = "24px";
              target.style.height = Math.min(target.scrollHeight, 100) + "px";
            }}
          />
          <motion.button
            onClick={toggle}
            disabled={!canSubmit}
            whileHover={{ scale: 1.05 }}
            whileTap={{ scale: 0.95 }}
            className={`shrink-0 w-8 h-8 rounded-xl flex items-center justify-center transition-colors ${
              isRunning
                ? "bg-red-500/30 border border-red-400/30 text-red-300"
                : canSubmit
                ? "bg-blue-500/30 border border-blue-400/30 text-blue-300"
                : "bg-white/5 border border-white/10 text-white/20"
            }`}
          >
            {isRunning ? <Square size={14} /> : <Send size={14} />}
          </motion.button>
        </div>
      </div>
    </div>
  );
}
