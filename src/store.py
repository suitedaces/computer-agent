import logging
from .anthropic import AnthropicClient
from .computer import ComputerControl
from anthropic.types.beta import BetaMessage, BetaToolUseBlock, BetaTextBlock
import json


logging.basicConfig(level=logging.DEBUG, format='%(asctime)s - %(levelname)s - %(message)s')
logger = logging.getLogger(__name__)

class Store:
    def __init__(self):
        self.instructions = ""
        self.fully_auto = True
        self.running = False
        self.error = None
        self.run_history = []
        self.last_tool_use_id = None
        
        try:
            self.anthropic_client = AnthropicClient()
        except ValueError as e:
            self.error = str(e)
            logger.error(f"AnthropicClient initialization error: {self.error}")
        self.computer_control = ComputerControl()
        
    def set_instructions(self, instructions):
        self.instructions = instructions
        logger.info(f"Instructions set: {instructions}")
        
    def run_agent(self, update_callback):
        if self.error:
            update_callback(f"Error: {self.error}")
            logger.error(f"Agent run failed due to initialization error: {self.error}")
            return

        self.running = True
        self.error = None
        self.run_history = [{"role": "user", "content": self.instructions}]
        logger.info("Starting agent run")
        
        while self.running:
            try:
                message = self.anthropic_client.get_next_action(self.run_history)
                self.run_history.append(message)
                logger.debug(f"Received message from Anthropic: {message}")
                
                # Display assistant's message in the chat
                self.display_assistant_message(message, update_callback)
                
                action = self.extract_action(message)
                logger.info(f"Extracted action: {action}")
                
                if action['type'] == 'error':
                    self.error = action['message']
                    update_callback(f"Error: {self.error}")
                    logger.error(f"Action extraction error: {self.error}")
                    self.running = False
                    break
                elif action['type'] == 'finish':
                    update_callback("Task completed successfully.")
                    logger.info("Task completed successfully")
                    self.running = False
                    break
                
                try:
                    # Perform the action and get the screenshot
                    screenshot = self.computer_control.perform_action(action)
                    
                    if screenshot:  # Only add screenshot if one was returned
                        self.run_history.append({
                            "role": "user",
                            "content": [
                                {
                                    "type": "tool_result",
                                    "tool_use_id": self.last_tool_use_id,
                                    "content": [
                                        {"type": "text", "text": "Here is a screenshot after the action was executed"},
                                        {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": screenshot}}
                                    ]
                                }
                            ]
                        })
                        logger.debug("Screenshot added to run history")
                    
                except Exception as action_error:
                    error_msg = f"Action failed: {str(action_error)}"
                    update_callback(f"Error: {error_msg}")
                    logger.error(error_msg)
                    # Don't stop running, let the AI handle the error
                    self.run_history.append({
                        "role": "user",
                        "content": [{"type": "text", "text": error_msg}]
                    })
                
            except Exception as e:
                self.error = str(e)
                update_callback(f"Error: {self.error}")
                logger.exception(f"Unexpected error during agent run: {self.error}")
                self.running = False
                break
        
    def stop_run(self):
        """Stop the current agent run and clean up"""
        self.running = False
        logger.info("Stopping agent run")
        # Add a message to the run history to indicate stopping
        self.run_history.append({
            "role": "user",
            "content": [{"type": "text", "text": "Agent run stopped by user."}]
        })
        
    def extract_action(self, message):
        logger.debug(f"Extracting action from message: {message}")
        if not isinstance(message, BetaMessage):
            logger.error(f"Unexpected message type: {type(message)}")
            return {'type': 'error', 'message': 'Unexpected message type'}
        
        for item in message.content:
            if isinstance(item, BetaToolUseBlock):
                tool_use = item
                logger.debug(f"Found tool use: {tool_use}")
                self.last_tool_use_id = tool_use.id
                if tool_use.name == 'finish_run':
                    return {'type': 'finish'}
                
                if tool_use.name != 'computer':
                    logger.error(f"Unexpected tool: {tool_use.name}")
                    return {'type': 'error', 'message': f"Unexpected tool: {tool_use.name}"}
                
                input_data = tool_use.input
                action_type = input_data.get('action')
                
                if action_type in ['mouse_move', 'left_click_drag']:
                    if 'coordinate' not in input_data or len(input_data['coordinate']) != 2:
                        logger.error(f"Invalid coordinate for mouse action: {input_data}")
                        return {'type': 'error', 'message': 'Invalid coordinate for mouse action'}
                    return {
                        'type': action_type,
                        'x': input_data['coordinate'][0],
                        'y': input_data['coordinate'][1]
                    }
                elif action_type in ['left_click', 'right_click', 'middle_click', 'double_click', 'screenshot', 'cursor_position']:
                    return {'type': action_type}
                elif action_type in ['type', 'key']:
                    if 'text' not in input_data:
                        logger.error(f"Missing text for keyboard action: {input_data}")
                        return {'type': 'error', 'message': 'Missing text for keyboard action'}
                    return {'type': action_type, 'text': input_data['text']}
                else:
                    logger.error(f"Unsupported action: {action_type}")
                    return {'type': 'error', 'message': f"Unsupported action: {action_type}"}
        
        logger.error("No tool use found in message")
        return {'type': 'error', 'message': 'No tool use found in message'}

    def display_assistant_message(self, message, update_callback):
        if isinstance(message, BetaMessage):
            for item in message.content:
                if isinstance(item, BetaTextBlock):
                    # Clean and format the text
                    text = item.text.strip()
                    if text:  # Only send non-empty messages
                        update_callback(f"Assistant: {text}")
                elif isinstance(item, BetaToolUseBlock):
                    # Format tool use in a more readable way
                    tool_name = item.name
                    tool_input = item.input
                    
                    # Convert tool use to a more readable format
                    if tool_name == 'computer':
                        action = {
                            'type': tool_input.get('action'),
                            'x': tool_input.get('coordinate', [0, 0])[0] if 'coordinate' in tool_input else None,
                            'y': tool_input.get('coordinate', [0, 0])[1] if 'coordinate' in tool_input else None,
                            'text': tool_input.get('text')
                        }
                        update_callback(f"Performed action: {json.dumps(action)}")
                    elif tool_name == 'finish_run':
                        update_callback("Assistant: Task completed! ")
                    else:
                        update_callback(f"Assistant action: {tool_name} - {json.dumps(tool_input)}")
