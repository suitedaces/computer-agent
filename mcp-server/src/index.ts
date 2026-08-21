#!/usr/bin/env node
import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
} from "@modelcontextprotocol/sdk/types.js";
import { z } from "zod";
import { takeScreenshot, mouseMove, click, drag, typeText, pressKey, scroll } from "./tools/computer.js";
import { bashRun } from "./tools/bash.js";
import { browserSnapshot, browserNavigate, browserTabs, browserClick, browserType } from "./tools/browser.js";

const server = new Server(
  {
    name: "computer-agent",
    version: "0.1.0",
  },
  {
    capabilities: {
      tools: {},
    },
  }
);

server.setRequestHandler(ListToolsRequestSchema, async () => {
  return {
    tools: [
      {
        name: "computer_screenshot",
        description: "Take a screenshot of the primary monitor. Returns base64 PNG (1280x800 AI space). Use before any mouse action to get coordinates.",
        inputSchema: { type: "object", properties: {}, additionalProperties: false },
      },
      {
        name: "computer_click",
        description: "Click at AI coordinates (0-1280 x 0-800). Maps to real screen. Matches computer.rs left_click/right_click/double_click.",
        inputSchema: {
          type: "object",
          properties: {
            x: { type: "number", description: "AI x 0-1280" },
            y: { type: "number", description: "AI y 0-800" },
            button: { type: "string", enum: ["left", "right", "middle"], default: "left" },
            double: { type: "boolean", description: "double click", default: false },
          },
          required: ["x", "y"],
        },
      },
      {
        name: "computer_drag",
        description: "Drag from start to end (AI coords). Mirrors left_click_drag.",
        inputSchema: {
          type: "object",
          properties: {
            from_x: { type: "number" },
            from_y: { type: "number" },
            to_x: { type: "number" },
            to_y: { type: "number" },
          },
          required: ["from_x", "from_y", "to_x", "to_y"],
        },
      },
      {
        name: "computer_type",
        description: "Type text via keyboard (enigo / nut.js). Mirrors computer.rs type action.",
        inputSchema: {
          type: "object",
          properties: { text: { type: "string" } },
          required: ["text"],
        },
      },
      {
        name: "computer_key",
        description: "Press key combo e.g. 'ctrl+c', 'cmd+enter', 'escape', 'tab'. Mirrors key/hold_key.",
        inputSchema: {
          type: "object",
          properties: { combo: { type: "string" } },
          required: ["combo"],
        },
      },
      {
        name: "computer_scroll",
        description: "Scroll at position. direction up/down/left/right, amount 1-10. Mirrors scroll.",
        inputSchema: {
          type: "object",
          properties: {
            x: { type: "number", default: 640 },
            y: { type: "number", default: 400 },
            direction: { type: "string", enum: ["up", "down", "left", "right"] },
            amount: { type: "number", default: 3 },
          },
          required: ["direction"],
        },
      },
      {
        name: "computer_move",
        description: "Move mouse to AI coordinates without clicking.",
        inputSchema: {
          type: "object",
          properties: { x: { type: "number" }, y: { type: "number" } },
          required: ["x", "y"],
        },
      },
      {
        name: "bash_run",
        description: "Run a shell command (blocked patterns filtered like bash.rs). Returns stdout/stderr + exit code. Use for terminal tasks (Background Mode).",
        inputSchema: {
          type: "object",
          properties: {
            command: { type: "string" },
            timeout_ms: { type: "number", default: 30000 },
          },
          required: ["command"],
        },
      },
      {
        name: "browser_snapshot",
        description: "Get browser pages / lightweight snapshot via CDP (http://127.0.0.1:9222). For full a11y tree use taskhomie app Background Mode.",
        inputSchema: {
          type: "object",
          properties: { verbose: { type: "boolean", default: false } },
        },
      },
      {
        name: "browser_navigate",
        description: "Navigate browser. Provide url to open new tab, or action back/forward/reload.",
        inputSchema: {
          type: "object",
          properties: {
            url: { type: "string" },
            action: { type: "string", enum: ["back", "forward", "reload"] },
          },
        },
      },
      {
        name: "browser_tabs",
        description: "List open Chrome tabs via CDP.",
        inputSchema: { type: "object", properties: {} },
      },
      {
        name: "browser_click",
        description: "Click a11y element by uid (from browser_snapshot). Stub - falls back to computer_click guidance.",
        inputSchema: {
          type: "object",
          properties: { uid: { type: "string" } },
          required: ["uid"],
        },
      },
      {
        name: "browser_type",
        description: "Type into a11y element.",
        inputSchema: {
          type: "object",
          properties: { uid: { type: "string" }, text: { type: "string" } },
          required: ["uid", "text"],
        },
      },
    ],
  };
});

