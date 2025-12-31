import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useCallback } from "react";
import { useAgentStore } from "../stores/agentStore";
import { AgentUpdate } from "../types";
import { queueAudio } from "../utils/audio";

type UnlistenFn = () => void;

const shouldAutoplayAudio = (() => {
  if (typeof window === "undefined") return true;
  const params = new URLSearchParams(window.location.search);
  // only the main window should auto-play to avoid duplicate audio across windows
  return !params.has("mini") && !params.has("spotlight") && !params.has("border");
})();

let listenersAttached = false;
let listenersRefCount = 0;
let unlistenPromises: Array<Promise<UnlistenFn>> = [];

function attachListeners() {
  if (listenersAttached) return;
  listenersAttached = true;

  const store = useAgentStore.getState;

  invoke<boolean>("check_api_key")
    .then((v) => store().setApiKeySet(v))
    .catch(() => store().setApiKeySet(false));

  const unlistenPromise = listen<AgentUpdate>("agent-update", (event) => {
    const { update_type, message, action, screenshot, exit_code, mode } = event.payload;
    const s = store();

    switch (update_type) {
      case "started":
        s.setIsRunning(true);
        if (mode === "computer") {
          invoke("set_main_click_through", { ignore: true }).catch(() => {});
          invoke("set_mini_click_through", { ignore: true }).catch(() => {});
          invoke("set_spotlight_click_through", { ignore: true }).catch(() => {});
          invoke("show_border_overlay").catch(() => {});
        }
        break;

      case "user_message":
        s.addMessage({ role: "user", content: message, screenshot });
        break;

      case "thinking":
        s.clearStreamingThinking();
        s.addMessage({ role: "assistant", content: message, type: "thinking" });
        break;

      case "response":
        s.clearStreamingText();
        s.addMessage({ role: "assistant", content: message, type: "info" });
        break;

      case "action":
        if (message.startsWith("$ ")) {
          s.addMessage({
            role: "assistant",
            content: message.slice(2),
            type: "bash",
          });
        } else {
          s.addMessage({
            role: "assistant",
            content: message,
            type: "action",
            action: action,
          });
        }
        break;

      case "screenshot":
        s.markLastActionComplete(screenshot);
        break;

      case "finished":
        s.setIsRunning(false);
        invoke("set_main_click_through", { ignore: false }).catch(() => {});
        invoke("set_mini_click_through", { ignore: false }).catch(() => {});
        invoke("set_spotlight_click_through", { ignore: false }).catch(() => {});
        invoke("hide_border_overlay").catch(() => {});
        break;

      case "error":
        s.setIsRunning(false);
        invoke("set_main_click_through", { ignore: false }).catch(() => {});
        invoke("set_mini_click_through", { ignore: false }).catch(() => {});
        invoke("set_spotlight_click_through", { ignore: false }).catch(() => {});
        invoke("hide_border_overlay").catch(() => {});
        s.addMessage({ role: "assistant", content: message, type: "error" });
        break;

      case "bash_result":
        s.updateLastBashWithResult(message, exit_code);
        break;

      case "browser_result":
        s.markLastActionComplete();
        break;
    }
  });

  const unlistenStreamPromise = listen<{ type: string; text?: string }>("agent-stream", (event) => {
    const { type, text } = event.payload;
    const s = store();
    if (type === "thinking_delta" && text) {
      s.appendStreamingThinking(text);
    } else if (type === "text_delta" && text) {
      s.appendStreamingText(text);
    }
  });

  const unlistenConvIdPromise = listen<string>("agent:conversation_id", (event) => {
    store().setConversationId(event.payload);
  });

  const unlistenSpeakPromise = listen<{ audio: string; text: string }>("agent:speak", (event) => {
    const { audio, text } = event.payload;
    console.log("[voice] Speaking:", text.slice(0, 50) + "...");
    store().addMessage({ role: "assistant", content: text, type: "speak", audioData: audio });
    if (shouldAutoplayAudio) {
      queueAudio(audio);
    }
  });

  unlistenPromises = [
    unlistenPromise,
    unlistenStreamPromise,
    unlistenConvIdPromise,
    unlistenSpeakPromise,
  ];
}

function detachListeners() {
  if (!listenersAttached) return;
  listenersAttached = false;

  const toRemove = unlistenPromises;
  unlistenPromises = [];
  toRemove.forEach((promise) => {
    promise.then((fn) => fn());
  });
}

export function useAgent() {
  const {
    isRunning,
    inputText,
    selectedModel,
    selectedMode,
    voiceMode,
    messages,
    conversationId,
    setIsRunning,
    addMessage,
    setInputText,
  } = useAgentStore();

  // setup event listeners once on mount
  // use getState() inside handlers to avoid stale closures and dep array issues
  useEffect(() => {
    listenersRefCount += 1;
    attachListeners();

    return () => {
      listenersRefCount -= 1;
      if (listenersRefCount <= 0) {
        listenersRefCount = 0;
        detachListeners();
      }
    };
  }, []);

  const submit = useCallback(async (overrideText?: string, contextScreenshot?: string, overrideMode?: string) => {
    const text = (overrideText ?? inputText).trim();
    if (!text || isRunning) return;

    // build history from past messages (user + assistant responses)
    const history = messages
      .filter(m => m.role === "user" || (m.role === "assistant" && (m.type === "thinking" || m.type === "info")))
      .map(m => ({ role: m.role, content: m.content }));

    // clear input before invoking (user message comes from backend via user_message event)
    if (!overrideText) setInputText("");

    // use override mode if provided, otherwise use selected mode
    const mode = overrideMode ?? selectedMode;

    try {
      await invoke("run_agent", { instructions: text, model: selectedModel, mode, voiceMode, history, contextScreenshot: contextScreenshot ?? null, conversationId });
    } catch (error) {
      // on early failure, show the user message so they know what failed
      addMessage({ role: "user", content: text });
      addMessage({ role: "assistant", content: String(error), type: "error" });
      setIsRunning(false);
    }
  }, [inputText, isRunning, selectedModel, selectedMode, voiceMode, messages, conversationId, addMessage, setInputText, setIsRunning]);

  const stop = useCallback(async () => {
    try {
      await invoke("stop_agent");
      setIsRunning(false);
      invoke("hide_border_overlay").catch(() => {});
      addMessage({ role: "assistant", content: "Interrupted", type: "error" });
    } catch (e) {
      console.error(e);
    }
  }, [setIsRunning, addMessage]);

  const toggle = useCallback(() => {
    if (isRunning) {
      stop();
    } else {
      submit();
    }
  }, [isRunning, stop, submit]);

  return { submit, stop, toggle };
}
