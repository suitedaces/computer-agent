# computer-agent MCP server

MCP server exposing `computer-agent` (taskhomie) tools to any MCP client - **opencode**, Claude Code, Cursor, etc.

Implements the same tool surface as `src-tauri/src/agent.rs` / `computer.rs` / `browser.rs` / `bash.rs` but over MCP stdio.

## Tools

| Tool | Mirrors | Description |
|------|---------|-------------|
| `computer_screenshot` | `ComputerControl::take_screenshot` | Base64 PNG, AI space 1280x800 |
| `computer_click` | `left_click`/`right_click`/`double_click` | AI coords 0-1280,0-800 |
| `computer_drag` | `left_click_drag` | from/to AI coords |
| `computer_type` | `type` | keyboard text |
| `computer_key` | `key`/`hold_key` | combo `ctrl+c`, `cmd+enter` |
| `computer_scroll` | `scroll` | direction + amount |
| `computer_move` | `mouse_move` | |
| `bash_run` | `BashExecutor::execute` | shell, blocked patterns, truncated |
| `browser_snapshot` | `BrowserClient::take_snapshot` | lightweight CDP via `http://127.0.0.1:9222` |
| `browser_navigate` | `navigate_page` | `url` or `back/forward/reload` |
| `browser_tabs` | `list_pages` | |
| `browser_click`/`browser_type` | `click`/`fill` | stub with fallback guidance |

For full a11y tree + `see_page`/`page_action`, run taskhomie app in **Background Mode** (Chrome ` --remote-debugging-port=9222`) or use `ego-browser` skill.

## Run

```bash
cd mcp-server
npm install
npm run build
node dist/index.js  # stdio
```

## Use with opencode

Add to `opencode.json`:

```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "computer-agent": {
      "type": "local",
      "command": ["node", "mcp-server/dist/index.js"],
      "enabled": true
    }
  }
}
```

Or from repo root:

```json
{
  "mcp": {
    "computer-agent": {
      "type": "local",
      "command": ["npx", "-y", "tsx", "mcp-server/src/index.ts"],
      "enabled": true
    }
  }
}
```

Then prompt: `use computer-agent to take a screenshot and click the login button`

## Permissions

Give accessibility permission on macOS (System Settings -> Privacy & Security -> Accessibility) for mouse/keyboard to work. Install optional `@nut/nut-js` for full computer control:

```bash
npm install --include=optional
```
