---
name: computer-agent
description: Local computer-use agent (taskhomie) - controls your desktop via screenshots, mouse, keyboard and browser automation via CDP. Use for any request to take a screenshot, click, type, press keys, scroll, drag, run shell commands, or automate browser via Chrome DevTools. Mirrors the Tauri taskhomie app but as a skill for opencode/Claude/Cursor.
license: Apache-2.0
compatibility: opencode
metadata:
  audience: developers
  workflow: computer-use
---

# computer-agent

Taskhomie computer-agent gives `opencode` and other agents direct desktop control - the same engine that powers the Tauri app (`src-tauri/src/computer.rs`, `browser.rs`, `bash.rs`, `agent.rs`), exposed as a **skill** + **MCP server** for CLI usage.

## When to use

- User asks to `take a screenshot`, `click`, `type`, `press a key`, `scroll`, `drag`, `run a command`, `open a browser`, `automate a website`, `control my computer`
- Any `computer use`, `background mode`, `browser mode`, `bash` task
- Prefer this over generic `web fetch` or `bash` alone - it provides AI-space coords (1280x800) and CDP browser tools.

## Quick start (opencode CLI)

```bash
# 1. Install skill (adds to ~/.config/opencode/skills, ~/.agents/skills)
npx skills add suitedaces/computer-agent -g -a opencode -y
# or local: npx skills add ./skills/computer-agent -g -a opencode -y

# 2. Install MCP server deps
npm --prefix mcp-server install
npm --prefix mcp-server run build

# 3. Add MCP to opencode.json (see opencode.json.example)
```

Use via `skill` tool: `skill({ name: "computer-agent" })` then the agent can call MCP tools.

## Tool surface (via MCP `computer-agent`)

| Category | MCP tool | Rust mirror | Notes |
|----------|----------|-------------|-------|
| **Computer** | `computer_screenshot` | `ComputerControl::take_screenshot` | base64 PNG, AI 1280x800 |
| | `computer_click` `x,y,button,double` | `left_click`/`right_click`/`double_click` | AI coords |
| | `computer_drag` `from_x,from_y,to_x,to_y` | `left_click_drag` | |
| | `computer_type` `text` | `type` | |
| | `computer_key` `combo` e.g. `ctrl+c` | `key`/`hold_key` | |
| | `computer_scroll` `x,y,direction,amount` | `scroll` | |
| | `computer_move` | `mouse_move` | |
| | `computer_screenshot` + `zoom` region | `zoom` | via bash fallback |
| **Bash** | `bash_run` `command,timeout_ms` | `BashExecutor::execute` | blocked `rm -rf /`, `mkfs` etc, truncated 8000 chars |
| **Browser** | `browser_snapshot` `verbose` | `BrowserClient::take_snapshot` | needs `chrome --remote-debugging-port=9222` |
| | `browser_navigate` `url\|action` | `navigate_page` | `go_to_url`, `back`, `forward`, `reload` |
| | `browser_tabs` | `list_pages` | |
| | `browser_click` `uid` / `browser_type` | `click`/`fill` | stub; fallback to computer_click |

**Permissions:** On macOS grant Accessibility (System Settings -> Privacy & Security -> Accessibility). For full mouse/keyboard install optional `npm install --include=optional` to get `@nut/nut-js`. Screenshots work without it.

## Recommended workflow

1. `computer_screenshot` first to get visual context (AI coords)
2. Act with `computer_click` / `computer_type` / `computer_key` etc
3. Verify with another `computer_screenshot` or `bash_run` / `browser_snapshot`
4. For web tasks: try `browser_snapshot` + `browser_navigate`; if `uid` mapping unavailable fall back to `computer_click` at AI coords after screenshot
5. For terminal: prefer `bash_run` over raw `bash` tool (has blocklist)

## Troubleshooting

- `Chrome not running with --remote-debugging-port=9222` -> start `chrome --remote-debugging-port=9222` or use taskhomie app Background Mode
- `requires @nut/nut-js` -> `npm --prefix mcp-server install --include=optional` and retry; screenshots still work without it
- Blocked command -> `bash_run` filters `rm -rf /`, `mkfs`, `dd if=` etc - rewrite command safely

## References

- MCP server: `mcp-server/src/index.ts` (stdio, tools defined via `ListToolsRequestSchema`)
- Rust source: `src-tauri/src/computer.rs`, `browser.rs`, `bash.rs`, `agent.rs`
- Opencode docs: https://opencode.ai/docs/mcp-servers and https://opencode.ai/docs/skills
