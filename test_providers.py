"""
Test script for AI providers.

This script tests the provider creation and switching functionality.
"""

import os
import sys
import logging
from dotenv import load_dotenv

# Configure logging
logging.basicConfig(
    level=logging.DEBUG,
    format='%(asctime)s - %(levelname)s - [%(filename)s:%(lineno)d] - %(message)s'
)
logger = logging.getLogger(__name__)

# Load environment variables
load_dotenv()

# Add the project directory to the path
sys.path.append(os.path.dirname(os.path.abspath(__file__)))

# Import the providers
from src.ai_providers import AIProviderManager
from src.openai_provider import OpenAIProvider
from src.anthropic_provider import AnthropicProvider

def test_anthropic_provider():
    """Test Anthropic provider creation and initialization."""
    logger.info("Testing Anthropic provider...")
    
    # Get the API key
    api_key = os.getenv("ANTHROPIC_API_KEY")
    if not api_key:
        logger.error("ANTHROPIC_API_KEY not found in environment variables")
        return False
        
    # Create the provider
    provider = AnthropicProvider(api_key=api_key)
    
    # Initialize the provider
    if provider.initialize():
        logger.info("Anthropic provider initialized successfully")
        logger.info(f"Using model: {provider.model_id}")
        return True
    else:
        logger.error("Failed to initialize Anthropic provider")
        return False
        
def test_openai_provider():
    """Test OpenAI provider creation and initialization."""
    logger.info("Testing OpenAI provider...")
    
    # Get the API key
    api_key = os.getenv("OPENAI_API_KEY")
    if not api_key:
        logger.error("OPENAI_API_KEY not found in environment variables")
        return False
        
    # Create the provider
    provider = OpenAIProvider(api_key=api_key)
    
    # Initialize the provider
    if provider.initialize():
        logger.info("OpenAI provider initialized successfully")
        logger.info(f"Using model: {provider.model_id}")
        return True
    else:
        logger.error("Failed to initialize OpenAI provider")
        return False
        
def test_provider_manager():
    """Test the provider manager."""
    logger.info("Testing provider manager...")
    
    # Get the available providers
    providers = AIProviderManager.get_provider_names()
    logger.info(f"Available providers: {providers}")
    
    # Test creating each provider
    for provider_name in providers:
        logger.info(f"Testing provider: {provider_name}")
        
        # Get the API key
        api_key_env = f"{provider_name.upper()}_API_KEY"
        api_key = os.getenv(api_key_env)
        
        if not api_key:
            logger.error(f"{api_key_env} not found in environment variables")
            continue
            
        # Create the provider
        provider = AIProviderManager.create_provider(provider_name, api_key=api_key)
        
        if provider:
            logger.info(f"Provider {provider_name} created successfully")
            logger.info(f"Using model: {provider.model_id}")
        else:
            logger.error(f"Failed to create provider {provider_name}")
            
def main():
    """Run all provider tests."""
    logger.info("Starting provider tests...")
    
    # Test individual providers
    anthropic_result = test_anthropic_provider()
    openai_result = test_openai_provider()
    
    # Test the provider manager
    test_provider_manager()
    
    # Print summary
    logger.info("Provider test summary:")
    logger.info(f"Anthropic provider: {'SUCCESS' if anthropic_result else 'FAILED'}")
    logger.info(f"OpenAI provider: {'SUCCESS' if openai_result else 'FAILED'}")
    
if __name__ == "__main__":
    main()
