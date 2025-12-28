export interface AgentUpdate {
  update_type: "started" | "thinking" | "action" | "screenshot" | "finished" | "error" | "bash_result";
  message: string;
  action?: ComputerAction;
  screenshot?: string;
  bash_command?: string;
  exit_code?: number;
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

export interface AgentState {
  isRunning: boolean;
  messages: ChatMessage[];
  screenshot: string | null;
  apiKeySet: boolean;
  inputText: string;

  setIsRunning: (running: boolean) => void;
  addMessage: (msg: Omit<ChatMessage, "id" | "timestamp">) => void;
  markLastActionComplete: () => void;
  updateLastBashWithResult: (output: string, exitCode?: number) => void;
  setScreenshot: (screenshot: string | null) => void;
  setApiKeySet: (set: boolean) => void;
  setInputText: (text: string) => void;
  clearMessages: () => void;
}
