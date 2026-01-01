// centralized tool formatting - single source of truth for all tool display
// used by both streaming (useAgent) and history loading (convertApiToChat)

import type { ChatMessage, ComputerAction } from "../types";

export interface ToolInput {
  // computer tool
  action?: string;
  coordinate?: [number, number];
  start_coordinate?: [number, number];
  text?: string;
  scroll_direction?: string;
  scroll_amount?: number;
  // bash tool
  command?: string;
  // speak tool (text reused)
  // consolidated browser tools (see_page, page_action, browser_navigate)
  // see_page
  screenshot?: boolean;
  list_tabs?: boolean;
  // page_action
  click?: string;
  double_click?: string;
  type_into?: string;
  hover?: string;
  drag_from_to?: string[];
  press_key?: string;
  scroll?: string;
  scroll_pixels?: number;
  fill_form?: Array<{ element: string; text: string }>;
  dialog?: string;
  dialog_text?: string;
  // browser_navigate
  go_to_url?: string;
  go_back?: boolean;
  go_forward?: boolean;
  reload?: boolean;
  reload_skip_cache?: boolean;
  open_new_tab?: string;
  switch_to_tab?: number;
  close_tab?: number;
  wait_for_text?: string;
  wait_timeout_ms?: number;
  // legacy browser tools (for backwards compat with old conversations)
  uid?: string;
  value?: string;
  key?: string;
  url?: string;
  type?: string;
  dblClick?: boolean;
  pageIdx?: number;
  verbose?: boolean;
  direction?: string; // legacy scroll direction
  // web search/fetch tools (server-side)
  query?: string;
}

interface FormatOptions {
  pending?: boolean; // true = present tense (streaming), false = past tense (history)
}

// format tool call for display - returns content string and optional parsed action
export function formatToolMessage(
  toolName: string,
  input: ToolInput,
  options: FormatOptions = {}
): { content: string; type: ChatMessage["type"]; action?: ComputerAction } {
  const { pending = false } = options;

  switch (toolName) {
    case "computer":
      return formatComputerTool(input, pending);
    case "bash":
      return {
        content: String(input.command || ""),
        type: "bash",
      };
    case "speak":
      return {
        content: String(input.text || ""),
        type: "speak",
      };
    case "web_search": {
      const query = input.query;
      if (query) {
        const preview = query.length > 40 ? `${query.slice(0, 40)}...` : query;
        return {
          content: `${pending ? "Searching" : "Searched"}: "${preview}"`,
          type: "action",
        };
      }
      return {
        content: pending ? "Searching the web" : "Searched the web",
        type: "action",
      };
    }
    case "web_fetch": {
      const url = input.url;
      if (url) {
        return {
          content: `${pending ? "Fetching" : "Fetched"} ||${url}||`,
          type: "action",
        };
      }
      return {
        content: pending ? "Fetching page" : "Fetched page",
        type: "action",
      };
    }
    default:
      // browser tools and unknown
      return {
        content: formatBrowserTool(toolName, input, pending),
        type: "action",
      };
  }
}

