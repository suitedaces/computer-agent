from google import genai
from dotenv import load_dotenv
from google.genai import types
import os
from .prompt_manager import PromptManager

class GeminiClient:
    def __init__(self):
        load_dotenv()
        self.api_key=os.getenv("GOOGLE_API_KEY")
        if not self.api_key:
            raise ValueError("GOOGLE_API_KEY not found in environment variables")
        
        try:
            self.client=genai.Client(api_key=self.api_key)
            self.promp_manager=PromptManager()

        except Exception as e:
            raise ValueError(f"Failed to initialize Gemini client: {str(e)}")
        
        def get_next_action(self,run_history) -> types.GenerateContentResponse:
            