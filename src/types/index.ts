export interface AgentUpdate {
  update_type: "started" | "thinking" | "response" | "action" | "screenshot" | "finished" | "error" | "bash_result" | "user_message" | "browser_result" | "web_result" | "tool";
  message: string;
  tool_name?: string;
  tool_input?: Record<string, unknown>;
  action?: ComputerAction; // deprecated, use tool_input
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
  type?: "thinking" | "action" | "error" | "info" | "bash" | "speak";
  audioData?: string; // base64 audio for speak messages
  action?: ComputerAction;
  screenshot?: string;
  pending?: boolean;
  bashOutput?: string;
  exitCode?: number;
}

export type ModelId = "claude-haiku-4-5-20251001" | "claude-sonnet-4-5" | "claude-opus-4-5";

export type AgentMode = "computer" | "browser";

export interface ConversationMeta {
  id: string;
  title: string;
  created_at: number;
  updated_at: number;
  model: string;
  mode: string;
  message_count: number;
  total_input_tokens: number;
  total_output_tokens: number;
}

// anthropic api content block format
export interface ContentBlock {
  type: "text" | "image" | "tool_use" | "tool_result" | "thinking" | "redacted_thinking";
  text?: string;
  source?: { type: string; media_type: string; data: string };
  id?: string;
  name?: string;
  input?: unknown;
  tool_use_id?: string;
  content?: ContentBlock[];
  thinking?: string;
  signature?: string;
  data?: string;
}

export interface ApiMessage {
  role: "user" | "assistant";
  content: ContentBlock[];
}

export interface Conversation {
  id: string;
  title: string;
  created_at: number;
  updated_at: number;
  model: string;
  mode: string;
  messages: ApiMessage[];
  turn_usage: unknown[];
  total_input_tokens: number;
  total_output_tokens: number;
  voice_mode: boolean;
}

export interface AgentState {
  isRunning: boolean;
  messages: ChatMessage[];
  apiKeySet: boolean;
  inputText: string;
  selectedModel: ModelId;
  selectedMode: AgentMode;
  voiceMode: boolean;
  streamingText: string;
  streamingThinking: string;
  conversationId: string | null;

  setIsRunning: (running: boolean) => void;
  addMessage: (msg: Omit<ChatMessage, "id" | "timestamp">) => void;
  markLastActionComplete: (screenshot?: string) => void;
  updateLastBashWithResult: (output: string, exitCode?: number) => void;
  setApiKeySet: (set: boolean) => void;
  setInputText: (text: string) => void;
  setSelectedModel: (model: ModelId) => void;
  setSelectedMode: (mode: AgentMode) => void;
  setVoiceMode: (voiceMode: boolean) => void;
  clearMessages: () => void;
  setMessages: (messages: ChatMessage[]) => void;
  appendStreamingText: (text: string) => void;
  clearStreamingText: () => void;
  appendStreamingThinking: (text: string) => void;
  clearStreamingThinking: () => void;
  setConversationId: (id: string | null) => void;
}
