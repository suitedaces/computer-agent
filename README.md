# Taskhomie (Computer Use AI Agent)
<img width="846" height="606" alt="Screenshot 2025-12-29 at 2 06 38 AM" src="https://github.com/user-attachments/assets/b5b7de82-ec58-424f-af68-e9287a6422d6" />

Local AI agent that controls your computer. Give it natural language instructions and watch it take screenshots, move your mouse, click, type, and run terminal commands.

Built with Tauri, React, and Rust.

<a href="https://trendshift.io/repositories/21250?utm_source=trendshift-badge&amp;utm_medium=badge&amp;utm_campaign=badge-trendshift-21250" target="_blank" rel="noopener noreferrer"><img src="https://trendshift.io/api/badge/trendshift/repositories/21250/daily?language=Rust" alt="suitedaces%2Fcomputer-agent | Trendshift" width="250" height="55"/></a>
<a href="https://trendshift.io/repositories/21250?utm_source=trendshift-badge&amp;utm_medium=badge&amp;utm_campaign=badge-trendshift-21250" target="_blank" rel="noopener noreferrer"><img src="https://trendshift.io/api/badge/trendshift/repositories/21250/weekly?language=Rust" alt="suitedaces%2Fcomputer-agent | Trendshift" width="250" height="55"/></a>


## Demo

https://github.com/user-attachments/assets/8edd92a7-7d3e-472a-9e48-3b561f0257d6

Here, I used it to autonomously read and reply to tweets, lol. This is purely for demonstration/research, you should not attempt to do the same, lol.

## Modes

**Computer Use Mode** - Takes over your screen. Sees what you see via screenshots and controls your cursor and keyboard directly. Use when the task spans multiple apps or needs full desktop access. You step away while it works.

**Background Mode** - Runs async while you do other things. Uses Chrome DevTools Protocol for web automation and terminal for everything else. Doesn't touch your mouse or keyboard. Faster and more reliable for web + terminal tasks.

## Setup

**Requirements:**
- Rust & Cargo
- Node.js & npm
- Anthropic API key

```bash
# install deps
npm install

# add your api key
echo "ANTHROPIC_API_KEY=your-key-here" > .env

# run dev
npm run tauri dev

# or build for production
npm run tauri build
```

On macOS, you'll need to grant accessibility permissions when prompted (System Settings → Privacy & Security → Accessibility).

## Use with opencode CLI (new)

This repo now works headless via **MCP server** + **Agent Skill** for `opencode`, Claude Code, Cursor, etc. No Tauri window needed.

**1. Install skill (opencode):**
```bash
npx skills add suitedaces/computer-agent -g -a opencode -y
# or locally
npx skills add ./skills/computer-agent -g -a opencode -y
# verify
npx skills ls -g -a opencode
```

Skill lives at `skills/computer-agent/SKILL.md:1` and is discovered by opencode at `~/.config/opencode/skills` / `~/.agents/skills` (https://opencode.ai/docs/skills).

**2. Run MCP server:**
```bash
cd mcp-server
npm install          # installs @modelcontextprotocol/sdk, screenshot-desktop
npm run build
# optional for mouse/keyboard:
npm install --include=optional  # gets @nut/nut-js
node dist/index.js   # stdio - test
```

**3. Wire into opencode - add to `opencode.json`:**
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
See `opencode.json.example` and `mcp-server/README.md`.

**4. Prompt opencode:**
```
use computer-agent to take a screenshot and click the login button at 600,400
use computer-agent bash_run "ls -la"
use computer-agent browser_navigate https://example.com
```

The MCP exposes the same tools as the Tauri app: `computer_screenshot`, `computer_click` (AI 1280x800), `computer_type`, `computer_key`, `computer_scroll`, `computer_drag`, `bash_run` (blocked `rm -rf /` etc), `browser_snapshot`/`browser_navigate`/`browser_tabs` via `http://127.0.0.1:9222`. For full a11y `see_page` use the app Background Mode or `ego-browser`.

## Shortcuts

- `⌃⇧C` - push-to-talk → computer use mode
- `⌃⇧B` - push-to-talk → background mode
- `⌘⇧H` - help mode (screenshot + quick prompt)
- `⌘⇧S` - stop agent

## Stack

- **Frontend**: React, TypeScript, Tailwind, Zustand, Framer Motion
- **Backend**: Rust, Tauri 2, Tokio
- **Models**: Haiku, Sonnet, Opus (selectable in UI)

## Contributing

PRs welcome. Hit me up on Twitter @ishanxnagpal.

## License

[Apache License 2.0](LICENSE)
