import { useRef, useEffect, useState, KeyboardEvent } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Streamdown } from "streamdown";
import { useAgentStore } from "./stores/agentStore";
import { useAgent } from "./hooks/useAgent";
import { ChatMessage, ModelId } from "./types";
import {
  Send,
  Square,
  MousePointer2,
  Keyboard,
  Camera,
  ScrollText,
  AlertCircle,
  ChevronDown,
  ChevronUp,
  Monitor,
  Brain,
} from "lucide-react";

function BashBlock({ msg }: { msg: ChatMessage }) {
  const [expanded, setExpanded] = useState(true);
  const hasOutput = msg.bashOutput !== undefined;
  const isSuccess = msg.exitCode === 0;
  const isError = msg.exitCode !== undefined && msg.exitCode !== 0;

  return (
    <motion.div
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      className=""
    >
      <div className="rounded-md overflow-hidden bg-[#0d1117] border border-[#30363d]">
        {/* command with inline status */}
        <div className="px-2 py-1.5 font-mono flex items-center gap-2">
          <span className="text-[#3fb950] text-[11px] select-none">$</span>
          <span className={`text-[11px] text-[#e6edf3] break-all flex-1 ${msg.pending ? "sweep-text" : ""}`}>
            {msg.content}
          </span>
          {msg.pending && <span className="text-[8px] text-[#8b949e] animate-pulse shrink-0">...</span>}
          {hasOutput && msg.exitCode !== undefined && (
            <span className={`text-[8px] font-mono shrink-0 ${isSuccess ? "text-[#3fb950]" : "text-[#f85149]"}`}>
              {msg.exitCode}
            </span>
          )}
        </div>

        {/* output */}
        {hasOutput && (
          <>
            <div className="border-t border-[#30363d]">
              <button
                onClick={() => setExpanded(!expanded)}
                className="w-full px-2 py-0.5 flex items-center gap-1 text-[8px] text-[#8b949e] hover:text-[#c9d1d9] transition-colors"
              >
                {expanded ? <ChevronUp size={8} /> : <ChevronDown size={8} />}
                output
              </button>
            </div>
            <AnimatePresence>
              {expanded && (
                <motion.div
                  initial={{ height: 0, opacity: 0 }}
                  animate={{ height: "auto", opacity: 1 }}
                  exit={{ height: 0, opacity: 0 }}
                  className="overflow-hidden"
                >
                  <pre className={`px-2 py-1.5 text-[10px] leading-relaxed break-words whitespace-pre-wrap max-h-[120px] overflow-y-auto ${
                    isError ? "text-[#f85149]" : "text-[#8b949e]"
                  }`}>
                    {msg.bashOutput}
                  </pre>
                </motion.div>
              )}
            </AnimatePresence>
          </>
        )}
      </div>
    </motion.div>
  );
}

function MessageBubble({ msg }: { msg: ChatMessage }) {
  const isUser = msg.role === "user";

  // bash commands get their own block
  if (msg.type === "bash") {
    return <BashBlock msg={msg} />;
  }

  const getIcon = () => {
    if (isUser) return null;
    switch (msg.type) {
      case "action":
        const action = msg.action?.action;
        if (action?.includes("click") || action === "mouse_move")
          return <MousePointer2 size={14} />;
        if (action === "type") return <Keyboard size={14} />;
        if (action === "screenshot") return <Camera size={14} />;
        if (action === "scroll") return <ScrollText size={14} />;
        return <MousePointer2 size={14} />;
      case "error":
        return <AlertCircle size={14} />;
      default:
        return null;
    }
  };

  const getBubbleStyle = () => {
    if (isUser) {
      return "bg-blue-500/30 border-blue-400/30 px-3 py-2 rounded-2xl border backdrop-blur-sm";
    }
    switch (msg.type) {
      case "error":
        return "bg-red-500/20 border-red-400/30 px-3 py-2 rounded-2xl border backdrop-blur-sm";
      default:
        return "";
    }
  };

  const icon = getIcon();
  const isAction = msg.type === "action";
  const showSweep = isAction && msg.pending;

  return (
    <motion.div
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      className={`flex ${isUser ? "justify-end" : "justify-start"}`}
    >
      <div className={getBubbleStyle()}>
        <div className="flex items-start gap-2">
          {icon && <span className="mt-0.5 text-white/50">{icon}</span>}
          {msg.type === "thinking" ? (
            <div className="text-[13px] leading-relaxed text-white/90 prose prose-invert prose-sm max-w-none">
              <Streamdown isAnimating={false}>{msg.content}</Streamdown>
            </div>
          ) : (
            <p className={`text-[13px] leading-relaxed break-words ${isAction ? (msg.pending ? "text-white/50 italic" : "text-white/50") : "text-white/90"}`}>
              {showSweep && <span className="sweep-text">{msg.content}</span>}
              {isAction && !showSweep && msg.content}
              {!isAction && msg.content}
            </p>
          )}
        </div>
      </div>
    </motion.div>
  );
}