function formatComputerTool(
  input: ToolInput,
  pending: boolean
): { content: string; type: "action"; action: ComputerAction } {
  const action = input.action || "";
  const coord = input.coordinate;
  const text = input.text;

  const actionObj: ComputerAction = {
    action,
    coordinate: coord,
    start_coordinate: input.start_coordinate,
    text,
    scroll_direction: input.scroll_direction as ComputerAction["scroll_direction"],
    scroll_amount: input.scroll_amount,
  };

  let content: string;
  switch (action) {
    case "screenshot":
      content = pending ? "Taking screenshot" : "Took screenshot";
      break;
    case "mouse_move":
      content = coord
        ? `${pending ? "Moving" : "Moved"} mouse to (${coord[0]}, ${coord[1]})`
        : `${pending ? "Moving" : "Moved"} mouse`;
      break;
    case "left_click":
      content = coord
        ? `${pending ? "Clicking" : "Clicked"} at (${coord[0]}, ${coord[1]})`
        : pending ? "Clicking" : "Clicked";
      break;
    case "right_click":
      content = pending ? "Right clicking" : "Right clicked";
      break;
    case "double_click":
      content = coord
        ? `${pending ? "Double clicking" : "Double clicked"} at (${coord[0]}, ${coord[1]})`
        : pending ? "Double clicking" : "Double clicked";
      break;
    case "triple_click":
      content = pending ? "Triple clicking" : "Triple clicked";
      break;
    case "middle_click":
      content = pending ? "Middle clicking" : "Middle clicked";
      break;
    case "left_click_drag":
      if (input.start_coordinate && coord) {
        content = `${pending ? "Dragging" : "Dragged"} from (${input.start_coordinate[0]}, ${input.start_coordinate[1]}) to (${coord[0]}, ${coord[1]})`;
      } else {
        content = pending ? "Dragging" : "Dragged";
      }
      break;
    case "type":
      if (text) {
        const preview = text.length > 30 ? `${text.slice(0, 30)}...` : text;
        content = `${pending ? "Typing" : "Typed"}: "${preview}"`;
      } else {
        content = pending ? "Typing" : "Typed";
      }
      break;
    case "key":
      content = text
        ? `${pending ? "Pressing" : "Pressed"} ${text}`
        : pending ? "Pressing key" : "Pressed key";
      break;
    case "scroll": {
      const dir = input.scroll_direction || "down";
      content = `${pending ? "Scrolling" : "Scrolled"} ${dir}`;
      break;
    }
    case "wait":
      content = pending ? "Waiting" : "Waited";
      break;
    default:
      content = action;
  }

  return { content, type: "action", action: actionObj };
}

