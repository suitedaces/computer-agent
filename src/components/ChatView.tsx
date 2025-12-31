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
  ChevronLeft,
  Brain,
  Clock,
  X,
  Maximize2,
  Minimize2,
  Plus,
  MessageSquare,
  Trash2,
  Globe,
  Hand,
  Navigation,
  Layers,
  MousePointerClick,
  FormInput,
  RotateCw,
  PanelTop,
  FileUp,
  MessageCircle,
  Eye,
  GripHorizontal,
  Mic,
  MicOff,
  Settings,
  Volume2,
  Play,
  Pause,
} from "lucide-react";
import SettingsContent from "./SettingsContent";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { createAudioElement } from "../utils/audio";

// url pill component showing domain and path
function UrlLink({ url }: { url: string }) {
  let domain = "";
  let path = "";
  try {
    const parsed = new URL(url);
    domain = parsed.hostname.replace(/^www\./, "");
    path = parsed.pathname !== "/" ? parsed.pathname : "";
    if (path.length > 20) path = path.slice(0, 20) + "...";
  } catch {
    domain = url.length > 30 ? url.slice(0, 30) + "..." : url;
  }

  return (
    <span className="inline-flex items-center gap-1 px-1.5 py-0.5 bg-white/10 rounded text-[11px] font-mono">
      <Globe size={10} className="opacity-50" />
      <span className="opacity-70">{domain}</span>
      {path && <span className="opacity-40">{path}</span>}
    </span>
  );
}

// format action for display (past tense for loaded history)
// stores url in format "action||url" when url is present for later rendering
function formatActionForHistory(name: string, input: Record<string, unknown>): string {
  // computer tool actions
  if (name === "computer") {
    const action = input.action as string;
    const coord = input.coordinate as [number, number] | undefined;
    const text = input.text as string | undefined;

    switch (action) {
      case "screenshot":
        return "Took screenshot";
      case "mouse_move":
        return coord ? `Moved mouse to (${coord[0]}, ${coord[1]})` : "Moved mouse";
      case "left_click":
        return coord ? `Clicked at (${coord[0]}, ${coord[1]})` : "Left clicked";
      case "right_click":
        return "Right clicked";
      case "double_click":
        return coord ? `Double clicked at (${coord[0]}, ${coord[1]})` : "Double clicked";
      case "type":
        if (text) {
          const preview = text.length > 30 ? `${text.slice(0, 30)}...` : text;
          return `Typed: "${preview}"`;
        }
        return "Typed text";
      case "key":
        return text ? `Pressed key: ${text}` : "Pressed key";
      case "scroll": {
        const dir = input.scroll_direction as string || "down";
        return `Scrolled ${dir}`;
      }
      case "wait":
        return "Waited";
      default:
        return action;
    }
  }

  // browser tool actions
  switch (name) {
    case "take_snapshot":
      return "Took snapshot";
    case "click": {
      const dbl = input.dblClick as boolean;
      return dbl ? "Double clicked" : "Clicked";
    }
    case "hover":
      return "Hovered";
    case "fill": {
      const val = input.value as string | undefined;
      if (val) {
        const preview = val.length > 20 ? `${val.slice(0, 20)}...` : val;
        return `Filled: "${preview}"`;
      }
      return "Filled field";
    }
    case "press_key": {
      const key = input.key as string | undefined;
      return key ? `Pressed ${key}` : "Pressed key";
    }
    case "navigate_page": {
      const type = input.type as string | undefined;
      switch (type) {
        case "goto": {
          const url = input.url as string | undefined;
          if (url) {
            return `Navigated to ||${url}||`;
          }
          return "Navigated";
        }
        case "back":
          return "Went back";
        case "forward":
          return "Went forward";
        case "reload":
          return "Reloaded page";
        default:
          return "Navigated";
      }
    }
    case "wait_for": {
      const text = input.text as string | undefined;
      if (text) {
        const preview = text.length > 20 ? `${text.slice(0, 20)}...` : text;
        return `Waited for "${preview}"`;
      }
      return "Waited";
    }
    case "new_page": {
      const url = input.url as string | undefined;
      if (url) {
        return `Opened new tab ||${url}||`;
      }
      return "Opened new tab";
    }
    case "list_pages":
      return "Listed tabs";
    case "select_page": {
      const idx = input.pageIdx as number | undefined;
      return idx !== undefined ? `Switched to tab ${idx}` : "Switched tab";
    }
    case "close_page": {
      const idx = input.pageIdx as number | undefined;
      return idx !== undefined ? `Closed tab ${idx}` : "Closed tab";
    }
    case "drag":
      return "Dragged";
    case "fill_form":
      return "Filled form";
    case "handle_dialog": {
      const action = input.action as string | undefined;
      switch (action) {
        case "accept":
          return "Accepted dialog";
        case "dismiss":
          return "Dismissed dialog";
        default:
          return "Handled dialog";
      }
    }
    case "screenshot":
      return "Took screenshot";
    case "upload_file":
      return "Uploaded file";
    default:
      // fallback: convert snake_case to readable
      return name.replace(/_/g, " ").replace(/\b\w/g, c => c.toUpperCase());
  }
}

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

