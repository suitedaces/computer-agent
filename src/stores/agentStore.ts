import { create } from "zustand";
import { AgentMode, AgentState, ModelId } from "../types";

function toPastTense(text: string): string {
  const replacements: [RegExp, string][] = [
    // computer mode actions
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
    // browser mode actions
    [/^Taking snapshot$/, "Took snapshot"],
    [/^Clicking$/, "Clicked"],
    [/^Double clicking$/, "Double clicked"],
    [/^Hovering$/, "Hovered"],
    [/^Filling:/, "Filled:"],
    [/^Filling field$/, "Filled field"],
    [/^Filling form$/, "Filled form"],
    [/^Pressing /, "Pressed "],
    [/^Navigating to/, "Navigated to"],
    [/^Navigating$/, "Navigated"],
    [/^Going back$/, "Went back"],
    [/^Going forward$/, "Went forward"],
    [/^Reloading page$/, "Reloaded page"],
    [/^Waiting for/, "Waited for"],
    [/^Opening new tab/, "Opened new tab"],
    [/^Listing tabs$/, "Listed tabs"],
    [/^Switching to tab/, "Switched to tab"],
    [/^Switching tab$/, "Switched tab"],
    [/^Closing tab/, "Closed tab"],
    [/^Dragging$/, "Dragged"],
    [/^Accepting dialog$/, "Accepted dialog"],
    [/^Dismissing dialog$/, "Dismissed dialog"],
    [/^Handling dialog$/, "Handled dialog"],
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
  apiKeySet: false,
  inputText: "",
  selectedModel: "claude-haiku-4-5-20251001" as ModelId,
  selectedMode: "browser" as AgentMode,
  voiceMode: false,
  streamingText: "",
  streamingThinking: "",
  conversationId: null,

  setIsRunning: (running) => set({ isRunning: running }),

  addMessage: (msg) =>
    set((state) => {
      const now = Date.now();
      const last = state.messages[state.messages.length - 1];
      const msgType = msg.type;
      const isAssistant = msg.role === "assistant";
      const isDedupeType = msgType === "info" || msgType === "speak";

      if (isAssistant && isDedupeType && last && last.role === "assistant") {
        const lastType = last.type;
        const lastContent = last.content.trim();
        const nextContent = msg.content.trim();
        const lastTime = last.timestamp instanceof Date ? last.timestamp.getTime() : new Date(last.timestamp).getTime();
        const recent = now - lastTime < 10000;

        if (recent && lastContent === nextContent && (lastType === "info" || lastType === "speak")) {
          // drop duplicate text after a recent speak, or repeated info/speak messages
          if (lastType === msgType || (lastType === "speak" && msgType === "info")) {
            return state;
          }

          // if speak arrives after identical text, upgrade the last message to speak
          if (lastType === "info" && msgType === "speak") {
            const messages = [...state.messages];
            messages[messages.length - 1] = {
              ...last,
              ...msg,
              id: last.id,
              timestamp: last.timestamp,
            };
            return { messages };
          }
        }
      }

      return {
        messages: [
          ...state.messages,
          {
            ...msg,
            id: crypto.randomUUID(),
            timestamp: new Date(),
            pending: (msg.type === "action" || msg.type === "bash") ? true : undefined,
          },
        ],
      };
    }),

  markLastActionComplete: (screenshot?: string) =>
    set((state) => {
      const messages = [...state.messages];
      for (let i = messages.length - 1; i >= 0; i--) {
        if (messages[i].type === "action" && messages[i].pending) {
          const content = toPastTense(messages[i].content);
          messages[i] = { ...messages[i], pending: false, content, screenshot };
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

  setApiKeySet: (apiKeySet) => set({ apiKeySet }),

  setInputText: (inputText) => set({ inputText }),

  setSelectedModel: (selectedModel) => set({ selectedModel }),

  setSelectedMode: (selectedMode) => set({ selectedMode }),

  setVoiceMode: (voiceMode) => set({ voiceMode }),

  clearMessages: () => set({ messages: [], conversationId: null, streamingText: "", streamingThinking: "" }),

  setMessages: (messages) => set({ messages }),

  appendStreamingText: (text) => set((state) => ({
    streamingText: state.streamingText + text,
  })),

  clearStreamingText: () => set({ streamingText: "" }),

  appendStreamingThinking: (text) => set((state) => ({
    streamingThinking: state.streamingThinking + text,
  })),

  clearStreamingThinking: () => set({ streamingThinking: "" }),

  setConversationId: (id) => set({ conversationId: id }),
}));
