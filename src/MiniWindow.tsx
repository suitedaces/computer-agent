import { useEffect, useState, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { motion, AnimatePresence } from "framer-motion";
import {
  MousePointer2,
  Keyboard,
  Camera,
  ScrollText,
  Terminal,
  Globe,
  Clock,
  Square,
} from "lucide-react";

interface FeedItem {
  id: number;
  type: "text" | "action" | "bash" | "mcp";
  content: string;
  actionType?: string;
  timestamp: number;
}

let itemId = 0;

function getIcon(type: string, actionType?: string) {
  if (type === "bash") return <Terminal size={10} />;
  if (type === "mcp") return <Globe size={10} />;
  if (type === "action") {
    if (actionType?.includes("click") || actionType === "mouse_move") return <MousePointer2 size={10} />;
    if (actionType === "type" || actionType === "key") return <Keyboard size={10} />;
    if (actionType === "scroll") return <ScrollText size={10} />;
    if (actionType === "screenshot") return <Camera size={10} />;
    if (actionType === "wait") return <Clock size={10} />;
    return <MousePointer2 size={10} />;
  }
  return null;
}

function truncate(text: string, max: number) {
  if (text.length <= max) return text;
  return text.slice(0, max) + "...";
}

export default function MiniWindow() {
  console.log("[mini] MiniWindow component rendering");
  const [feed, setFeed] = useState<FeedItem[]>([]);
  const [streamingText, setStreamingText] = useState("");
  const [isRunning, setIsRunning] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);

  // auto-scroll
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [feed, streamingText]);

  // poll running state
  useEffect(() => {
    const checkRunning = () => {
      invoke<boolean>("is_agent_running").then(setIsRunning).catch(() => {});
    };
    checkRunning();
    const interval = setInterval(checkRunning, 500);
    return () => clearInterval(interval);
  }, []);

  const handleStop = async () => {
    try {
      await invoke("stop_agent");
      setIsRunning(false);
      invoke("hide_mini_window").catch(() => {});
    } catch (e) {
      console.error(e);
    }
  };

  useEffect(() => {
    console.log("[mini] Setting up event listeners");

    const unlisten1 = listen<{ delta: string }>("agent:text_delta", (e) => {
      console.log("[mini] text_delta received");
      setStreamingText((prev) => prev + e.payload.delta);
    });

    const unlisten2 = listen("agent:message", () => {
      console.log("[mini] message received");
      // commit streaming text to feed
      setStreamingText((prev) => {
        if (prev.trim()) {
          const lines = prev.trim().split("\n");
          const truncated = lines.map(l => truncate(l.trim(), 60)).join(" ");
          setFeed((f) => [...f.slice(-20), {
            id: itemId++,
            type: "text",
            content: truncate(truncated, 100),
            timestamp: Date.now(),
          }]);
        }
        return "";
      });
    });

    const unlisten3 = listen<{ action: string; text?: string }>("agent:action", (e) => {
      console.log("[mini] action received:", e.payload.action);
      const { action, text } = e.payload;
      let content = action;
      if (action === "type" && text) content = `type: ${truncate(text, 25)}`;
      else if (action === "key" && text) content = `key: ${text}`;
      else if (action === "left_click") content = "click";
      else if (action === "double_click") content = "double click";
      else if (action === "mouse_move") content = "move";
      else if (action === "scroll") content = "scroll";
      else if (action === "screenshot") content = "screenshot";
      else if (action === "wait") content = "wait";

      setFeed((f) => [...f.slice(-20), {
        id: itemId++,
        type: "action",
        content,
        actionType: action,
        timestamp: Date.now(),
      }]);
    });

    const unlisten4 = listen<{ command: string }>("agent:bash", (e) => {
      console.log("[mini] bash received:", e.payload.command);
      setFeed((f) => [...f.slice(-20), {
        id: itemId++,
        type: "bash",
        content: `$ ${truncate(e.payload.command, 35)}`,
        timestamp: Date.now(),
      }]);
    });

    const unlisten5 = listen<{ name: string }>("agent:mcp_tool", (e) => {
      console.log("[mini] mcp_tool received:", e.payload.name);
      setFeed((f) => [...f.slice(-20), {
        id: itemId++,
        type: "mcp",
        content: e.payload.name,
        timestamp: Date.now(),
      }]);
    });

    const unlisten6 = listen("agent:started", () => {
      console.log("[mini] started received");
      setIsRunning(true);
      setFeed([]);
      setStreamingText("");
    });

    const unlisten7 = listen("agent:stopped", () => {
      console.log("[mini] stopped received");
      setIsRunning(false);
      setStreamingText("");
      invoke("hide_mini_window").catch(() => {});
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
  }, []);

  const handleOpenMain = async () => {
    try {
      await invoke("show_main_window");
    } catch (e) {
      console.error(e);
    }
  };

  if (!isRunning) {
    return (
      <div className="h-screen w-screen flex items-start justify-start p-2">
        <motion.div
          initial={{ opacity: 0, scale: 0.9, y: -10 }}
          animate={{ opacity: 1, scale: 1, y: 0 }}
          onClick={handleOpenMain}
          className="mini-feed cursor-pointer hover:border-white/20 transition-colors"
          style={{ maxHeight: 'auto' }}
        >
          <div className="flex items-center gap-2 px-3 py-2.5">
            <span className="w-2 h-2 rounded-full bg-white/30" />
            <span className="text-[10px] text-white/40">idle</span>
          </div>
        </motion.div>
      </div>
    );
  }

  return (
    <div className="h-screen w-screen flex items-start justify-start p-2">
      <motion.div
        initial={{ opacity: 0, scale: 0.9, y: -10 }}
        animate={{ opacity: 1, scale: 1, y: 0 }}
        className="mini-feed"
      >
        {/* scrollable feed */}
        <div ref={scrollRef} className="flex-1 overflow-y-auto overflow-x-hidden px-2 py-1.5 space-y-1">
          <AnimatePresence mode="popLayout">
            {feed.map((item) => (
              <motion.div
                key={item.id}
                initial={{ opacity: 0, x: -8 }}
                animate={{ opacity: 1, x: 0 }}
                exit={{ opacity: 0 }}
                layout
              >
                {item.type === "text" ? (
                  <p className="text-[10px] text-white/70 leading-relaxed">
                    {item.content}
                  </p>
                ) : (
                  <div className="inline-flex items-center gap-1.5 px-2 py-1 rounded-md bg-white/10 border border-white/10">
                    <span className="text-white/50">{getIcon(item.type, item.actionType)}</span>
                    <span className="text-[10px] text-white/70">{item.content}</span>
                  </div>
                )}
              </motion.div>
            ))}
          </AnimatePresence>

          {/* streaming text */}
          {streamingText && (
            <motion.p
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              className="text-[10px] text-white/50 leading-relaxed italic"
            >
              {truncate(streamingText.slice(-80), 80)}
              <span className="animate-pulse">...</span>
            </motion.p>
          )}

          {/* idle state */}
          {feed.length === 0 && !streamingText && (
            <div className="flex items-center gap-2 py-1">
              <span className="w-2 h-2 rounded-full bg-green-400 animate-pulse" />
              <span className="text-[10px] text-white/40">working...</span>
            </div>
          )}
        </div>

        {/* stop button */}
        <div className="shrink-0 px-2 pb-2 pt-1 border-t border-white/5">
          <button
            onClick={handleStop}
            className="w-full flex items-center justify-center gap-1.5 py-1.5 rounded-md bg-red-500/20 border border-red-400/20 text-red-300 hover:bg-red-500/30 transition-colors text-[10px]"
          >
            <Square size={10} />
            <span>Stop</span>
          </button>
        </div>
      </motion.div>
    </div>
  );
}