function SpeakBubble({ msg }: { msg: ChatMessage }) {
  const [isPlaying, setIsPlaying] = useState(false);
  const audioRef = useRef<HTMLAudioElement | null>(null);

  const togglePlay = () => {
    if (!msg.audioData) return;

    if (!audioRef.current) {
      audioRef.current = createAudioElement(msg.audioData);
      audioRef.current.onended = () => setIsPlaying(false);
    }

    const audio = audioRef.current;
    if (isPlaying) {
      audio.pause();
      audio.currentTime = 0;
      setIsPlaying(false);
    } else {
      audio.play();
      setIsPlaying(true);
    }
  };

  return (
    <motion.div
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      className="flex justify-start"
    >
      <div className="flex items-start gap-2 max-w-full">
        <motion.button
          onClick={togglePlay}
          whileHover={{ scale: 1.1 }}
          whileTap={{ scale: 0.95 }}
          className={`shrink-0 w-8 h-8 rounded-full flex items-center justify-center transition-colors ${
            isPlaying
              ? "bg-orange-500/30 border border-orange-400/30 text-orange-300"
              : "bg-white/10 border border-white/20 text-white/60 hover:text-white/80"
          }`}
        >
          {isPlaying ? <Pause size={14} /> : <Play size={14} className="ml-0.5" />}
        </motion.button>
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-1.5 mb-0.5">
            <Volume2 size={12} className="text-orange-300" />
            <span className="text-[10px] text-white/40">Voice response</span>
          </div>
          <p className="text-[13px] text-white/80 leading-relaxed">{msg.content}</p>
        </div>
      </div>
    </motion.div>
  );
}

// render content with embedded URLs as clickable links
function renderContentWithUrls(content: string): React.ReactNode {
  const urlPattern = /\|\|(.+?)\|\|/g;
  const parts: React.ReactNode[] = [];
  let lastIndex = 0;
  let match;

  while ((match = urlPattern.exec(content)) !== null) {
    if (match.index > lastIndex) {
      parts.push(content.slice(lastIndex, match.index));
    }
    parts.push(<UrlLink key={match.index} url={match[1]} />);
    lastIndex = match.index + match[0].length;
  }

  if (lastIndex < content.length) {
    parts.push(content.slice(lastIndex));
  }

  return parts.length > 0 ? <>{parts}</> : content;
}

