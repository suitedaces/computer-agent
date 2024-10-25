import sys
import logging
from PyQt6.QtWidgets import QApplication
from window import MainWindow
from store import Store
from anthropic_client import AnthropicClient

# Configure logging to write to a file
logging.basicConfig(filename='agent.log', level=logging.DEBUG, 
                    format='%(asctime)s - %(levelname)s - %(message)s')

def main():
    app = QApplication(sys.argv)
    
    store = Store()
    anthropic_client = AnthropicClient()
    
    window = MainWindow(store, anthropic_client)
    window.show()
    
    sys.exit(app.exec())

if __name__ == "__main__":
    main()