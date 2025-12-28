import { create } from "zustand";
import { AgentState, ModelId } from "../types";

function toPastTense(text: string): string {
  const replacements: [RegExp, string][] = [
    [/^Taking screenshot$/, "Took screenshot"],
    [/^Moving mouse to/, "Moved mouse to"],
    [/^Clicking at/, "Clicked at"],
    [/^Double clicking at/, "Double clicked at"],
    [/^Right click$/, "Right clicked"],
    [/^Left click$/, "Left clicked"],
    [/^Typing:/, "Typed:"],
    [/^Pressing key:/, "Pressed key:"],
    [/^Scrolling/, "Scrolled"],
    [/^Waiting$/, "Waited"],
    [/^\$ /, "$ "],  // bash commands stay the same
  ];

  for (const [pattern, replacement] of replacements) {
    if (pattern.test(text)) {
      return text.replace(pattern, replacement);
    }
  }
  return text;
}

export const useAgentStore = create<AgentState>((set) => ({
  isRunning: false,
  messages: [],
  screenshot: null,
  apiKeySet: false,
  inputText: "",
  selectedModel: "claude-haiku-4-5-20251001" as ModelId,
  streamingText: "",
  streamingThinking: "",

  setIsRunning: (running) => set({ isRunning: running }),

  addMessage: (msg) =>
    set((state) => ({
      messages: [
        ...state.messages,
        {
          ...msg,
          id: crypto.randomUUID(),
          timestamp: new Date(),
          pending: (msg.type === "action" || msg.type === "bash") ? true : undefined,
        },
      ],
    })),

  markLastActionComplete: () =>
    set((state) => {
      const messages = [...state.messages];
      for (let i = messages.length - 1; i >= 0; i--) {
        if (messages[i].type === "action" && messages[i].pending) {
          const content = toPastTense(messages[i].content);
          messages[i] = { ...messages[i], pending: false, content };
          break;
        }
      }
      return { messages };
    }),

  updateLastBashWithResult: (output, exitCode) =>
    set((state) => {
      const messages = [...state.messages];
      for (let i = messages.length - 1; i >= 0; i--) {
        if (messages[i].type === "bash" && messages[i].pending) {
          messages[i] = { ...messages[i], pending: false, bashOutput: output, exitCode };
          break;
        }
      }
      return { messages };
    }),

  setScreenshot: (screenshot) => set({ screenshot }),

  setApiKeySet: (apiKeySet) => set({ apiKeySet }),

  setInputText: (inputText) => set({ inputText }),

  setSelectedModel: (selectedModel) => set({ selectedModel }),

  clearMessages: () => set({ messages: [] }),

  appendStreamingText: (text) => set((state) => ({
    streamingText: state.streamingText + text,
  })),

  clearStreamingText: () => set({ streamingText: "" }),

  appendStreamingThinking: (text) => set((state) => ({
    streamingThinking: state.streamingThinking + text,
  })),

  clearStreamingThinking: () => set({ streamingThinking: "" }),
}));