function formatActionContent(msg: ChatMessage): React.ReactNode {
  const action = msg.action;

  // handle browser tools (no action object, content has the formatted text)
  if (!action) {
    return renderContentWithUrls(msg.content);
  }

  const coord = action.coordinate;
  const actionType = action.action;
  const text = action.text;

  // format computer actions with nice styling
  switch (actionType) {
    case "left_click":
      if (coord) {
        const label = msg.pending ? "Clicking" : "Clicked";
        return <>{label} <sub className="text-[10px] opacity-60">({coord[0]}, {coord[1]})</sub></>;
      }
      return msg.pending ? "Clicking" : "Clicked";

    case "right_click":
      return msg.pending ? "Right clicking" : "Right clicked";

    case "middle_click":
      return msg.pending ? "Middle clicking" : "Middle clicked";

    case "double_click":
      if (coord) {
        const label = msg.pending ? "Double clicking" : "Double clicked";
        return <>{label} <sub className="text-[10px] opacity-60">({coord[0]}, {coord[1]})</sub></>;
      }
      return msg.pending ? "Double clicking" : "Double clicked";

    case "triple_click":
      return msg.pending ? "Triple clicking" : "Triple clicked";

    case "mouse_move":
      if (coord) {
        const label = msg.pending ? "Moving to" : "Moved to";
        return <>{label} <sub className="text-[10px] opacity-60">({coord[0]}, {coord[1]})</sub></>;
      }
      return msg.pending ? "Moving mouse" : "Moved mouse";

    case "left_click_drag":
      if (action.start_coordinate && coord) {
        const label = msg.pending ? "Dragging" : "Dragged";
        return <>{label} <sub className="text-[10px] opacity-60">({action.start_coordinate[0]}, {action.start_coordinate[1]}) → ({coord[0]}, {coord[1]})</sub></>;
      }
      return msg.pending ? "Dragging" : "Dragged";

    case "type":
      if (text) {
        const preview = text.length > 30 ? `${text.slice(0, 30)}...` : text;
        return msg.pending ? `Typing: "${preview}"` : `Typed: "${preview}"`;
      }
      return msg.pending ? "Typing" : "Typed";

    case "key":
      if (text) {
        return msg.pending ? `Pressing ${text}` : `Pressed ${text}`;
      }
      return msg.pending ? "Pressing key" : "Pressed key";

    case "scroll": {
      const dir = action.scroll_direction || "down";
      return msg.pending ? `Scrolling ${dir}` : `Scrolled ${dir}`;
    }

    case "wait":
      return msg.pending ? "Waiting" : "Waited";

    case "screenshot":
      return msg.pending ? "Taking screenshot" : "Took screenshot";

    default:
      return renderContentWithUrls(msg.content);
  }
}

