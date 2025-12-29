import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useCallback } from "react";
import { useAgentStore } from "../stores/agentStore";
import { AgentUpdate } from "../types";

export function useAgent() {
  const {
    isRunning,
    inputText,
    selectedModel,
    selectedMode,
    messages,
    setIsRunning,
    addMessage,
    markLastActionComplete,
    updateLastBashWithResult,
    setApiKeySet,
    setInputText,
    appendStreamingText,
    clearStreamingText,
    appendStreamingThinking,
    clearStreamingThinking,
  } = useAgentStore();

  // setup event listener
  useEffect(() => {
    invoke<boolean>("check_api_key")
      .then(setApiKeySet)
      .catch(() => setApiKeySet(false));

    invoke("debug_log", { message: "Setting up event listener..." });

    const unlistenPromise = listen<AgentUpdate>("agent-update", (event) => {
      invoke("debug_log", { message: `Event received: ${event.payload.update_type}` });
      const { update_type, message, action, screenshot, exit_code } = event.payload;

      switch (update_type) {
        case "started":
          setIsRunning(true);
          // make main window click-through while agent runs
          invoke("set_main_click_through", { ignore: true }).catch(() => {});
          break;

        case "thinking":
          clearStreamingThinking();
          addMessage({ role: "assistant", content: message, type: "thinking" });
          break;

        case "response":
          clearStreamingText();
          addMessage({ role: "assistant", content: message, type: "info" });
          break;

        case "action":
          // bash commands get their own type
          if (message.startsWith("$ ")) {
            addMessage({
              role: "assistant",
              content: message.slice(2), // remove "$ " prefix, store just the command
              type: "bash",
            });
          } else {
            addMessage({
              role: "assistant",
              content: message,
              type: "action",
              action: action,
            });
          }
          break;

        case "screenshot":
          markLastActionComplete(screenshot);
          break;

        case "finished":
          setIsRunning(false);
          // disable click-through when done
          invoke("set_main_click_through", { ignore: false }).catch(() => {});
          break;

        case "error":
          setIsRunning(false);
          // disable click-through on error
          invoke("set_main_click_through", { ignore: false }).catch(() => {});
          addMessage({ role: "assistant", content: message, type: "error" });
          break;

        case "bash_result":
          updateLastBashWithResult(message, exit_code);
          break;
      }
    });

    unlistenPromise.then(() => {
      invoke("debug_log", { message: "Event listener ready" });
    }).catch((err) => {
      invoke("debug_log", { message: `Event listener FAILED: ${err}` });
    });

    // streaming event listener
    const unlistenStreamPromise = listen<{ type: string; text?: string; name?: string }>("agent-stream", (event) => {
      const { type, text } = event.payload;
      if (type === "thinking_delta" && text) {
        appendStreamingThinking(text);
      } else if (type === "text_delta" && text) {
        appendStreamingText(text);
      }
    });

    return () => {
      unlistenPromise.then((fn) => fn());
      unlistenStreamPromise.then((fn) => fn());
    };
  }, [setIsRunning, addMessage, markLastActionComplete, updateLastBashWithResult, setApiKeySet, appendStreamingText, clearStreamingText, appendStreamingThinking, clearStreamingThinking]);

  const submit = useCallback(async () => {
    const text = inputText.trim();
    if (!text || isRunning) return;

    // build history from past messages (user + assistant text only)
    const history = messages
      .filter(m => m.role === "user" || (m.role === "assistant" && m.type === "thinking"))
      .map(m => ({ role: m.role, content: m.content }));

    // add user message
    addMessage({ role: "user", content: text });
    setInputText("");

    try {
      await invoke("run_agent", { instructions: text, model: selectedModel, mode: selectedMode, history, contextScreenshot: null });
    } catch (error) {
      addMessage({ role: "assistant", content: String(error), type: "error" });
      setIsRunning(false);
    }
  }, [inputText, isRunning, selectedModel, selectedMode, messages, addMessage, setInputText, setIsRunning]);

  const stop = useCallback(async () => {
    try {
      await invoke("stop_agent");
      setIsRunning(false);
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
