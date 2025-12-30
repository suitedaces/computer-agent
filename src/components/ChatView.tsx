import { useRef, useEffect, useState, KeyboardEvent } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Streamdown } from "streamdown";
import { useAgentStore } from "../stores/agentStore";
import { useAgent } from "../hooks/useAgent";
import { ChatMessage, ConversationMeta, Conversation, ModelId, AgentMode } from "../types";
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
  Brain,
  Clock,
  X,
  Maximize2,
  Minimize2,
  Monitor,
  Plus,
  MessageSquare,
  Trash2,
} from "lucide-react";
import { invoke } from "@tauri-apps/api/core";

interface ChatViewProps {
  variant: "sidebar" | "spotlight" | "mini";
}

function BashBlock({ msg }: { msg: ChatMessage }) {
  const [expanded, setExpanded] = useState(true);
  const hasOutput = msg.bashOutput !== undefined;
  const isSuccess = msg.exitCode === 0;
  const isError = msg.exitCode !== undefined && msg.exitCode !== 0;

  return (
    <motion.div
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
    >
      <div className="rounded-md overflow-hidden bg-[#0d1117] border border-[#30363d]">
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

function ScreenshotBlock({ msg }: { msg: ChatMessage }) {
  const [expanded, setExpanded] = useState(false);
  const prevScreenshot = useRef(msg.screenshot);

  useEffect(() => {
    if (msg.screenshot && !prevScreenshot.current) {
      setExpanded(true);
      const timer = setTimeout(() => setExpanded(false), 2000);
      return () => clearTimeout(timer);
    }
    prevScreenshot.current = msg.screenshot;
  }, [msg.screenshot]);

  return (
    <motion.div
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      className="flex justify-start"
    >
      <div>
        <button
          onClick={() => msg.screenshot && setExpanded(!expanded)}
          className="flex items-center gap-2 text-white/50 hover:text-white/70 transition-colors"
        >
          <Camera size={14} />
          <span className={`text-[13px] ${msg.pending ? "sweep-text italic" : ""}`}>
            {msg.pending ? "Taking screenshot" : "Took screenshot"}
          </span>
          {msg.screenshot && (
            <span className="text-white/30">
              {expanded ? <ChevronUp size={10} /> : <ChevronDown size={10} />}
            </span>
          )}
        </button>
        <AnimatePresence>
          {expanded && msg.screenshot && (
            <motion.div
              initial={{ height: 0, opacity: 0 }}
              animate={{ height: "auto", opacity: 1 }}
              exit={{ height: 0, opacity: 0 }}
              className="overflow-hidden mt-1.5"
            >
              <div className="rounded-lg overflow-hidden bg-black/40">
                <img
                  src={`data:image/jpeg;base64,${msg.screenshot}`}
                  alt="Screenshot"
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

function formatActionContent(msg: ChatMessage): React.ReactNode {
  const action = msg.action;
  if (!action) return msg.content;

  const coord = action.coordinate;
  const actionType = action.action;

  if (actionType === "left_click" && coord) {
    const label = msg.pending ? "Clicking" : "Clicked";
    return <>{label} <sub className="text-[10px] opacity-60">({coord[0]}, {coord[1]})</sub></>;
  }
  if (actionType === "double_click" && coord) {
    const label = msg.pending ? "Double clicking" : "Double clicked";
    return <>{label} <sub className="text-[10px] opacity-60">({coord[0]}, {coord[1]})</sub></>;
  }
  if (actionType === "mouse_move" && coord) {
    const label = msg.pending ? "Moving to" : "Moved to";
    return <>{label} <sub className="text-[10px] opacity-60">({coord[0]}, {coord[1]})</sub></>;
  }

  return msg.content;
}

function MessageBubble({ msg }: { msg: ChatMessage }) {
  const isUser = msg.role === "user";

  if (msg.type === "bash") {
    return <BashBlock msg={msg} />;
  }

  if (msg.type === "action" && msg.action?.action === "screenshot") {
    return <ScreenshotBlock msg={msg} />;
  }

  const getIcon = () => {
    if (isUser) return null;
    switch (msg.type) {
      case "action":
        const action = msg.action?.action;
        if (action?.includes("click") || action === "mouse_move" || action === "left_click_drag")
          return <MousePointer2 size={14} />;
        if (action === "type" || action === "key") return <Keyboard size={14} />;
        if (action === "scroll") return <ScrollText size={14} />;
        if (action === "wait") return <Clock size={14} />;
        return <MousePointer2 size={14} />;
      case "error":
        return <AlertCircle size={14} className="text-red-400" />;
      default:
        return null;
    }
  };

  const getBubbleStyle = () => {
    if (isUser) {
      return "bg-blue-500/30 border-blue-400/30 px-3 py-2 rounded-2xl border backdrop-blur-sm";
    }
    return "";
  };

  const icon = getIcon();
  const isAction = msg.type === "action";
  const isError = msg.type === "error";
  const showSweep = isAction && msg.pending;

  return (
    <motion.div
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      className={`flex ${isUser ? "justify-end" : "justify-start"}`}
    >
      <div className={getBubbleStyle()}>
        {/* user message with screenshot */}
        {isUser && msg.screenshot && (
          <div className="mb-2 rounded-lg overflow-hidden">
            <img
              src={`data:image/jpeg;base64,${msg.screenshot}`}
              alt="Context"
              className="w-full max-w-[300px] h-auto rounded-lg"
            />
          </div>
        )}
        <div className="flex items-start gap-2">
          {icon && <span className="mt-0.5 text-white/50">{icon}</span>}
          {msg.type === "thinking" || msg.type === "info" ? (
            <div className="text-[13px] leading-relaxed prose prose-invert prose-sm max-w-none text-white/90">
              <Streamdown isAnimating={false}>{msg.content}</Streamdown>
            </div>
          ) : (
            <p className={`text-[13px] leading-relaxed break-words ${
              isError ? "text-red-400" :
              isAction ? (msg.pending ? "text-white/50 italic" : "text-white/50") :
              "text-white/90"
            }`}>
              {showSweep && <span className="sweep-text">{formatActionContent(msg)}</span>}
              {isAction && !showSweep && formatActionContent(msg)}
              {!isAction && msg.content}
            </p>
          )}
        </div>
      </div>
    </motion.div>
  );
}

const MODELS: { id: ModelId; label: string }[] = [
  { id: "claude-haiku-4-5-20251001", label: "Haiku 4.5" },
  { id: "claude-sonnet-4-5", label: "Sonnet 4.5" },
  { id: "claude-opus-4-5", label: "Opus 4.5" },
];

function formatRelativeTime(timestamp: number): string {
  const now = Date.now() / 1000;
  const diff = now - timestamp;
  if (diff < 60) return "now";
  if (diff < 3600) return `${Math.floor(diff / 60)}m`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h`;
  if (diff < 604800) return `${Math.floor(diff / 86400)}d`;
  return new Date(timestamp * 1000).toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

// convert anthropic api messages to chat messages for display
function convertApiToChat(conversation: Conversation): ChatMessage[] {
  const chatMessages: ChatMessage[] = [];

  for (const msg of conversation.messages) {
    // find first text block for main content
    let mainText = "";
    let screenshot: string | undefined;

    for (const block of msg.content) {
      if (block.type === "text" && block.text) {
        mainText = block.text;
        break;
      }
      if (block.type === "image" && block.source?.data) {
        screenshot = block.source.data;
      }
    }

    if (msg.role === "user") {
      chatMessages.push({
        id: crypto.randomUUID(),
        role: "user",
        content: mainText,
        timestamp: new Date(conversation.updated_at * 1000),
        screenshot,
      });
    } else {
      // assistant messages - extract text, thinking, tool uses
      for (const block of msg.content) {
        if (block.type === "thinking" && block.thinking) {
          chatMessages.push({
            id: crypto.randomUUID(),
            role: "assistant",
            content: block.thinking,
            timestamp: new Date(conversation.updated_at * 1000),
            type: "thinking",
          });
        } else if (block.type === "text" && block.text) {
          chatMessages.push({
            id: crypto.randomUUID(),
            role: "assistant",
            content: block.text,
            timestamp: new Date(conversation.updated_at * 1000),
          });
        } else if (block.type === "tool_use" && block.name) {
          const input = block.input as Record<string, unknown> | undefined;
          if (block.name === "bash" && input?.command) {
            chatMessages.push({
              id: crypto.randomUUID(),
              role: "assistant",
              content: String(input.command),
              timestamp: new Date(conversation.updated_at * 1000),
              type: "bash",
              pending: false,
            });
          } else if (block.name === "computer" && input?.action) {
            chatMessages.push({
              id: crypto.randomUUID(),
              role: "assistant",
              content: `${input.action}`,
              timestamp: new Date(conversation.updated_at * 1000),
              type: "action",
              action: input as unknown as ChatMessage["action"],
              pending: false,
            });
          }
        }
      }
    }
  }

  return chatMessages;
}

interface HistoryDropdownProps {
  onNewChat: () => void;
  onLoad: (messages: ChatMessage[], model: ModelId, mode: AgentMode) => void;
  disabled?: boolean;
}

function HistoryDropdown({ onNewChat, onLoad, disabled }: HistoryDropdownProps) {
  const [open, setOpen] = useState(false);
  const [conversations, setConversations] = useState<ConversationMeta[]>([]);
  const [loading, setLoading] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (open) {
      setLoading(true);
      invoke<ConversationMeta[]>("list_conversations", { limit: 20, offset: 0 })
        .then(setConversations)
        .catch(console.error)
        .finally(() => setLoading(false));
    }
  }, [open]);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    if (open) document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [open]);

  const handleDelete = async (e: React.MouseEvent, id: string) => {
    e.stopPropagation();
    await invoke("delete_conversation", { id });
    setConversations(prev => prev.filter(c => c.id !== id));
  };

  const handleLoad = async (id: string) => {
    try {
      const conv = await invoke<Conversation | null>("load_conversation", { id });
      if (conv) {
        const chatMessages = convertApiToChat(conv);
        const model = conv.model as ModelId;
        const mode = conv.mode as AgentMode;
        onLoad(chatMessages, model, mode);
      }
    } catch (e) {
      console.error("Failed to load conversation:", e);
    }
    setOpen(false);
  };

  return (
    <div ref={dropdownRef} className="relative">
      <button
        onClick={() => !disabled && setOpen(!open)}
        disabled={disabled}
        className={`flex items-center gap-1.5 text-[11px] font-medium tracking-wide uppercase transition-colors ${
          disabled ? "text-white/20 cursor-not-allowed" : "text-white/40 hover:text-white/70"
        }`}
      >
        <span>taskhomie</span>
        <ChevronDown size={10} className={`transition-transform ${open ? "rotate-180" : ""}`} />
      </button>

      <AnimatePresence>
        {open && (
          <motion.div
            initial={{ opacity: 0, y: -4, scale: 0.96 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: -4, scale: 0.96 }}
            transition={{ duration: 0.12 }}
            className="absolute top-full left-0 mt-2 w-64 z-50 rounded-lg overflow-hidden"
            style={{
              background: "rgba(0, 0, 0, 0.92)",
              backdropFilter: "blur(24px) saturate(180%)",
              border: "1px solid rgba(255, 255, 255, 0.12)",
              boxShadow: "0 8px 32px rgba(0, 0, 0, 0.5), inset 0 1px 0 rgba(255, 255, 255, 0.08)",
            }}
          >
            {/* new chat button */}
            <button
              onClick={() => { onNewChat(); setOpen(false); }}
              className="w-full flex items-center gap-2 px-3 py-2.5 text-[11px] text-white/70 hover:bg-white/8 transition-colors border-b border-white/8"
            >
              <Plus size={12} className="text-blue-400" />
              <span>New conversation</span>
            </button>

            {/* conversation list */}
            <div className="max-h-[280px] overflow-y-auto">
              {loading ? (
                <div className="px-3 py-4 text-[10px] text-white/30 text-center">Loading...</div>
              ) : conversations.length === 0 ? (
                <div className="px-3 py-4 text-[10px] text-white/30 text-center">No conversations yet</div>
              ) : (
                conversations.map((conv, i) => (
                  <motion.div
                    key={conv.id}
                    initial={{ opacity: 0, x: -8 }}
                    animate={{ opacity: 1, x: 0 }}
                    transition={{ delay: i * 0.02 }}
                    className="group flex items-center gap-2 px-3 py-2 hover:bg-white/6 transition-colors cursor-pointer"
                    onClick={() => handleLoad(conv.id)}
                  >
                    <MessageSquare size={11} className="text-white/25 shrink-0" />
                    <div className="flex-1 min-w-0">
                      <p className="text-[11px] text-white/80 truncate">{conv.title || "Untitled"}</p>
                      <p className="text-[9px] text-white/30">{formatRelativeTime(conv.updated_at)} · {conv.message_count} msgs</p>
                    </div>
                    <button
                      onClick={(e) => handleDelete(e, conv.id)}
                      className="opacity-0 group-hover:opacity-100 p-1 rounded hover:bg-red-500/20 transition-all"
                    >
                      <Trash2 size={10} className="text-red-400/70" />
                    </button>
                  </motion.div>
                ))
              )}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

function ThinkingBubble() {
  const { streamingThinking, isRunning } = useAgentStore();
  const [expanded, setExpanded] = useState(false);
  const thinkingScrollRef = useRef<HTMLDivElement>(null);

  // auto-scroll thinking content to bottom
  useEffect(() => {
    if (!streamingThinking || !isRunning) return;
    const frame = requestAnimationFrame(() => {
      if (thinkingScrollRef.current) {
        thinkingScrollRef.current.scrollTop = thinkingScrollRef.current.scrollHeight;
      }
    });
    return () => cancelAnimationFrame(frame);
  }, [streamingThinking, isRunning]);

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
        <div
          ref={thinkingScrollRef}
          className={`text-[11px] leading-relaxed text-white/50 overflow-hidden transition-all ${expanded ? "max-h-[300px]" : "max-h-[60px]"} overflow-y-auto`}
        >
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

export default function ChatView({ variant }: ChatViewProps) {
  const { messages, isRunning, inputText, setInputText, selectedModel, setSelectedModel, selectedMode, setSelectedMode, streamingText, streamingThinking, clearMessages, setMessages } = useAgentStore();
  const { submit } = useAgent();
  const scrollRef = useRef<HTMLDivElement>(null);
  const bottomRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  const isSpotlight = variant === "spotlight";
  const isMini = variant === "mini";
  const panelClass = isMini ? "mini-panel" : isSpotlight ? "spotlight-panel" : "app-panel";
  const padding = isMini ? "px-2 py-2" : isSpotlight ? "px-4 py-4" : "px-3 py-3";
  const inputPadding = isMini ? "p-2 pt-0" : isSpotlight ? "p-4 pt-0" : "p-3 pt-0";
  const gifSize = isMini ? "w-[16rem]" : isSpotlight ? "w-[32rem]" : "w-[28rem]";

  // auto-scroll on new messages
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  // auto-scroll during streaming (throttled)
  useEffect(() => {
    if (!streamingText && !streamingThinking) return;
    const frame = requestAnimationFrame(() => {
      if (scrollRef.current) {
        scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
      }
    });
    return () => cancelAnimationFrame(frame);
  }, [streamingText, streamingThinking]);

  // auto-scroll when agent finishes
  useEffect(() => {
    if (!isRunning) {
      bottomRef.current?.scrollIntoView({ behavior: "smooth" });
    }
  }, [isRunning]);

  // focus input on mount
  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      if (inputText.trim()) {
        submit();
      }
    }
    if (e.key === "Escape" && isSpotlight) {
      invoke("hide_spotlight_window").catch(() => {});
    }
  };

  const handleToggleView = () => {
    if (isSpotlight) {
      invoke("hide_spotlight_window");
      invoke("show_main_window");
    } else {
      invoke("hide_main_window");
      invoke("show_spotlight_window");
    }
  };

  return (
    <motion.div
      initial={{ opacity: 0, scale: isSpotlight ? 0.98 : 1 }}
      animate={{ opacity: 1, scale: 1 }}
      transition={{ duration: 0.12, ease: "easeOut" }}
      className={`h-screen flex flex-col ${panelClass} overflow-hidden`}
    >
      {/* titlebar - hidden for mini */}
      {!isMini && (
        <div className="titlebar h-11 flex items-center justify-between px-3 border-b border-white/5 shrink-0">
          <HistoryDropdown
              onNewChat={() => clearMessages()}
              onLoad={(msgs, model, mode) => {
                setMessages(msgs);
                setSelectedModel(model);
                setSelectedMode(mode);
              }}
              disabled={isRunning}
            />
          <div className="flex items-center gap-2">
            {/* take over toggle - switches to computer mode */}
            <button
              onClick={() => setSelectedMode(selectedMode === "computer" ? "browser" : "computer")}
              disabled={isRunning}
              className={`flex items-center gap-1.5 px-2 py-1 rounded-md text-[10px] transition-colors border ${
                selectedMode === "computer"
                  ? "bg-orange-500/20 border-orange-400/40 text-orange-300"
                  : "bg-white/5 border-white/10 text-white/40 hover:text-white/60 hover:border-white/20"
              } ${isRunning ? "opacity-50 cursor-not-allowed" : ""}`}
              title={selectedMode === "computer" ? "Computer control active" : "Enable computer control"}
            >
              <Monitor size={12} />
              <span>Take over</span>
            </button>
            <select
              value={selectedModel}
              onChange={(e) => setSelectedModel(e.target.value as ModelId)}
              disabled={isRunning}
              className="glass-select"
            >
              {MODELS.map((m) => (
                <option key={m.id} value={m.id}>
                  {m.label}
                </option>
              ))}
            </select>
            <button
              onClick={handleToggleView}
              className="w-7 h-7 flex items-center justify-center rounded-md text-white/40 hover:text-white/70 hover:bg-white/10 transition-colors"
              title={isSpotlight ? "Switch to sidebar" : "Switch to spotlight"}
            >
              {isSpotlight ? <Minimize2 size={14} /> : <Maximize2 size={14} />}
            </button>
            <button
              onClick={() => {
                if (isSpotlight) invoke("hide_spotlight_window");
                invoke("minimize_to_mini");
              }}
              className="w-7 h-7 flex items-center justify-center rounded-md text-red-400/60 hover:text-red-400 hover:bg-red-500/10 transition-colors"
            title="Collapse"
          >
            <X size={16} />
            </button>
          </div>
        </div>
      )}

      {/* messages */}
      <div ref={scrollRef} className={`flex-1 overflow-y-auto ${padding}`}>
        <div className={messages.length === 0 && !streamingText && !streamingThinking ? "h-full" : "space-y-2"}>
          <AnimatePresence mode="popLayout">
            {messages.length === 0 && !streamingText && !streamingThinking ? (
              <motion.div
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                className="flex flex-col items-center justify-center h-full text-white/25"
              >
                <img src="/vaporlofi.gif" alt="" className={`${gifSize} h-auto opacity-60`} />
                <p className="text-sm mt-4 text-white/50">sip coffee while ai takes over your computer</p>
              </motion.div>
            ) : (
              <>
                {messages.map((msg) => <MessageBubble key={msg.id} msg={msg} />)}
                <ThinkingBubble />
                <StreamingBubble />
                <div ref={bottomRef} />
              </>
            )}
          </AnimatePresence>
        </div>
      </div>

      {/* input or stop hint */}
      <div className={`${inputPadding} shrink-0`}>
        {isRunning ? (
          <div className="glass-card flex items-center justify-center gap-2 p-3 text-red-300/70">
            <Square size={14} />
            <span className="text-[12px]">⌘⇧S to stop</span>
          </div>
        ) : (
          <div className="glass-card flex items-center gap-2 p-2">
            <textarea
              ref={inputRef}
              value={inputText}
              onChange={(e) => setInputText(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="what should I do?"
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
              onClick={() => submit()}
              disabled={!inputText.trim()}
              whileHover={{ scale: 1.05 }}
              whileTap={{ scale: 0.95 }}
              className={`shrink-0 w-8 h-8 rounded-xl flex items-center justify-center transition-colors ${
                inputText.trim()
                  ? "bg-blue-500/30 border border-blue-400/30 text-blue-300"
                  : "bg-white/5 border border-white/10 text-white/20"
              }`}
            >
              <Send size={14} />
            </motion.button>
          </div>
        )}
      </div>
    </motion.div>
  );
}
