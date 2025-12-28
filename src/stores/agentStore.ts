import { create } from "zustand";
import { AgentState } from "../types";

export const useAgentStore = create<AgentState>((set) => ({
  isRunning: false,
  messages: [],
  screenshot: null,
  apiKeySet: false,
  inputText: "",

  setIsRunning: (running) => set({ isRunning: running }),

  addMessage: (msg) =>
    set((state) => ({
      messages: [
        ...state.messages,
        {
          ...msg,
          id: crypto.randomUUID(),
          timestamp: new Date(),
          pending: msg.type === "action" ? true : undefined,
        },
      ],
    })),

  markLastActionComplete: () =>
    set((state) => {
      const messages = [...state.messages];
      for (let i = messages.length - 1; i >= 0; i--) {
        if (messages[i].type === "action" && messages[i].pending) {
          messages[i] = { ...messages[i], pending: false };
          break;
        }
      }
      return { messages };
    }),

  setScreenshot: (screenshot) => set({ screenshot }),

  setApiKeySet: (apiKeySet) => set({ apiKeySet }),

  setInputText: (inputText) => set({ inputText }),

  clearMessages: () => set({ messages: [] }),
}));
