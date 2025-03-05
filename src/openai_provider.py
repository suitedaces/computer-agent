import os
import json
import base64
import logging
from typing import Dict, Any, List, Optional, Union

try:
    import openai
    from openai.types.chat import ChatCompletionMessage
    OPENAI_AVAILABLE = True
except ImportError:
    OPENAI_AVAILABLE = False

from dotenv import load_dotenv
from .prompt_manager import OpenAIPromptManager
from .ai_providers import AIProvider

logger = logging.getLogger(__name__)

class OpenAIProvider(AIProvider):
    """OpenAI provider implementation."""
    
    # Available models with display names and IDs
    AVAILABLE_MODELS = [
        {
            "id": "gpt-4o",
            "name": "GPT-4o",
            "provider": "openai",
            "features": ["computer_use"]
        },
        {
            "id": "gpt-4-turbo",
            "name": "GPT-4 Turbo",
            "provider": "openai",
            "features": []
        },
        {
            "id": "gpt-4",
            "name": "GPT-4",
            "provider": "openai",
            "features": []
        }
    ]
    
    def __init__(self, api_key: Optional[str] = None, model_id: Optional[str] = None):
        """Initialize the OpenAI provider.
        
        Args:
            api_key: OpenAI API key (optional, will use env var if not provided)
            model_id: Model ID to use (optional, will use default if not provided)
        """
        load_dotenv()  # Load environment variables from .env file
        self.api_key = api_key or os.getenv("OPENAI_API_KEY")
        self.model_id = model_id or self.default_model()
        self.client = None
        self.prompt_manager = OpenAIPromptManager()
        self.last_tool_use_id = None
    
    def initialize(self) -> bool:
        """Initialize the OpenAI client.
        
        Returns:
            True if initialization was successful, False otherwise.
        """
        # Add more detailed error logging
        logger.info(f"Initializing OpenAI provider with model: {self.model_id}")
        
        # Check if OpenAI package is available
        if not OPENAI_AVAILABLE:
            error_msg = "OpenAI package not installed. Please install with 'pip install openai'"
            logger.error(error_msg)
            return False
            
        # Check if API key is available
        if not self.api_key:
            error_msg = "OPENAI_API_KEY not found in environment variables"
            logger.error(error_msg)
            return False
        
        # Try to initialize client
        try:
            logger.info("Creating OpenAI client")
            self.client = openai.OpenAI(api_key=self.api_key)
            
            # Try a simple API call to verify the client works
            logger.info("Testing OpenAI client with a models list request")
            models = self.client.models.list()
            logger.info(f"Successfully initialized OpenAI client, models available: {len(models.data)}")
            
            return True
        except Exception as e:
            import traceback
            stack_trace = traceback.format_exc()
            error_msg = f"Failed to initialize OpenAI client: {str(e)}"
            logger.error(f"{error_msg}\n{stack_trace}")
            return False
    
    def get_prompt_for_model(self, model_id: str) -> str:
        """Get the prompt formatted for the specific OpenAI model.
        
        Args:
            model_id: The model ID to get the prompt for.
            
        Returns:
            Formatted prompt string.
        """
        current_prompt = self.prompt_manager.get_current_prompt()
        return self.prompt_manager.format_prompt_for_model(current_prompt, model_id)
    
    def get_next_action(self, run_history: List[Dict[str, Any]]) -> Any:
        """Get the next action from OpenAI.
        
        Args:
            run_history: List of conversation messages.
            
        Returns:
            Response object from OpenAI.
        """
        if not OPENAI_AVAILABLE:
            raise ImportError("OpenAI package not installed. Please install with 'pip install openai'")
            
        if not self.client:
            if not self.initialize():
                raise ValueError("OpenAI client not initialized")
        
        try:
            # Convert history to OpenAI format
            messages = []
            
            # Add system message
            messages.append({
                "role": "system", 
                "content": self.get_prompt_for_model(self.model_id)
            })
            
            # Convert history messages
            for message in run_history:
                if message.get("role") == "user":
                    # Handle user messages with potential images
                    content = message.get("content", [])
                    if isinstance(content, str):
                        messages.append({"role": "user", "content": content})
                    elif isinstance(content, list):
                        # Format multi-part content (text and images)
                        formatted_content = []
                        for item in content:
                            if item.get("type") == "text":
                                formatted_content.append({"type": "text", "text": item.get("text", "")})
                            elif item.get("type") == "image":
                                # Handle base64 images
                                if item.get("source", {}).get("type") == "base64":
                                    formatted_content.append({
                                        "type": "image_url",
                                        "image_url": {
                                            "url": f"data:image/png;base64,{item['source']['data']}",
                                        }
                                    })
                            elif item.get("type") == "tool_result":
                                # Handle tool results
                                tool_content = []
                                for tool_item in item.get("content", []):
                                    if tool_item.get("type") == "text":
                                        tool_content.append({"type": "text", "text": tool_item.get("text", "")})
                                    elif tool_item.get("type") == "image":
                                        if tool_item.get("source", {}).get("type") == "base64":
                                            tool_content.append({
                                                "type": "image_url",
                                                "image_url": {
                                                    "url": f"data:image/png;base64,{tool_item['source']['data']}",
                                                }
                                            })
                                # Add tool message
                                messages.append({
                                    "role": "tool", 
                                    "tool_call_id": item.get("tool_use_id", "tool_1"),
                                    "content": tool_content if isinstance(tool_content, str) else json.dumps(tool_content)
                                })
                                continue
                        
                        if formatted_content:
                            messages.append({"role": "user", "content": formatted_content})
                elif message.get("role") == "assistant":
                    # Handle assistant messages
                    content = message.get("content", [])
                    if isinstance(content, str):
                        messages.append({"role": "assistant", "content": content})
                    elif isinstance(content, list):
                        # Look for tool use
                        tool_calls = []
                        text_content = ""
                        
                        for item in content:
                            if item.get("type") == "text":
                                text_content += item.get("text", "")
                            elif item.get("type") == "tool_use":
                                tool_calls.append({
                                    "id": item.get("id", f"tool_{len(tool_calls)}"),
                                    "type": "function",
                                    "function": {
                                        "name": item.get("name", ""),
                                        "arguments": json.dumps(item.get("input", {}))
                                    }
                                })
                        
                        if tool_calls:
                            messages.append({
                                "role": "assistant",
                                "content": text_content if text_content else None,
                                "tool_calls": tool_calls
                            })
                        elif text_content:
                            messages.append({"role": "assistant", "content": text_content})
            
            # Check if the selected model supports computer use
            model_info = next((m for m in self.AVAILABLE_MODELS if m["id"] == self.model_id), None)
            supports_computer_use = model_info and "computer_use" in model_info.get("features", [])
            
            # Define tools based on model capabilities
            tools = []
            
            # Add computer use tool if supported
            if supports_computer_use:
                tools.append({
                    "type": "function",
                    "function": {
                        "name": "computer",
                        "description": "Control the computer with mouse and keyboard actions",
                        "parameters": {
                            "type": "object",
                            "properties": {
                                "action": {
                                    "type": "string",
                                    "enum": ["mouse_move", "left_click", "right_click", "middle_click", 
                                            "double_click", "left_click_drag", "type", "key", 
                                            "screenshot", "cursor_position"],
                                    "description": "The action to perform"
                                },
                                "coordinate": {
                                    "type": "array",
                                    "items": {"type": "number"},
                                    "description": "The x,y coordinates for mouse actions"
                                },
                                "text": {
                                    "type": "string",
                                    "description": "The text to type or key to press"
                                }
                            },
                            "required": ["action"]
                        }
                    }
                })
            
            # Always add finish_run tool
            tools.append({
                "type": "function",
                "function": {
                    "name": "finish_run",
                    "description": "Call this function when you have achieved the goal of the task.",
                    "parameters": {
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
            })
            
            # Create the completion request
            response = self.client.chat.completions.create(
                model=self.model_id,
                messages=messages,
                tools=tools,
                temperature=0.7,
                max_tokens=1024,
                tool_choice="auto"
            )
            
            return response.choices[0].message
        
        except Exception as e:
            raise Exception(f"OpenAI API Error: {str(e)}")
    
    def generate_response(self, message: str, screenshot_path: Optional[str] = None, **kwargs) -> str:
        """Generate a response from the OpenAI model.
        
        Args:
            message: User message to respond to
            screenshot_path: Path to screenshot to include (optional)
            **kwargs: Additional arguments
            
        Returns:
            Response string from the model
        """
        if not self.client:
            logger.error("OpenAI client not initialized")
            return "Error: OpenAI client not initialized. Please check your API key and connectivity."
            
        try:
            logger.info(f"Generating response using model {self.model_id}")
            
            messages = self._prepare_messages(message, screenshot_path)
            tools = self._prepare_tools()
            
            logger.info(f"Calling OpenAI API with {len(messages)} messages and {len(tools)} tools")
            
            # Call the OpenAI API with the prepared messages and tools
            completion = self.client.chat.completions.create(
                model=self.model_id,
                messages=messages,
                tools=tools,
                tool_choice="auto",
                max_tokens=kwargs.get("max_tokens", 2048),
                temperature=kwargs.get("temperature", 0.7)
            )
            
            # Process the response
            response_message = completion.choices[0].message
            logger.info(f"Received response with {len(response_message.content or '')} chars")
            
            # Check for tool calls
            if hasattr(response_message, "tool_calls") and response_message.tool_calls:
                tool_calls = response_message.tool_calls
                logger.info(f"Response contains {len(tool_calls)} tool call(s)")
                
                for tool_call in tool_calls:
                    if tool_call.function.name == "computer_control":
                        return self._handle_computer_control(tool_call, messages)
                
                # If we get here, no computer control was performed
                return "The AI attempted to use tools but none were for computer control."
            
            # Return the plain text response if there were no tool calls
            return response_message.content or "No response generated."
        
        except Exception as e:
            import traceback
            logger.error(f"Error generating response: {str(e)}\n{traceback.format_exc()}")
            return f"Error generating response: {str(e)}"
            
    def _handle_computer_control(self, tool_call, messages) -> str:
        """Handle a computer control tool call.
        
        Args:
            tool_call: The tool call object from OpenAI
            messages: The current conversation messages
            
        Returns:
            Response string from the model after executing the computer control
        """
        try:
            # Extract the command from the tool call
            function_args = json.loads(tool_call.function.arguments)
            command = function_args.get("command")
            
            if not command:
                return "Error: No command found in computer control request"
                
            logger.info(f"Executing computer control command: {command}")
            
            # Execute the command
            result = self._execute_computer_control(command)
            
            if result.get("error"):
                return f"Error executing command: {result['error']}"
                
            # Add the tool response to messages and generate a new response
            tool_id = tool_call.id
            tool_response = {
                "tool_call_id": tool_id,
                "role": "tool",
                "name": "computer_control",
                "content": json.dumps(result)
            }
            
            # Get a response object to work with
            response = self.client.chat.completions.create(
                model=self.model_id,
                messages=messages,
                tools=self._prepare_tools(),
                tool_choice="auto",
                max_tokens=1024,
            )
            
            # Create a new messages list with the tool response
            new_messages = messages + [
                self.response_message_to_dict(response.choices[0].message),
                tool_response
            ]
            
            # Generate a follow-up response
            followup_completion = self.client.chat.completions.create(
                model=self.model_id,
                messages=new_messages,
                max_tokens=1024,
            )
            
            # Return the follow-up response
            return followup_completion.choices[0].message.content or "No response after computer control."
        
        except Exception as e:
            import traceback
            logger.error(f"Error in computer control: {str(e)}\n{traceback.format_exc()}")
            return f"Error processing computer control: {str(e)}"
    
    def extract_action(self, response: Any) -> Dict[str, Any]:
        """Extract the action from the OpenAI response.
        
        Args:
            response: Response message from OpenAI.
            
        Returns:
            Dict with the parsed action.
        """
        if not response:
            logger.error("Received empty response from OpenAI")
            return {'type': 'error', 'message': 'Empty response from OpenAI'}
        
        # Check for tool calls
        if hasattr(response, 'tool_calls') and response.tool_calls:
            for tool_call in response.tool_calls:
                function_name = tool_call.function.name
                
                if function_name == 'finish_run':
                    return {'type': 'finish'}
                
                if function_name != 'computer':
                    logger.error(f"Unexpected tool: {function_name}")
                    return {'type': 'error', 'message': f"Unexpected tool: {function_name}"}
                
                try:
                    # Parse arguments
                    args = json.loads(tool_call.function.arguments)
                    action_type = args.get('action')
                    
                    if action_type in ['mouse_move', 'left_click_drag']:
                        if 'coordinate' not in args or len(args['coordinate']) != 2:
                            logger.error(f"Invalid coordinate for mouse action: {args}")
                            return {'type': 'error', 'message': 'Invalid coordinate for mouse action'}
                        return {
                            'type': action_type,
                            'x': args['coordinate'][0],
                            'y': args['coordinate'][1]
                        }
                    elif action_type in ['left_click', 'right_click', 'middle_click', 'double_click', 'screenshot', 'cursor_position']:
                        return {'type': action_type}
                    elif action_type in ['type', 'key']:
                        if 'text' not in args:
                            logger.error(f"Missing text for keyboard action: {args}")
                            return {'type': 'error', 'message': 'Missing text for keyboard action'}
                        return {'type': action_type, 'text': args['text']}
                    else:
                        logger.error(f"Unsupported action: {action_type}")
                        return {'type': 'error', 'message': f"Unsupported action: {action_type}"}
                except json.JSONDecodeError:
                    logger.error(f"Failed to parse tool arguments: {tool_call.function.arguments}")
                    return {'type': 'error', 'message': 'Failed to parse tool arguments'}
        
        # If no tool calls, return error
        return {'type': 'error', 'message': 'No tool use found in message'}
    
    def display_assistant_message(self, message: Any, update_callback: callable) -> None:
        """Format and display the assistant's message.
        
        Args:
            message: The message from OpenAI.
            update_callback: Callback function to update the UI with the message.
        """
        # Display content text if present
        if hasattr(message, 'content') and message.content:
            update_callback(f"Assistant: {message.content}")
        
        # Display tool calls
        if hasattr(message, 'tool_calls') and message.tool_calls:
            for tool_call in message.tool_calls:
                function_name = tool_call.function.name
                self.last_tool_use_id = tool_call.id
                
                try:
                    args = json.loads(tool_call.function.arguments)
                    
                    if function_name == 'computer':
                        action = {
                            'type': args.get('action'),
                            'x': args.get('coordinate', [0, 0])[0] if 'coordinate' in args else None,
                            'y': args.get('coordinate', [0, 0])[1] if 'coordinate' in args else None,
                            'text': args.get('text')
                        }
                        update_callback(f"Performed action: {json.dumps(action)}")
                    elif function_name == 'finish_run':
                        update_callback("Assistant: Task completed! ")
                    else:
                        update_callback(f"Assistant action: {function_name} - {tool_call.function.arguments}")
                except json.JSONDecodeError:
                    update_callback(f"Assistant action: {function_name} - (invalid JSON)")
    
    @staticmethod
    def get_available_models() -> List[Dict[str, str]]:
        """Get a list of available OpenAI models.
        
        Returns:
            List of dictionaries with model information.
        """
        return OpenAIProvider.AVAILABLE_MODELS
    
    @staticmethod
    def default_model() -> str:
        """Get the default OpenAI model ID.
        
        Returns:
            Default model ID string.
        """
        # Return the first model that supports computer use
        for model in OpenAIProvider.AVAILABLE_MODELS:
            if "computer_use" in model.get("features", []):
                return model["id"]
        
        # Fallback to the first model if none support computer use
        return OpenAIProvider.AVAILABLE_MODELS[0]["id"] if OpenAIProvider.AVAILABLE_MODELS else "gpt-4o"

    def _prepare_messages(self, message: str, screenshot_path: Optional[str] = None) -> List[Dict[str, Any]]:
        """Prepare the messages for the OpenAI API.
        
        Args:
            message: User message to respond to
            screenshot_path: Path to screenshot to include (optional)
            
        Returns:
            List of messages in OpenAI format
        """
        messages = []
        
        # Add system message
        messages.append({
            "role": "system", 
            "content": self.get_prompt_for_model(self.model_id)
        })
        
        # Add user message
        messages.append({
            "role": "user", 
            "content": message
        })
        
        # Add screenshot if provided
        if screenshot_path:
            with open(screenshot_path, "rb") as image_file:
                encoded_image = base64.b64encode(image_file.read()).decode("utf-8")
                messages.append({
                    "role": "user", 
                    "content": {
                        "type": "image",
                        "image": {
                            "url": f"data:image/png;base64,{encoded_image}",
                        }
                    }
                })
        
        return messages
    
    def _prepare_tools(self) -> List[Dict[str, Any]]:
        """Prepare the tools for the OpenAI API.
        
        Returns:
            List of tools in OpenAI format
        """
        tools = []
        
        # Add computer use tool
        tools.append({
            "type": "function",
            "function": {
                "name": "computer_control",
                "description": "Control the computer with mouse and keyboard actions",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "The command to execute"
                        }
                    },
                    "required": ["command"]
                }
            }
        })
        
        return tools
    
    def _execute_computer_control(self, command: str) -> Dict[str, Any]:
        """Execute a computer control command.
        
        Args:
            command: The command to execute
            
        Returns:
            Result of the command execution
        """
        try:
            logger.info(f"Executing computer control command: {command}")
            
            # Import the computer control module
            from .computer import ComputerControl
            
            # Create a computer control instance
            computer = ComputerControl()
            
            # Execute the command
            result = computer.execute_command(command)
            
            logger.info(f"Computer control execution result: {result}")
            return result
        except Exception as e:
            import traceback
            error_message = f"Error executing computer control: {str(e)}"
            logger.error(f"{error_message}\n{traceback.format_exc()}")
            return {"error": error_message}
    
    def response_message_to_dict(self, message) -> Dict[str, Any]:
        """Convert a response message to a dictionary.
        
        Args:
            message: Response message from OpenAI (can be various types)
            
        Returns:
            Dictionary representation of the message
        """
        try:
            # If it's already a dict, return it
            if isinstance(message, dict):
                return message
                
            # Handle ChatCompletionMessage objects
            if hasattr(message, 'model_dump'):
                # New OpenAI SDK returns objects with model_dump
                return message.model_dump()
                
            # Handle API response object
            result = {
                "role": getattr(message, "role", "assistant"),
                "content": getattr(message, "content", "")
            }
            
            # Add tool calls if present
            if hasattr(message, "tool_calls") and message.tool_calls:
                result["tool_calls"] = []
                for tool_call in message.tool_calls:
                    tc_dict = {
                        "id": tool_call.id,
                        "type": "function",
                        "function": {
                            "name": tool_call.function.name,
                            "arguments": tool_call.function.arguments
                        }
                    }
                    result["tool_calls"].append(tc_dict)
                    
            return result
        except Exception as e:
            logger.error(f"Error converting message to dict: {str(e)}")
            # Fall back to a basic message
            return {"role": "assistant", "content": str(message)}
