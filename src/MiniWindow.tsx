import { useEffect, useState, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
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
  ChevronRight,
  Send,
  X,
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
  const [helpMode, setHelpMode] = useState(false);
  const [helpPrompt, setHelpPrompt] = useState("Help me out here: ");
  const [helpScreenshot, setHelpScreenshot] = useState<string | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // auto-scroll
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [feed, streamingText]);

  // poll running state and resize window accordingly
  useEffect(() => {
    const checkRunning = () => {
      invoke<boolean>("is_agent_running").then((running) => {
        setIsRunning(running);
        // don't resize if in help mode
        if (helpMode) return;
        const win = getCurrentWindow();
        if (running) {
          win.setSize(new LogicalSize(300, 220));
        } else {
          // idle bar matches mini-feed width (280) but shorter height
          win.setSize(new LogicalSize(280, 36));
        }
      }).catch(() => {});
    };
    checkRunning();
    const interval = setInterval(checkRunning, 500);
    return () => clearInterval(interval);
  }, [helpMode]);

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

    // hotkey help mode - Cmd+Shift+H triggers this
    const unlisten8 = listen("hotkey-help", async () => {
      console.log("[mini] hotkey-help received");
      try {
        const screenshot = await invoke<string>("capture_screen_for_help");
        setHelpScreenshot(screenshot);
        setHelpMode(true);
        setHelpPrompt("Help me out here: ");
        // focus input after render
        setTimeout(() => inputRef.current?.focus(), 50);
      } catch (e) {
        console.error("[mini] screenshot failed:", e);
      }
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
    try {
      // run agent with screenshot context
      await invoke("run_agent", {
        instructions: helpPrompt,
        model: "claude-sonnet-4-5",
        mode: "computer",
        history: [],
        contextScreenshot: helpScreenshot,
      });
      setHelpMode(false);
      setHelpScreenshot(null);
    } catch (e) {
      console.error("[mini] help submit failed:", e);
    }
  };

  const handleHelpCancel = async () => {
    setHelpMode(false);
    setHelpScreenshot(null);
    setHelpPrompt("Help me out here: ");
    // resize back to idle
    const win = getCurrentWindow();
    await win.setSize(new LogicalSize(280, 36));
  };

  // help mode UI - shows when hotkey triggered
  if (helpMode && !isRunning) {
    return (
      <div className="h-screen w-screen flex items-start justify-start p-2">
        <motion.div
          initial={{ opacity: 0, scale: 0.95 }}
          animate={{ opacity: 1, scale: 1 }}
          className="mini-feed"
        >
          {/* screenshot preview */}
          {helpScreenshot && (
            <div className="px-2 pt-2">
              <img
                src={`data:image/jpeg;base64,${helpScreenshot}`}
                alt="Screenshot"
                className="w-full h-20 object-cover rounded-md border border-white/10"
              />
            </div>
          )}

          {/* prompt input */}
          <div className="flex-1 px-2 py-2">
            <input
              ref={inputRef}
              type="text"
              value={helpPrompt}
              onChange={(e) => setHelpPrompt(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleHelpSubmit()}
              placeholder="Help me out here..."
              className="w-full bg-white/5 border border-white/10 rounded-md px-2 py-1.5 text-[11px] text-white/90 placeholder:text-white/30 focus:outline-none focus:border-white/20"
              autoFocus
            />
          </div>

          {/* action buttons */}
          <div className="shrink-0 px-2 pb-2 flex gap-2">
            <button
              onClick={handleHelpCancel}
              className="flex-1 flex items-center justify-center gap-1 py-1.5 rounded-md bg-white/5 border border-white/10 text-white/50 hover:bg-white/10 transition-colors text-[10px]"
            >
              <X size={10} />
              <span>Cancel</span>
            </button>
            <button
              onClick={handleHelpSubmit}
              className="flex-1 flex items-center justify-center gap-1 py-1.5 rounded-md bg-blue-500/20 border border-blue-400/20 text-blue-300 hover:bg-blue-500/30 transition-colors text-[10px]"
            >
              <Send size={10} />
              <span>Send</span>
            </button>
          </div>
        </motion.div>
      </div>
    );
  }

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
