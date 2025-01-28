import anthropic
from anthropic.types.beta import BetaMessage, BetaTextBlock, BetaToolUseBlock
import os
from dotenv import load_dotenv
import logging
from .prompt_manager import PromptManager

class AnthropicClient:
    def __init__(self):
        load_dotenv()  # Load environment variables from .env file
        self.api_key = os.getenv("ANTHROPIC_API_KEY")
        if not self.api_key:
            raise ValueError("ANTHROPIC_API_KEY not found in environment variables")
        
        try:
            self.client = anthropic.Anthropic(api_key=self.api_key)
            self.prompt_manager = PromptManager()
        except Exception as e:
            raise ValueError(f"Failed to initialize Anthropic client: {str(e)}")
        
    def get_next_action(self, run_history) -> BetaMessage:
        try:
            # Convert BetaMessage objects to dictionaries
            cleaned_history = []
            for message in run_history:
                if isinstance(message, BetaMessage):
                    cleaned_history.append({
                        "role": message.role,
                        "content": message.content
                    })
                elif isinstance(message, dict):
                    cleaned_history.append(message)
                else:
                    raise ValueError(f"Unexpected message type: {type(message)}")
            
            response = self.client.beta.messages.create(
                model="claude-3-5-sonnet-20241022",
                max_tokens=1024,
                tools=[
                    {
                        "type": "computer_20241022",
                        "name": "computer",
                        "display_width_px": 1280,
                        "display_height_px": 800,
                        "display_number": 1,
                    },
                    {
                        "name": "finish_run",
                        "description": "Call this function when you have achieved the goal of the task.",
                        "input_schema": {
                            "type": "object",
                            "properties": {
                                "success": {
                                    "type": "boolean",
                                    "description": "Whether the task was successful"
                                },
                                "error": {
                                    "type": "string",
                                    "description": "The error message if the task was not successful"
                                }
                            },
                            "required": ["success"]
                        }
                    }
                ],
                messages=cleaned_history,
                system=self.prompt_manager.get_current_prompt(),
                betas=["computer-use-2024-10-22"],
            )

            # If Claude responds with just text (no tool use), create a finish_run action with the message
            has_tool_use = any(isinstance(content, BetaToolUseBlock) for content in response.content)
            if not has_tool_use:
                text_content = next((content.text for content in response.content if isinstance(content, BetaTextBlock)), "")
                # Create a synthetic tool use block for finish_run
                response.content.append(BetaToolUseBlock(
                    id="synthetic_finish",
                    type="tool_use",
                    name="finish_run",
                    input={
                        "success": False,
                        "error": f"Claude needs more information: {text_content}"
                    }
                ))
                logging.info(f"Added synthetic finish_run for text-only response: {text_content}")

            return response
            
        except anthropic.APIError as e:
            raise Exception(f"API Error: {str(e)}")
        except Exception as e:
            raise Exception(f"Unexpected error: {str(e)}")