function MessageBubble({ msg }: { msg: ChatMessage }) {
  const isUser = msg.role === "user";

  if (msg.type === "bash") {
    return <BashBlock msg={msg} />;
  }

  if (msg.type === "action" && msg.action?.action === "screenshot") {
    return <ScreenshotBlock msg={msg} />;
  }

  if (msg.type === "speak") {
    return <SpeakBubble msg={msg} />;
  }

  const getIcon = () => {
    if (isUser) return null;
    switch (msg.type) {
      case "action":
        const action = msg.action?.action;
        const content = msg.content.toLowerCase();

        // computer tool actions
        if (action?.includes("click") || action === "mouse_move" || action === "left_click_drag")
          return <MousePointer2 size={14} />;
        if (action === "type" || action === "key") return <Keyboard size={14} />;
        if (action === "scroll") return <ScrollText size={14} />;
        if (action === "wait") return <Clock size={14} />;

        // browser tool actions (no action object, match on content)
        if (!action) {
          if (content.includes("snapshot")) return <Layers size={14} />;
          if (content.includes("clicked") || content.includes("clicking")) return <MousePointerClick size={14} />;
          if (content.includes("hover")) return <Hand size={14} />;
          if (content.includes("filled") || content.includes("filling")) return <FormInput size={14} />;
          if (content.includes("pressed")) return <Keyboard size={14} />;
          if (content.includes("navigat") || content.includes("went back") || content.includes("went forward")) return <Navigation size={14} />;
          if (content.includes("reload")) return <RotateCw size={14} />;
          if (content.includes("waited")) return <Clock size={14} />;
          if (content.includes("tab") || content.includes("page")) return <PanelTop size={14} />;
          if (content.includes("dragged")) return <GripHorizontal size={14} />;
          if (content.includes("dialog")) return <MessageCircle size={14} />;
          if (content.includes("upload")) return <FileUp size={14} />;
          if (content.includes("screenshot")) return <Camera size={14} />;
          return <Eye size={14} />;
        }

        return <MousePointer2 size={14} />;
      case "error":
        return <AlertCircle size={14} className="text-red-400" />;
      default:
        return null;
    }
  };

  const getBubbleStyle = () => {
    if (isUser) {
      return "bg-white/10 border-white/20 px-3 py-2 rounded-2xl border backdrop-blur-sm";
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
  // map tool_use_id -> index in chatMessages for attaching results
  const toolUseIdToIndex: Map<string, number> = new Map();

  for (const msg of conversation.messages) {
    if (msg.role === "user") {
      // check if this is a tool result message
      const toolResults = msg.content.filter(b => b.type === "tool_result");
      if (toolResults.length > 0) {
        // attach results to corresponding tool_use messages
        for (const result of toolResults) {
          const idx = toolUseIdToIndex.get(result.tool_use_id || "");
          if (idx !== undefined && chatMessages[idx]) {
            const targetMsg = chatMessages[idx];
            // extract content from tool_result
            for (const contentBlock of result.content || []) {
              if (contentBlock.type === "text" && contentBlock.text) {
                // bash output - parse exit code if present
                if (targetMsg.type === "bash") {
                  const match = contentBlock.text.match(/^exit code: (-?\d+)\n/);
                  if (match) {
                    targetMsg.exitCode = parseInt(match[1], 10);
                    targetMsg.bashOutput = contentBlock.text.slice(match[0].length);
                  } else {
                    targetMsg.bashOutput = contentBlock.text;
                  }
                }
              } else if (contentBlock.type === "image" && contentBlock.source?.data) {
                // screenshot result
                targetMsg.screenshot = contentBlock.source.data;
              }
            }
          }
        }
        continue;
      }

      // regular user message
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
      if (!mainText && !screenshot) continue;

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
          const toolUseId = block.id;

          if (block.name === "bash" && input?.command) {
            const idx = chatMessages.length;
            chatMessages.push({
              id: crypto.randomUUID(),
              role: "assistant",
              content: String(input.command),
              timestamp: new Date(conversation.updated_at * 1000),
              type: "bash",
              pending: false,
            });
            if (toolUseId) toolUseIdToIndex.set(toolUseId, idx);
          } else if (block.name === "computer" && input?.action) {
            const idx = chatMessages.length;
            chatMessages.push({
              id: crypto.randomUUID(),
              role: "assistant",
              content: formatActionForHistory("computer", input),
              timestamp: new Date(conversation.updated_at * 1000),
              type: "action",
              action: input as unknown as ChatMessage["action"],
              pending: false,
            });
            if (toolUseId) toolUseIdToIndex.set(toolUseId, idx);
          } else {
            // browser tools - show as action
            const idx = chatMessages.length;
            chatMessages.push({
              id: crypto.randomUUID(),
              role: "assistant",
              content: formatActionForHistory(block.name, input || {}),
              timestamp: new Date(conversation.updated_at * 1000),
              type: "action",
              pending: false,
            });
            if (toolUseId) toolUseIdToIndex.set(toolUseId, idx);
          }
        }
      }
    }
  }

  return chatMessages;
}

interface HistoryDropdownProps {
  onNewChat: () => void;
  onLoad: (messages: ChatMessage[], model: ModelId, mode: AgentMode, conversationId: string, voiceMode: boolean) => void;
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
        onLoad(chatMessages, model, mode, conv.id, conv.voice_mode ?? false);
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
        className={`flex items-center gap-2 px-2 py-1 rounded-md transition-colors ${
          disabled
            ? "text-white/20 cursor-not-allowed"
            : open
              ? "bg-white/10 text-white/80"
              : "hover:bg-white/5 text-white/70 hover:text-white/90"
        }`}
      >
        <span className="text-[13px] font-semibold tracking-tight">taskhomie</span>
        <ChevronDown size={12} className={`text-white/40 transition-transform ${open ? "rotate-180" : ""}`} />
      </button>

