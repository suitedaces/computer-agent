import os
import json
import logging
from typing import Dict, Any, Optional, List
from dotenv import load_dotenv
from .computer import ComputerControl
from .ai_providers import AIProviderManager
from .ai_providers import AIProvider

logger = logging.getLogger(__name__)

class Store:
    def __init__(self):
        self.instructions = ""
        self.fully_auto = True
        self.running = False
        self.error = None
        self.run_history = []
        
        # Load environment variables
        load_dotenv()
        
        # Initialize AI provider
        self.current_provider_name = os.getenv("DEFAULT_AI_PROVIDER", "anthropic")
        self.current_model_id = None
        self.ai_provider = self._create_provider(self.current_provider_name)
        self.computer_control = ComputerControl()
        
    def _create_provider(self, provider_name: str) -> Optional[AIProvider]:
        """Create an AI provider instance.
        
        Args:
            provider_name: Name of the provider to create.
            
        Returns:
            AIProvider instance or None if creation failed.
        """
        logger.info(f"Creating AI provider in store: {provider_name}")
        
        try:
            # Try to get the API key from environment
            load_dotenv()
            api_key_env_var = f"{provider_name.upper()}_API_KEY"
            api_key = os.getenv(api_key_env_var)
            
            if not api_key:
                error_msg = f"No API key found for {provider_name} provider. "
                error_msg += f"Please set {api_key_env_var} in your .env file."
                logger.error(error_msg)
                self.error = error_msg
                return None
                
            logger.info(f"Found API key for {provider_name}")
            
            # Create provider instance through manager
            provider = AIProviderManager.create_provider(provider_name, api_key=api_key)
            
            if not provider:
                error_msg = f"Failed to create {provider_name} provider. "
                error_msg += "Check that you have the required dependencies installed."
                logger.error(error_msg)
                self.error = error_msg
                return None
                
            logger.info(f"Successfully created {provider_name} provider")
            return provider
            
        except Exception as e:
            import traceback
            self.error = str(e)
            logger.error(f"AI provider initialization error: {self.error}")
            logger.error(traceback.format_exc())
            return None
        
    def set_instructions(self, instructions):
        self.instructions = instructions
        logger.info(f"Instructions set: {instructions}")
    
    def set_ai_provider(self, provider_name: str, model_id: Optional[str] = None) -> bool:
        """Change the AI provider.
        
        Args:
            provider_name: Name of the provider to use.
            model_id: Specific model ID to use (optional).
            
        Returns:
            True if successful, False otherwise.
        """
        try:
            logger.info(f"Setting AI provider to {provider_name} with model {model_id}")
            
            # Only recreate provider if it's different
            if provider_name != self.current_provider_name:
                logger.info(f"Creating new provider instance for {provider_name}")
                self.current_provider_name = provider_name
                self.ai_provider = self._create_provider(provider_name)
                
                if not self.ai_provider:
                    logger.error(f"Failed to create provider: {self.error}")
                    return False
            else:
                logger.info(f"Provider {provider_name} is already active, no need to recreate")
                
            # Set model ID if provided or use current
            if model_id:
                logger.info(f"Setting model to {model_id}")
                self.current_model_id = model_id
                if self.ai_provider:
                    self.ai_provider.model_id = model_id
                    
            return self.ai_provider is not None
        except Exception as e:
            import traceback
            self.error = str(e)
            logger.error(f"Failed to set AI provider: {self.error}")
            logger.error(traceback.format_exc())
            return False
    
    def get_available_providers(self) -> List[str]:
        """Get a list of available AI providers.
        
        Returns:
            List of provider name strings.
        """
        return AIProviderManager.get_provider_names()
    
    def get_available_models(self, provider_name: Optional[str] = None) -> List[Dict[str, Any]]:
        """Get a list of available models for a provider.
        
        Args:
            provider_name: Name of the provider to get models for (uses current if None).
            
        Returns:
            List of model info dictionaries.
        """
        name = provider_name or self.current_provider_name
        provider = AIProviderManager.create_provider(name)
        
        if provider:
            return provider.get_available_models()
        return []
        
    def get_prompt_manager(self):
        """Get the current provider's prompt manager.
        
        Returns:
            PromptManagerBase instance for the current provider.
        """
        if self.ai_provider:
            return self.ai_provider.prompt_manager
        return None
    
    def update_prompt(self, prompt: str) -> bool:
        """Update the system prompt for the current provider.
        
        Args:
            prompt: New system prompt.
            
        Returns:
            True if successful, False otherwise.
        """
        if not self.ai_provider or not hasattr(self.ai_provider, 'prompt_manager'):
            return False
            
        return self.ai_provider.prompt_manager.save_prompt(prompt)
    
    def reset_prompt_to_default(self) -> bool:
        """Reset the system prompt to the default for the current provider.
        
        Returns:
            True if successful, False otherwise.
        """
        if not self.ai_provider or not hasattr(self.ai_provider, 'prompt_manager'):
            return False
            
        return self.ai_provider.prompt_manager.reset_to_default()
    
    def run_agent(self, update_callback):
        if not self.ai_provider:
            update_callback(f"Error: AI provider not initialized")
            logger.error("Agent run failed due to missing AI provider")
            return

        self.running = True
        self.error = None
        self.run_history = [{"role": "user", "content": self.instructions}]
        logger.info("Starting agent run")
        
        while self.running:
            try:
                message = self.ai_provider.get_next_action(self.run_history)
                self.run_history.append(message)
                logger.debug(f"Received message from AI: {message}")
                
                # Display assistant's message in the chat
                self.ai_provider.display_assistant_message(message, update_callback)
                
                action = self.ai_provider.extract_action(message)
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
                                    "tool_use_id": self.ai_provider.last_tool_use_id,
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
        """Stop the current agent run and clean up resources"""
        self.running = False
        if hasattr(self, 'computer_control'):
            self.computer_control.cleanup()
        logger.info("Agent run stopped")
        # Add a message to the run history to indicate stopping
        self.run_history.append({
            "role": "user",
            "content": [{"type": "text", "text": "Agent run stopped by user."}]
        })
        
    def cleanup(self):
        if hasattr(self, 'computer_control'):
            self.computer_control.cleanup()
