# 👨🏽‍💻 Grunty

Self-hosted desktop app to have AI control your computer, powered by the Claude [computer use](https://www.anthropic.com/news/3-5-models-and-computer-use) capability and OpenAI's GPT models. Allow AI to take over your laptop and do your tasks for you (or at least attempt to, lol). Written in Python, using PyQt.

## Demo
Here, I asked it to use [vim](https://vim.rtorr.com/) to create a game in Python, run it, and play it.

https://github.com/user-attachments/assets/fa9b195e-fae6-4dbc-adb9-dc42519624b1

Video was sped up 8x btw. [Computer use](https://www.anthropic.com/news/3-5-models-and-computer-use) is pretty slow as of today.

## ⚠️ Important Disclaimers

1. **This is experimental software** - It gives an AI control of your mouse and keyboard. Things can and will go wrong.

2. **Tread Lightly** - If it wipes your computer, sends weird emails, or orders 100 pizzas... that's on you. 

AI providers can see your screen through screenshots during actions. Hide sensitive information or private stuff.

## 🎯 Features
- Literally ask AI to do ANYTHING on your computer that you do with a mouse and keyboard. Browse the web, write code, blah blah.
- **Multiple AI providers support**: Switch between Anthropic Claude and OpenAI models
- **Model selection**: Choose from various models for each provider
- **Theme toggling**: Light/Dark mode support
- **System tray integration**: Minimize to tray and run in background
- **Optional voice control**: Experimental voice input and text-to-speech support

# 💻 Platforms
- Anything you can run Python on: MacOS, Windows, Linux, etc.

## 🛠️ Setup

Get an Anthropic API key [here](https://console.anthropic.com/dashboard) and/or an OpenAI API key [here](https://platform.openai.com/api-keys).

```bash
# Python 3.10+ recommended
python -m venv venv
source venv/bin/activate  # or `venv\Scripts\activate` on Windows
pip install -r requirements.txt

# Add API keys to .env
echo "ANTHROPIC_API_KEY=your-key-here" > .env
echo "OPENAI_API_KEY=your-key-here" >> .env
echo "DEFAULT_AI_PROVIDER=anthropic" >> .env  # or "openai"

# Run
python run.py
```

## 🧠 Supported AI Providers and Models

### Anthropic
- Claude 3.5 Sonnet
- Claude 3 Opus
- Claude 3 Sonnet
- Claude 3 Haiku

### OpenAI
- GPT-4o
- GPT-4 Turbo
- GPT-4

## 🔑 Productivity Keybindings
- `Ctrl + Enter`: Execute the current instruction
- `Ctrl + C`: Stop the current agent action
- `Ctrl + W`: Minimize to system tray
- `Ctrl + Q`: Quit application

## 💡 Tips
- Claude really loves Firefox. You might want to install it for better UI detection and accurate mouse clicks.
- Be specific and explicit, help it out a bit
- Always monitor the agent's actions
- Different models have different capabilities for computer control - experiment to find the best one for your tasks

## 🐛 Known Issues

- Sometimes, the AI doesn't take a screenshot to validate that the input is selected, and types stuff in the wrong place. Press CMD+C to end the action when this happens, and quit and restart the agent.
- Not all models support full computer control with the same level of capability
- Voice control is experimental and may not work reliably on all platforms

## 🤝 Contributing

Issues and PRs are most welcome! Made this is in a day so don't really have a roadmap in mind. Hmu on Twitter @ishanxnagpal if you're got interesting ideas you wanna share. 

## 📄 License

[Apache License 2.0](LICENSE)

---