server.setRequestHandler(CallToolRequestSchema, async (request) => {
  const { name, arguments: args } = request.params;
  try {
    switch (name) {
      case "computer_screenshot": {
        const b64 = await takeScreenshot();
        return {
          content: [
            { type: "text", text: `Screenshot taken (${b64.length} b64 chars, AI space 1280x800).` },
            { type: "image", data: b64, mimeType: "image/png" },
          ],
        };
      }
      case "computer_click": {
        const { x, y, button, double } = z.object({ x: z.number(), y: z.number(), button: z.enum(["left", "right", "middle"]).optional(), double: z.boolean().optional() }).parse(args);
        const res = await click(x, y, (button as any) ?? "left", !!double);
        return { content: [{ type: "text", text: res }] };
      }
      case "computer_drag": {
        const { from_x, from_y, to_x, to_y } = z.object({ from_x: z.number(), from_y: z.number(), to_x: z.number(), to_y: z.number() }).parse(args);
        const res = await drag([from_x, from_y], [to_x, to_y]);
        return { content: [{ type: "text", text: res }] };
      }
      case "computer_type": {
        const { text } = z.object({ text: z.string() }).parse(args);
        const res = await typeText(text);
        return { content: [{ type: "text", text: res }] };
      }
      case "computer_key": {
        const { combo } = z.object({ combo: z.string() }).parse(args);
        const res = await pressKey(combo);
        return { content: [{ type: "text", text: res }] };
      }
      case "computer_scroll": {
        const { x, y, direction, amount } = z.object({ x: z.number().optional(), y: z.number().optional(), direction: z.string(), amount: z.number().optional() }).parse(args);
        const res = await scroll(x ?? 640, y ?? 400, direction, amount ?? 3);
        return { content: [{ type: "text", text: res }] };
      }
      case "computer_move": {
        const { x, y } = z.object({ x: z.number(), y: z.number() }).parse(args);
        const res = await mouseMove(x, y);
        return { content: [{ type: "text", text: res }] };
      }
      case "bash_run": {
        const { command, timeout_ms } = z.object({ command: z.string(), timeout_ms: z.number().optional() }).parse(args);
        const out = await bashRun(command, timeout_ms ?? 30000);
        return { content: [{ type: "text", text: out }] };
      }
      case "browser_snapshot": {
        const { verbose } = z.object({ verbose: z.boolean().optional() }).parse(args ?? {});
        const out = await browserSnapshot(!!verbose);
        return { content: [{ type: "text", text: out }] };
      }
      case "browser_navigate": {
        const { url, action } = z.object({ url: z.string().optional(), action: z.string().optional() }).parse(args ?? {});
        const out = await browserNavigate(url, action);
        return { content: [{ type: "text", text: out }] };
      }
      case "browser_tabs": {
        const out = await browserTabs();
        return { content: [{ type: "text", text: out }] };
      }
      case "browser_click": {
        const { uid } = z.object({ uid: z.string() }).parse(args);
        const out = await browserClick(uid);
        return { content: [{ type: "text", text: out }] };
      }
      case "browser_type": {
        const { uid, text } = z.object({ uid: z.string(), text: z.string() }).parse(args);
        const out = await browserType(uid, text);
        return { content: [{ type: "text", text: out }] };
      }
      default:
        throw new Error(`Unknown tool: ${name}`);
    }
  } catch (e: any) {
    return { content: [{ type: "text", text: `Error: ${e.message}` }], isError: true };
  }
});

async function main() {
  const transport = new StdioServerTransport();
  await server.connect(transport);
  console.error("computer-agent MCP server running on stdio");
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
