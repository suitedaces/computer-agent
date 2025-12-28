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
    setIsRunning,
    addMessage,
    markLastActionComplete,
    updateLastBashWithResult,
    setScreenshot,
    setApiKeySet,
    setInputText,
    appendStreamingText,
    clearStreamingText,
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
          break;

        case "thinking":
          clearStreamingText();
          addMessage({ role: "assistant", content: message, type: "thinking" });
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
          markLastActionComplete();
          if (screenshot) {
            setScreenshot(screenshot);
          }
          break;

        case "finished":
          setIsRunning(false);
          break;

        case "error":
          setIsRunning(false);
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
      if (type === "text_delta" && text) {
        appendStreamingText(text);
      }
    });

    return () => {
      unlistenPromise.then((fn) => fn());
      unlistenStreamPromise.then((fn) => fn());
    };
  }, [setIsRunning, addMessage, markLastActionComplete, updateLastBashWithResult, setScreenshot, setApiKeySet, appendStreamingText, clearStreamingText]);

  const submit = useCallback(async () => {
    const text = inputText.trim();
    if (!text || isRunning) return;

    // add user message
    addMessage({ role: "user", content: text });
    setInputText("");

    try {
      await invoke("run_agent", { instructions: text, model: selectedModel });
    } catch (error) {
      addMessage({ role: "assistant", content: String(error), type: "error" });
      setIsRunning(false);
    }
  }, [inputText, isRunning, selectedModel, addMessage, setInputText, setIsRunning]);

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
