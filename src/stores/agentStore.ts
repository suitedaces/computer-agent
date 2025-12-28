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
        },
      ],
    })),

  setScreenshot: (screenshot) => set({ screenshot }),

  setApiKeySet: (apiKeySet) => set({ apiKeySet }),

  setInputText: (inputText) => set({ inputText }),

  clearMessages: () => set({ messages: [] }),
}));