function formatBrowserTool(name: string, input: ToolInput, pending: boolean): string {
  switch (name) {
    // consolidated browser tools
    case "see_page": {
      if (input.screenshot) {
        return pending ? "Taking screenshot" : "Took screenshot";
      }
      if (input.list_tabs) {
        return pending ? "Listing tabs" : "Listed tabs";
      }
      return pending ? "Getting page elements" : "Got page elements";
    }

    case "page_action": {
      if (input.click) {
        return pending ? "Clicking" : "Clicked";
      }
      if (input.double_click) {
        return pending ? "Double clicking" : "Double clicked";
      }
      if (input.type_into) {
        const text = input.text;
        if (text) {
          const preview = text.length > 20 ? `${text.slice(0, 20)}...` : text;
          return `${pending ? "Typing" : "Typed"}: "${preview}"`;
        }
        return pending ? "Typing" : "Typed";
      }
      if (input.hover) {
        return pending ? "Hovering" : "Hovered";
      }
      if (input.drag_from_to) {
        return pending ? "Dragging" : "Dragged";
      }
      if (input.press_key) {
        return `${pending ? "Pressing" : "Pressed"} ${input.press_key}`;
      }
      if (input.scroll) {
        return `${pending ? "Scrolling" : "Scrolled"} ${input.scroll}`;
      }
      if (input.fill_form) {
        const count = input.fill_form.length;
        return `${pending ? "Filling" : "Filled"} ${count} field${count !== 1 ? "s" : ""}`;
      }
      if (input.dialog) {
        return input.dialog === "accept"
          ? (pending ? "Accepting dialog" : "Accepted dialog")
          : (pending ? "Dismissing dialog" : "Dismissed dialog");
      }
      return pending ? "Interacting" : "Interacted";
    }

    case "browser_navigate": {
      if (input.go_to_url) {
        return `${pending ? "Navigating to" : "Navigated to"} ||${input.go_to_url}||`;
      }
      if (input.go_back) {
        return pending ? "Going back" : "Went back";
      }
      if (input.go_forward) {
        return pending ? "Going forward" : "Went forward";
      }
      if (input.reload || input.reload_skip_cache) {
        return pending ? "Reloading page" : "Reloaded page";
      }
      if (input.open_new_tab) {
        return `${pending ? "Opening new tab" : "Opened new tab"} ||${input.open_new_tab}||`;
      }
      if (input.switch_to_tab !== undefined) {
        return `${pending ? "Switching to" : "Switched to"} tab ${input.switch_to_tab}`;
      }
      if (input.close_tab !== undefined) {
        return `${pending ? "Closing" : "Closed"} tab ${input.close_tab}`;
      }
      if (input.wait_for_text) {
        const preview = input.wait_for_text.length > 20
          ? `${input.wait_for_text.slice(0, 20)}...`
          : input.wait_for_text;
        return `${pending ? "Waiting for" : "Waited for"} "${preview}"`;
      }
      return pending ? "Navigating" : "Navigated";
    }

    // legacy browser tools (for old conversations)
    case "take_snapshot":
      return pending ? "Getting page elements" : "Got page elements";
    case "click": {
      const dbl = input.dblClick;
      return dbl
        ? (pending ? "Double clicking" : "Double clicked")
        : (pending ? "Clicking" : "Clicked");
    }
    case "hover":
      return pending ? "Hovering" : "Hovered";
    case "fill": {
      const val = input.value;
      if (val) {
        const preview = val.length > 20 ? `${val.slice(0, 20)}...` : val;
        return `${pending ? "Typing" : "Typed"}: "${preview}"`;
      }
      return pending ? "Typing" : "Typed";
    }
    case "press_key": {
      const key = input.key;
      return key
        ? `${pending ? "Pressing" : "Pressed"} ${key}`
        : pending ? "Pressing key" : "Pressed key";
    }
    case "navigate_page": {
      const type = input.type;
      switch (type) {
        case "goto":
        case "url": {
          const url = input.url;
          if (url) {
            return `${pending ? "Navigating to" : "Navigated to"} ||${url}||`;
          }
          return pending ? "Navigating" : "Navigated";
        }
        case "back":
          return pending ? "Going back" : "Went back";
        case "forward":
          return pending ? "Going forward" : "Went forward";
        case "reload":
          return pending ? "Reloading page" : "Reloaded page";
        default:
          return pending ? "Navigating" : "Navigated";
      }
    }
    case "wait_for": {
      const text = input.text;
      if (text) {
        const preview = text.length > 20 ? `${text.slice(0, 20)}...` : text;
        return `${pending ? "Waiting for" : "Waited for"} "${preview}"`;
      }
      return pending ? "Waiting" : "Waited";
    }
    case "new_page": {
      const url = input.url;
      if (url) {
        return `${pending ? "Opening new tab" : "Opened new tab"} ||${url}||`;
      }
      return pending ? "Opening new tab" : "Opened new tab";
    }
    case "list_pages":
      return pending ? "Listing tabs" : "Listed tabs";
    case "select_page": {
      const idx = input.pageIdx;
      return idx !== undefined
        ? `${pending ? "Switching to" : "Switched to"} tab ${idx}`
        : pending ? "Switching tab" : "Switched tab";
    }
    case "close_page": {
      const idx = input.pageIdx;
      return idx !== undefined
        ? `${pending ? "Closing" : "Closed"} tab ${idx}`
        : pending ? "Closing tab" : "Closed tab";
    }
    case "scroll": {
      const dir = input.direction || input.scroll_direction || "down";
      return `${pending ? "Scrolling" : "Scrolled"} ${dir}`;
    }
    case "drag":
      return pending ? "Dragging" : "Dragged";
    case "fill_form":
      return pending ? "Filling form" : "Filled form";
    case "handle_dialog": {
      const action = input.action;
      switch (action) {
        case "accept":
          return pending ? "Accepting dialog" : "Accepted dialog";
        case "dismiss":
          return pending ? "Dismissing dialog" : "Dismissed dialog";
        default:
          return pending ? "Handling dialog" : "Handled dialog";
      }
    }
    case "screenshot":
      return pending ? "Taking screenshot" : "Took screenshot";
    case "upload_file":
      return pending ? "Uploading file" : "Uploaded file";
    default:
      // fallback: convert snake_case to readable
      return name.replace(/_/g, " ").replace(/\b\w/g, c => c.toUpperCase());
  }
}

// helper to strip voice_input tags from user messages
export function stripVoiceInputTags(text: string): string {
  return text.replace(/<\/?voice_input>/g, "");
}
