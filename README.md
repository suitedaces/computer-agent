# Taskhomie (Computer Use AI Agent)
<img width="846" height="606" alt="Screenshot 2025-12-29 at 2 06 38 AM" src="https://github.com/user-attachments/assets/b5b7de82-ec58-424f-af68-e9287a6422d6" />


Local AI agent that controls your computer. Give it natural language instructions and watch Claude take screenshots, move your mouse, click, type, and run terminal commands.

Built with Tauri, React, and Rust.


## Demo

https://github.com/user-attachments/assets/8edd92a7-7d3e-472a-9e48-3b561f0257d6

Here, I used it to autonomously read and reply to tweets, lol. This is purely for demonstration/research, you should not attempt to do the same, lol.

## Disclaimers

1. **Experimental software** - An AI controls your mouse and keyboard. Things can go wrong.
2. **You're responsible** - If it wipes your computer, sends emails, or orders 100 pizzas... that's on you.
3. **Anthropic sees your screen** - Screenshots are sent to the API during actions. Hide sensitive info.

## How It Works

1. You type an instruction ("open firefox and search for cats")
2. Claude takes a screenshot of your screen
3. Claude decides what to do: move mouse, click, type, run bash commands
4. Action is executed, new screenshot is taken
5. Loop continues until task is complete

The agent uses Claude's computer-use capability with two tools:
- **computer** - mouse/keyboard control and screenshots
- **bash** - terminal commands (with safety guards against destructive operations)

## Setup

**Requirements:**
- Node.js & npm
- Rust & Cargo
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

## Tips

- Claude works best with Firefox for UI detection
- Be specific with your instructions
- Always monitor what it's doing
- Start with simple tasks to get a feel for it

## Known Limitations

- AI sees screen at 1280x800 resolution (resized from actual)
- Sometimes types in wrong place if it skips screenshot validation
- Computer use is still slow - be patient

## Stack

- **Frontend**: React, TypeScript, Tailwind, Zustand, Framer Motion
- **Backend**: Rust, Tauri 2, Tokio
- **Computer control**: enigo (mouse/keyboard), xcap (screenshots)
- **API**: Claude claude-sonnet-4-5 with computer-use-2025-01-24 beta

## Contributing

PRs welcome. Hit me up on Twitter @ishanxnagpal.

## License

[Apache License 2.0](LICENSE)