function ScreenPreview({ scrollRef }: { scrollRef: React.RefObject<HTMLDivElement | null> }) {
  const { screenshot, isRunning } = useAgentStore();
  const [collapsed, setCollapsed] = useState(false);

  // auto-collapse after 2 seconds when expanded, then scroll to bottom
  useEffect(() => {
    if (!collapsed && screenshot) {
      const timer = setTimeout(() => {
        setCollapsed(true);
        // scroll to bottom after collapse
        setTimeout(() => {
          if (scrollRef.current) {
            scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
          }
        }, 100);
      }, 2000);
      return () => clearTimeout(timer);
    }
  }, [collapsed, screenshot, scrollRef]);

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
                  src={`data:image/jpeg;base64,${screenshot}`}
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

const MODELS: { id: ModelId; label: string }[] = [
  { id: "claude-haiku-4-5-20251001", label: "Haiku 4.5" },
  { id: "claude-sonnet-4-5", label: "Sonnet 4.5" },
  { id: "claude-opus-4-5", label: "Opus 4.5" },
];

function ThinkingBubble() {
  const { streamingThinking, isRunning } = useAgentStore();
  const [expanded, setExpanded] = useState(false);

  if (!streamingThinking) return null;

  return (
    <motion.div
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      className="flex justify-start"
    >
      <div className="w-full">
        <button
          onClick={() => setExpanded(!expanded)}
          className="flex items-center gap-1.5 text-[10px] text-white/40 hover:text-white/60 transition-colors mb-1"
        >
          <Brain size={10} className={isRunning ? "animate-pulse" : ""} />
          <span>thinking</span>
          {isRunning && <span className="animate-pulse">...</span>}
          <span className="ml-1">
            {expanded ? <ChevronUp size={10} /> : <ChevronDown size={10} />}
          </span>
        </button>
        <div className={`text-[11px] leading-relaxed text-white/50 overflow-hidden transition-all ${expanded ? "max-h-[300px]" : "max-h-[60px]"} overflow-y-auto`}>
          <Streamdown isAnimating={isRunning}>{streamingThinking}</Streamdown>
        </div>
      </div>
    </motion.div>
  );
}

function StreamingBubble() {
  const { streamingText, isRunning } = useAgentStore();

  if (!streamingText) return null;

  return (
    <motion.div
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      className="flex justify-start"
    >
      <div>
        <div className="text-[13px] leading-relaxed text-white/90 prose prose-invert prose-sm max-w-none">
          <Streamdown isAnimating={isRunning}>{streamingText}</Streamdown>
        </div>
      </div>
    </motion.div>
  );
}

export default function App() {
  const { messages, isRunning, inputText, setInputText, selectedModel, setSelectedModel, streamingText, streamingThinking } = useAgentStore();
  const { toggle, submit, stop } = useAgent();
  const scrollRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  // auto-scroll on messages, streaming text, or thinking
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages, streamingText, streamingThinking]);

  // focus input on mount
  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  // esc to stop
  useEffect(() => {
    const handleEsc = (e: globalThis.KeyboardEvent) => {
      if (e.key === "Escape" && isRunning) {
        stop();
      }
    };
    window.addEventListener("keydown", handleEsc);
    return () => window.removeEventListener("keydown", handleEsc);
  }, [isRunning, stop]);

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
    <div className="h-screen flex flex-col bg-black/85 backdrop-blur-2xl overflow-hidden rounded-xl">
      {/* titlebar */}
      <div className="titlebar h-11 flex items-center justify-between px-3 border-b border-white/5 shrink-0">
        <span className="text-[11px] font-medium text-white/40 tracking-wide uppercase">
          taskhomie
        </span>
        <select
          value={selectedModel}
          onChange={(e) => setSelectedModel(e.target.value as ModelId)}
          disabled={isRunning}
          className="bg-white/5 border border-white/10 rounded-md px-2 py-1 text-[10px] text-white/60 focus:outline-none focus:border-white/20 disabled:opacity-50"
        >
          {MODELS.map((m) => (
            <option key={m.id} value={m.id} className="bg-black text-white">
              {m.label}
            </option>
          ))}
        </select>
      </div>

      {/* messages */}
      <div ref={scrollRef} className="flex-1 overflow-y-auto px-3 py-3">
        <div className="space-y-2">
          <AnimatePresence mode="popLayout">
            {messages.length === 0 && !streamingText && !streamingThinking ? (
              <motion.div
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                className="flex flex-col items-center justify-center h-full pt-24 text-white/25"
              >
                <img src="/vaporlofi.gif" alt="" className="w-[28rem] h-auto opacity-60" />
                <p className="text-sm mt-4 text-white/50">sip coffee while ai takes over your computer</p>
              </motion.div>
            ) : (
              <>
                {messages.map((msg) => <MessageBubble key={msg.id} msg={msg} />)}
                <ThinkingBubble />
                <StreamingBubble />
              </>
            )}
          </AnimatePresence>
        </div>
      </div>

      {/* screen preview */}
      <AnimatePresence>
        <ScreenPreview scrollRef={scrollRef} />
      </AnimatePresence>

      {/* input */}
      <div className="p-3 pt-0 shrink-0">
        <div className="glass-card flex items-center gap-2 p-2">
          <textarea
            ref={inputRef}
            value={inputText}
            onChange={(e) => setInputText(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={isRunning ? "running..." : "what should I do?"}
            disabled={isRunning}
            rows={1}
            className="flex-1 bg-transparent text-white text-[13px] placeholder-white/30 resize-none focus:outline-none min-h-[24px] max-h-[100px] py-1 px-1 overflow-hidden"
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
