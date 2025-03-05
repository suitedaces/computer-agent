import os
import logging
from abc import ABC, abstractmethod
from typing import List, Dict, Any, Optional
from dotenv import load_dotenv

logger = logging.getLogger(__name__)

class AIProvider(ABC):
    """Base abstract class for AI providers that can control the computer."""
    
    def __init__(self, api_key: Optional[str] = None):
        self.api_key = api_key
    
    @abstractmethod
    def initialize(self) -> bool:
        """Initialize the client with API key and any needed setup.
        Returns True if successful, False otherwise."""
        pass
    
    @abstractmethod
    def get_next_action(self, run_history: List[Dict[str, Any]]) -> Any:
        """Get the next action from the AI based on the conversation history.
        
        Args:
            run_history: List of conversation messages.
            
        Returns:
            Response object from the AI provider.
        """
        pass
    
    @abstractmethod
    def extract_action(self, response: Any) -> Dict[str, Any]:
        """Extract the action from the AI response.
        
        Args:
            response: Response object from the AI provider.
            
        Returns:
            Dict with the parsed action.
        """
        pass
    
    @abstractmethod
    def display_assistant_message(self, message: Any, update_callback: callable) -> None:
        """Format and display the assistant's message.
        
        Args:
            message: The message from the assistant.
            update_callback: Callback function to update the UI with the message.
        """
        pass
    
    @abstractmethod
    def get_prompt_for_model(self, model_id: str) -> str:
        """Get the prompt formatted for the specific model.
        
        Args:
            model_id: The model ID to get the prompt for.
            
        Returns:
            Formatted prompt string.
        """
        pass
    
    @staticmethod
    def get_available_models() -> List[Dict[str, str]]:
        """Get a list of available models for this provider.
        
        Returns:
            List of dictionaries with model information.
        """
        return []
    
    @staticmethod
    def default_model() -> str:
        """Get the default model ID for this provider.
        
        Returns:
            Default model ID string.
        """
        return ""

# Manager class to handle multiple AI providers
class AIProviderManager:
    """Manager for different AI provider integrations."""
    
    PROVIDERS = {
        "anthropic": "AnthropicProvider",
        "openai": "OpenAIProvider"
        # Add more providers here as they are implemented
    }
    
    @staticmethod
    def get_provider_names() -> List[str]:
        """Get a list of available provider names.
        
        Returns:
            List of provider name strings.
        """
        return list(AIProviderManager.PROVIDERS.keys())
    
    @staticmethod
    def create_provider(provider_name: str, **kwargs) -> Optional[AIProvider]:
        """Factory method to create an AI provider.
        
        Args:
            provider_name: Name of the provider to create.
            **kwargs: Additional arguments to pass to the provider constructor.
            
        Returns:
            AIProvider instance or None if creation failed.
        """
        logger.info(f"Creating AI provider: {provider_name} with kwargs: {kwargs}")
        
        # Dynamically import providers without circular imports
        if provider_name == "anthropic":
            try:
                from .anthropic_provider import AnthropicProvider
                provider = AnthropicProvider(**kwargs)
                success = provider.initialize()
                if success:
                    logger.info(f"Successfully created and initialized AnthropicProvider")
                    return provider
                else:
                    logger.error(f"Failed to initialize AnthropicProvider")
                    return None
            except ImportError as e:
                logger.error(f"Failed to import AnthropicProvider: {str(e)}")
                return None
            except Exception as e:
                import traceback
                logger.error(f"Error creating AnthropicProvider: {str(e)}\n{traceback.format_exc()}")
                return None
        elif provider_name == "openai":
            try:
                # First check if openai package is installed
                try:
                    import openai
                    logger.info("OpenAI package found")
                except ImportError as e:
                    logger.error(f"OpenAI package not installed: {str(e)}")
                    return None
                    
                # Then try to import our provider
                from .openai_provider import OpenAIProvider
                logger.info("Creating OpenAIProvider instance")
                provider = OpenAIProvider(**kwargs)
                
                # Initialize the provider
                logger.info("Initializing OpenAIProvider")
                success = provider.initialize()
                
                if success:
                    logger.info("Successfully created and initialized OpenAIProvider")
                    return provider
                else:
                    logger.error("Failed to initialize OpenAIProvider")
                    return None
            except ImportError as e:
                logger.error(f"Failed to import OpenAIProvider: {str(e)}")
                return None
            except Exception as e:
                import traceback
                logger.error(f"Error creating OpenAIProvider: {str(e)}\n{traceback.format_exc()}")
                return None
        
        # Add more provider imports here as they are implemented
        
        logger.error(f"Unknown provider name: {provider_name}")
        return None
