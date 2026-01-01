import { useRef, useEffect, useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Streamdown } from "streamdown";
import { useAgentStore } from "../stores/agentStore";
import { ChatMessage } from "../types";
import {
  MousePointer2,
  Keyboard,
  Camera,
  ScrollText,
  AlertCircle,
  ChevronDown,
  ChevronUp,
  Brain,
  Clock,
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
  Volume2,
  Play,
  Pause,
  Search,
  FileText,
} from "lucide-react";
import { createAudioElement } from "../utils/audio";

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

  if (!action) {
    return renderContentWithUrls(msg.content);
  }

  const coord = action.coordinate;
  const actionType = action.action;
  const text = action.text;

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
          <span className={`text-[11px] text-[#e6edf3] break-all flex-1 select-text ${msg.pending ? "sweep-text" : ""}`}>
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

        if (action?.includes("click") || action === "mouse_move" || action === "left_click_drag")
          return <MousePointer2 size={14} />;
        if (action === "type" || action === "key") return <Keyboard size={14} />;
        if (action === "scroll") return <ScrollText size={14} />;
        if (action === "wait") return <Clock size={14} />;

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
          // server-side tools
          if (content.includes("search")) return <Search size={14} />;
          if (content.includes("fetch")) return <FileText size={14} />;
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

function ThinkingBubble() {
  const { streamingThinking, isRunning } = useAgentStore();
  const [expanded, setExpanded] = useState(false);
  const thinkingScrollRef = useRef<HTMLDivElement>(null);

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

interface MessagesDisplayProps {
  className?: string;
  header?: React.ReactNode;
}

export default function MessagesDisplay({ className = "", header }: MessagesDisplayProps) {
  const { messages, streamingText, streamingThinking } = useAgentStore();
  const scrollRef = useRef<HTMLDivElement>(null);
  const bottomRef = useRef<HTMLDivElement>(null);

  // auto-scroll on new messages
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  // auto-scroll during streaming
  useEffect(() => {
    if (!streamingText && !streamingThinking) return;
    const frame = requestAnimationFrame(() => {
      if (scrollRef.current) {
        scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
      }
    });
    return () => cancelAnimationFrame(frame);
  }, [streamingText, streamingThinking]);

  return (
    <div ref={scrollRef} className={`flex-1 overflow-y-auto ${className}`}>
      {header}
      <div className="space-y-2">
        {messages.map((msg) => <MessageBubble key={msg.id} msg={msg} />)}
        <ThinkingBubble />
        <StreamingBubble />
        <div ref={bottomRef} />
      </div>
    </div>
  );
}