      <AnimatePresence>
        {open && (
          <motion.div
            initial={{ opacity: 0, y: 4 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 4 }}
            transition={{ duration: 0.1 }}
            className="absolute top-full left-0 mt-1.5 w-72 z-50 rounded-xl overflow-hidden"
            style={{
              background: "rgba(20, 20, 20, 0.95)",
              backdropFilter: "blur(20px)",
              border: "1px solid rgba(255, 255, 255, 0.1)",
              boxShadow: "0 12px 40px rgba(0, 0, 0, 0.6)",
            }}
          >
            {/* new chat button */}
            <button
              onClick={() => { onNewChat(); setOpen(false); }}
              className="w-full flex items-center gap-2.5 px-3 py-2.5 text-[12px] text-white/90 hover:bg-white/10 transition-colors"
            >
              <div className="w-6 h-6 rounded-lg bg-blue-500/20 flex items-center justify-center">
                <Plus size={14} className="text-blue-400" />
              </div>
              <span className="font-medium">New chat</span>
            </button>

            {/* divider with label */}
            {conversations.length > 0 && (
              <div className="px-3 py-1.5 border-t border-white/5">
                <span className="text-[10px] text-white/30 uppercase tracking-wider">Recent</span>
              </div>
            )}

            {/* conversation list */}
            <div className="max-h-[260px] overflow-y-auto">
              {loading ? (
                <div className="px-3 py-6 text-[11px] text-white/40 text-center">Loading...</div>
              ) : conversations.length === 0 ? (
                <div className="px-3 py-6 text-[11px] text-white/40 text-center border-t border-white/5">
                  No conversations yet
                </div>
              ) : (
                conversations.map((conv) => (
                  <div
                    key={conv.id}
                    className="group flex items-center gap-2.5 px-3 py-2 hover:bg-white/8 transition-colors cursor-pointer"
                    onClick={() => handleLoad(conv.id)}
                  >
                    <MessageSquare size={13} className="text-white/30 shrink-0" />
                    <div className="flex-1 min-w-0">
                      <p className="text-[12px] text-white/80 truncate">{conv.title || "Untitled"}</p>
                      <p className="text-[10px] text-white/40">{formatRelativeTime(conv.updated_at)} · {conv.message_count} msgs</p>
                    </div>
                    <button
                      onClick={(e) => handleDelete(e, conv.id)}
                      className="opacity-0 group-hover:opacity-100 p-1.5 rounded-md hover:bg-red-500/20 transition-all"
                    >
                      <Trash2 size={12} className="text-red-400/80" />
                    </button>
                  </div>
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
  const { messages, isRunning, inputText, setInputText, selectedModel, setSelectedModel, selectedMode, setSelectedMode, streamingText, streamingThinking, clearMessages, setMessages, setVoiceMode, setConversationId } = useAgentStore();
  const { submit } = useAgent();
  const scrollRef = useRef<HTMLDivElement>(null);
  const bottomRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  // voice state
  const [isVoiceActive, setIsVoiceActive] = useState(false);
  const [voiceText, setVoiceText] = useState("");
  const [usedVoiceInput, setUsedVoiceInput] = useState(false);
  const [showVoiceConfirm, setShowVoiceConfirm] = useState(false);

  // settings panel state
  const [settingsOpen, setSettingsOpen] = useState(false);

  const isSpotlight = variant === "spotlight";
  const isMini = variant === "mini";
  const panelClass = isMini ? "mini-panel" : isSpotlight ? "spotlight-panel" : "app-panel";
  const padding = isMini ? "px-2 py-2" : isSpotlight ? "px-4 py-4" : "px-3 py-3";
  const inputPadding = isMini ? "p-2 pt-0" : isSpotlight ? "p-4 pt-0" : "p-3 pt-0";
  const gifSize = isMini ? "w-[16rem]" : isSpotlight ? "w-[32rem]" : "w-[28rem]";

  // ref to track current input for voice append
  const inputTextRef = useRef(inputText);
  useEffect(() => { inputTextRef.current = inputText; }, [inputText]);

  // voice event listeners
  useEffect(() => {
    const unlistenTranscription = listen<{ text: string; is_final: boolean }>("voice:transcription", (event) => {
      const { text, is_final } = event.payload;
      if (is_final) {
        const current = inputTextRef.current;
        const newText = current ? current + " " + text : text;
        setInputText(newText);
        setVoiceText("");
        setUsedVoiceInput(true);
      } else {
        setVoiceText(text);
      }
    });

    const unlistenStopped = listen("voice:stopped", () => {
      setIsVoiceActive(false);
      setVoiceText("");
      // show confirmation when recording stops (if there's text)
      if (inputTextRef.current.trim()) {
        setShowVoiceConfirm(true);
      }
    });

    const unlistenError = listen<string>("voice:error", (event) => {
      console.error("[voice] error:", event.payload);
      setIsVoiceActive(false);
      setVoiceText("");
    });

    return () => {
      unlistenTranscription.then((f) => f());
      unlistenStopped.then((f) => f());
      unlistenError.then((f) => f());
    };
  }, [setInputText]);

  const toggleVoice = async () => {
    console.log("[voice] toggleVoice called, isVoiceActive:", isVoiceActive);
    if (isVoiceActive) {
      console.log("[voice] stopping...");
      await invoke("stop_voice");
      setIsVoiceActive(false);
      setVoiceText("");
    } else {
      try {
        console.log("[voice] starting...");
        await invoke("start_voice");
        console.log("[voice] started successfully");
        setIsVoiceActive(true);
      } catch (e) {
        console.error("[voice] failed to start:", e);
      }
    }
  };

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

  // resize textarea when voice text changes (onInput doesn't fire for programmatic changes)
  // voiceText = interim, inputText gets final transcription
  useEffect(() => {
    requestAnimationFrame(() => {
      if (inputRef.current) {
        inputRef.current.style.height = "24px";
        inputRef.current.style.height = Math.min(inputRef.current.scrollHeight, 100) + "px";
      }
    });
  }, [voiceText, inputText]);

  const handleSubmit = () => {
    if (!inputText.trim()) return;
    // enable TTS response if voice input was used
    if (usedVoiceInput) {
      setVoiceMode(true);
      setUsedVoiceInput(false);
    }
    submit();
  };

  const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
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
          {settingsOpen ? (
            <>
              <button
                onClick={() => setSettingsOpen(false)}
                className="flex items-center gap-1.5 text-[13px] text-white/70 hover:text-white/90 transition-colors"
              >
                <ChevronLeft size={16} />
                <span>Settings</span>
              </button>
              <div className="flex items-center gap-2">
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
            </>
          ) : (
            <>
              <HistoryDropdown
                onNewChat={() => clearMessages()}
                onLoad={(msgs, model, mode, conversationId, voiceMode) => {
                  setMessages(msgs);
                  setSelectedModel(model);
                  setSelectedMode(mode);
                  setConversationId(conversationId);
                  setVoiceMode(voiceMode);
                }}
                disabled={isRunning}
              />
              <div className="flex items-center gap-2">
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
                  onClick={() => setSettingsOpen(true)}
                  className="w-7 h-7 flex items-center justify-center rounded-md text-white/40 hover:text-white/70 hover:bg-white/10 transition-colors"
                  title="Settings"
                >
                  <Settings size={14} />
                </button>
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
            </>
          )}
        </div>
      )}

      {/* content area - settings or chat */}
      {settingsOpen ? (
        <div className={`flex-1 overflow-y-auto ${padding}`}>
          <SettingsContent />
        </div>
      ) : (
        <>
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
            {/* voice confirmation bar */}
            <AnimatePresence>
              {showVoiceConfirm && !isRunning && (
                <motion.div
                  initial={{ opacity: 0, y: 10 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: 10 }}
                  className="glass-card mb-2 p-2"
                >
                  <div className="flex items-start gap-2 mb-2">
                    <Mic size={14} className="text-orange-300 shrink-0 mt-0.5" />
                    <p className="text-[12px] text-white/80 leading-relaxed break-words whitespace-pre-wrap">{inputText}</p>
                  </div>
                  <div className="flex justify-end gap-2">
                    <motion.button
                      onClick={() => setShowVoiceConfirm(false)}
                      whileHover={{ scale: 1.05 }}
                      whileTap={{ scale: 0.95 }}
                      className="px-3 py-1 rounded-lg bg-white/10 border border-white/10 text-[11px] text-white/70 hover:text-white/90 hover:bg-white/15 transition-colors"
                    >
                      Edit
                    </motion.button>
                    <motion.button
                      onClick={() => {
                        setShowVoiceConfirm(false);
                        setVoiceMode(true);
                        setUsedVoiceInput(false);
                        submit();
                      }}
                      whileHover={{ scale: 1.05 }}
                      whileTap={{ scale: 0.95 }}
                      className="px-3 py-1 rounded-lg bg-orange-500/30 border border-orange-400/30 text-[11px] text-orange-300 hover:bg-orange-500/40 transition-colors"
                    >
                      Send
                    </motion.button>
                  </div>
                </motion.div>
              )}
            </AnimatePresence>

            {isRunning ? (
              <div className="glass-card flex items-center justify-center gap-2 p-3 text-red-300/70">
                <Square size={14} />
                <span className="text-[12px]">⌘⇧S to stop</span>
              </div>
            ) : showVoiceConfirm ? null : (
              <div className="glass-card flex items-center gap-2 p-2">
                <motion.button
                  onClick={toggleVoice}
                  whileHover={{ scale: 1.05 }}
                  whileTap={{ scale: 0.95 }}
                  className={`shrink-0 w-8 h-8 rounded-xl flex items-center justify-center transition-colors ${
                    isVoiceActive
                      ? "bg-red-500/30 border border-red-400/30 text-red-300 animate-pulse"
                      : "bg-white/5 border border-white/10 text-white/40 hover:text-white/60"
                  }`}
                  title={isVoiceActive ? "Stop recording" : "Start voice input"}
                >
                  {isVoiceActive ? <MicOff size={14} /> : <Mic size={14} />}
                </motion.button>
                <textarea
                  ref={inputRef}
                  value={inputText + (voiceText ? (inputText ? " " : "") + voiceText : "")}
                  onChange={(e) => setInputText(e.target.value)}
                  onKeyDown={handleKeyDown}
                  placeholder={isVoiceActive ? "listening..." : "what should I do?"}
                  rows={1}
                  className={`flex-1 bg-transparent text-white text-[13px] placeholder-white/30 resize-none focus:outline-none min-h-[24px] max-h-[100px] py-1 px-1 overflow-hidden ${isVoiceActive ? "italic" : ""}`}
                  style={{ height: "24px" }}
                  onInput={(e) => {
                    const target = e.target as HTMLTextAreaElement;
                    target.style.height = "24px";
                    target.style.height = Math.min(target.scrollHeight, 100) + "px";
                  }}
                />
                <motion.button
                  onClick={() => setSelectedMode(selectedMode === "computer" ? "browser" : "computer")}
                  whileHover={{ scale: 1.05 }}
                  whileTap={{ scale: 0.95 }}
                  className={`shrink-0 h-8 px-2 rounded-xl flex items-center gap-1.5 transition-colors ${
                    selectedMode === "computer"
                      ? "bg-orange-500/30 border border-orange-400/30 text-orange-300"
                      : "bg-white/5 border border-white/10 text-white/40 hover:text-white/60"
                  }`}
                  title={selectedMode === "computer" ? "Computer control active" : "Enable computer control"}
                >
                  <MousePointerClick size={12} />
                  <span className="text-[9px]">Computer</span>
                </motion.button>
                <motion.button
                  onClick={handleSubmit}
                  disabled={!inputText.trim()}
                  whileHover={{ scale: 1.05 }}
                  whileTap={{ scale: 0.95 }}
                  className={`shrink-0 w-8 h-8 rounded-xl flex items-center justify-center transition-colors ${
                    inputText.trim()
                      ? "bg-orange-500/30 border border-orange-400/30 text-orange-300"
                      : "bg-white/5 border border-white/10 text-white/20"
                  }`}
                >
                  <Send size={14} />
                </motion.button>
              </div>
            )}
          </div>
        </>
      )}
    </motion.div>
  );
}
