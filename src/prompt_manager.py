import json
import os
from pathlib import Path

DEFAULT_SYSTEM_PROMPT = """The user will ask you to perform a task and you should use their computer to do so. After each step, take a screenshot and carefully evaluate if you have achieved the right outcome. Explicitly show your thinking: 'I have evaluated step X...' If not correct, try again. Only when you confirm a step was executed correctly should you move on to the next one. Note that you have to click into the browser address bar before typing a URL. You should always call a tool! Always return a tool call. Remember call the finish_run tool when you have achieved the goal of the task. Do not explain you have finished the task, just call the tool. Use keyboard shortcuts to navigate whenever possible. Please remember to take a screenshot after EVERY step to confirm you have achieved the right outcome."""

class PromptManager:
    def __init__(self):
        self.config_dir = Path.home() / ".grunty"
        self.config_file = self.config_dir / "prompts.json"
        self.current_prompt = self.load_prompt()

    def load_prompt(self) -> str:
        """Load the system prompt from the config file or return the default"""
        try:
            if not self.config_dir.exists():
                self.config_dir.mkdir(parents=True)
            
            if not self.config_file.exists():
                self.save_prompt(DEFAULT_SYSTEM_PROMPT)
                return DEFAULT_SYSTEM_PROMPT

            with open(self.config_file, 'r') as f:
                data = json.load(f)
                return data.get('system_prompt', DEFAULT_SYSTEM_PROMPT)
        except Exception as e:
            print(f"Error loading prompt: {e}")
            return DEFAULT_SYSTEM_PROMPT

    def save_prompt(self, prompt: str) -> bool:
        """Save the system prompt to the config file"""
        try:
            with open(self.config_file, 'w') as f:
                json.dump({'system_prompt': prompt}, f, indent=2)
            self.current_prompt = prompt
            return True
        except Exception as e:
            print(f"Error saving prompt: {e}")
            return False

    def reset_to_default(self) -> bool:
        """Reset the system prompt to the default value"""
        return self.save_prompt(DEFAULT_SYSTEM_PROMPT)

    def get_current_prompt(self) -> str:
        """Get the current system prompt"""
        return self.current_prompt
