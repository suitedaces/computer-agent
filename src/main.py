import sys
import logging
import traceback
from PyQt6.QtWidgets import QApplication, QMessageBox
from .window import MainWindow
from .store import Store
from .anthropic import AnthropicClient

# Set up more detailed logging
logging.basicConfig(
    filename='agent.log', 
    level=logging.DEBUG, 
    format='%(asctime)s - %(levelname)s - [%(filename)s:%(lineno)d] - %(message)s',
    force=True
)

# Add console handler for immediate feedback
console = logging.StreamHandler()
console.setLevel(logging.DEBUG)
formatter = logging.Formatter('%(asctime)s - %(levelname)s - [%(filename)s:%(lineno)d] - %(message)s')
console.setFormatter(formatter)
logging.getLogger('').addHandler(console)

logger = logging.getLogger(__name__)

def main():
    logger.info("Starting Grunty application")
    app = QApplication(sys.argv)
    
    app.setQuitOnLastWindowClosed(False)  # Prevent app from quitting when window is closed
    
    # Check for required dependencies
    try:
        import anthropic
        logger.info("Anthropic package found")
    except ImportError:
        error_msg = "The anthropic package is required. Please install it with: pip install anthropic"
        logger.error(error_msg)
        QMessageBox.critical(None, "Missing Dependency", error_msg)
        return
    
    # Optional dependency for OpenAI
    try:
        import openai
        logger.info("OpenAI package found")
    except ImportError:
        logger.warning("OpenAI package not installed. OpenAI models will not be available.")
        pass
    
    try:
        logger.info("Initializing store")
        store = Store()
        logger.info("Initializing Anthropic client")
        anthropic_client = AnthropicClient()
        
        logger.info("Creating main window")
        window = MainWindow(store, anthropic_client)
        logger.info("Showing main window")
        window.show()  # Just show normally, no maximize
        
        logger.info("Starting application event loop")
        sys.exit(app.exec())
    except Exception as e:
        error_msg = f"Error starting application: {str(e)}"
        stack_trace = traceback.format_exc()
        logger.error(f"{error_msg}\n{stack_trace}")
        QMessageBox.critical(None, "Application Error", 
                           f"{error_msg}\n\nCheck agent.log for details.")
        sys.exit(1)

if __name__ == "__main__":
    main()
