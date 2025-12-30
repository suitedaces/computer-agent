# Taskhomie (Computer Use AI Agent)
<img width="846" height="606" alt="Screenshot 2025-12-29 at 2 06 38 AM" src="https://github.com/user-attachments/assets/b5b7de82-ec58-424f-af68-e9287a6422d6" />

Local AI agent that controls your computer. Give it natural language instructions and watch it take screenshots, move your mouse, click, type, and run terminal commands.

Built with Tauri, React, and Rust.

## Demo

https://github.com/user-attachments/assets/8edd92a7-7d3e-472a-9e48-3b561f0257d6

Here, I used it to autonomously read and reply to tweets, lol. This is purely for demonstration/research, you should not attempt to do the same, lol.

## Modes

**Hands-Off Mode (Computer Use)** - Takes over your entire screen. Controls mouse, keyboard, any app. Use when task spans multiple apps or needs full desktop access. Toggle via "Hands Off" button in titlebar.

**Browser Mode** - Uses Chrome DevTools Protocol (CDP) instead of mouse/keyboard simulation. Reads page accessibility tree instead of screenshots. Faster and more reliable for web-only tasks. Uses isolated Chrome profile (`~/.taskhomie-chrome`) and auto-launches Chrome with debugging if needed.

**Bash** - Terminal commands with safety guards against destructive operations

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

## Shortcuts

- `⌘⇧S` - stop agent
- `⌘⇧H` - help mode (screenshot + quick prompt from mini bar)

## Stack

- **Frontend**: React, TypeScript, Tailwind, Zustand, Framer Motion
- **Backend**: Rust, Tauri 2, Tokio
- **Models**: Haiku, Sonnet, Opus (selectable in UI)

## Contributing

PRs welcome. Hit me up on Twitter @ishanxnagpal.

## License

[Apache License 2.0](LICENSE)
