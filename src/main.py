import sys
import logging
from PyQt6.QtWidgets import QApplication
from .window import MainWindow
from .store import Store
# from .anthropic import AnthropicClient
from .gemini_provider import GeminiClient

logging.basicConfig(filename='agent.log', level=logging.DEBUG, 
                    format='%(asctime)s - %(levelname)s - %(message)s')

def main():
    app = QApplication(sys.argv)
    
    app.setQuitOnLastWindowClosed(False)  # Prevent app from quitting when window is closed
    
    store = Store()
    # anthropic_client = AnthropicClient()
    gemini_client = GeminiClient()
    window = MainWindow(store, gemini_client)

    window.show()  # Just show normally, no maximize
    
    sys.exit(app.exec())

if __name__ == "__main__":
    main()
