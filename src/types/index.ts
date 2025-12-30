export interface AgentUpdate {
  update_type: "started" | "thinking" | "response" | "action" | "screenshot" | "finished" | "error" | "bash_result" | "user_message" | "browser_result";
  message: string;
  action?: ComputerAction;
  screenshot?: string;
  bash_command?: string;
  exit_code?: number;
  mode?: "computer" | "browser";
}

export interface ComputerAction {
  action: string;
  coordinate?: [number, number];
  start_coordinate?: [number, number];
  text?: string;
  scroll_direction?: "up" | "down" | "left" | "right";
  scroll_amount?: number;
}

export interface ChatMessage {
  id: string;
  role: "user" | "assistant";
  content: string;
  timestamp: Date;
  type?: "thinking" | "action" | "error" | "info" | "bash";
  action?: ComputerAction;
  screenshot?: string;
  pending?: boolean;
  bashOutput?: string;
  exitCode?: number;
}

export type ModelId = "claude-haiku-4-5-20251001" | "claude-sonnet-4-5" | "claude-opus-4-5";

export type AgentMode = "computer" | "browser";

export interface AgentState {
  isRunning: boolean;
  messages: ChatMessage[];
  apiKeySet: boolean;
  inputText: string;
  selectedModel: ModelId;
  selectedMode: AgentMode;
  streamingText: string;
  streamingThinking: string;

  setIsRunning: (running: boolean) => void;
  addMessage: (msg: Omit<ChatMessage, "id" | "timestamp">) => void;
  markLastActionComplete: (screenshot?: string) => void;
  updateLastBashWithResult: (output: string, exitCode?: number) => void;
  setApiKeySet: (set: boolean) => void;
  setInputText: (text: string) => void;
  setSelectedModel: (model: ModelId) => void;
  setSelectedMode: (mode: AgentMode) => void;
  clearMessages: () => void;
  appendStreamingText: (text: string) => void;
  clearStreamingText: () => void;
  appendStreamingThinking: (text: string) => void;
  clearStreamingThinking: () => void;
}
