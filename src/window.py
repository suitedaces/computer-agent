from PyQt6.QtWidgets import (QMainWindow, QVBoxLayout, QHBoxLayout, QWidget, QTextEdit, 
                             QPushButton, QLabel, QProgressBar, QSystemTrayIcon, QMenu, QApplication, QDialog, QLineEdit, QMenuBar, QStatusBar, QComboBox)
from PyQt6.QtCore import Qt, QPoint, pyqtSignal, QThread, QUrl, QSettings
from PyQt6.QtGui import QFont, QKeySequence, QShortcut, QAction, QTextCursor, QDesktopServices
from .store import Store
from .anthropic import AnthropicClient  
from .voice_control import VoiceController
from .prompt_manager import create_prompt_manager
import logging
import qtawesome as qta

logger = logging.getLogger(__name__)

class AgentThread(QThread):
    update_signal = pyqtSignal(str)
    finished_signal = pyqtSignal()

    def __init__(self, store):
        super().__init__()
        self.store = store

    def run(self):
        self.store.run_agent(self.update_signal.emit)
        self.finished_signal.emit()

class SystemPromptDialog(QDialog):
    def __init__(self, parent=None, prompt_manager=None, provider_name=None):
        super().__init__(parent)
        self.prompt_manager = prompt_manager
        self.provider_name = provider_name or "AI"
        self.setWindowTitle(f"Edit {self.provider_name.capitalize()} System Prompt")
        self.setFixedSize(800, 600)
        
        layout = QVBoxLayout()
        
        # Description
        desc_label = QLabel(f"Edit the system prompt for {self.provider_name.capitalize()} models. Be careful with changes as they may affect functionality.")
        desc_label.setWordWrap(True)
        desc_label.setStyleSheet("color: #666; margin: 10px 0;")
        layout.addWidget(desc_label)
        
        # Prompt editor
        self.prompt_editor = QTextEdit()
        self.prompt_editor.setPlainText(self.prompt_manager.get_current_prompt())
        self.prompt_editor.setStyleSheet("""
            QTextEdit {
                background-color: #262626;
                border: 1px solid #333333;
                border-radius: 8px;
                color: #ffffff;
                padding: 12px;
                font-family: Inter;
                font-size: 14px;
            }
        """)
        layout.addWidget(self.prompt_editor)
        
        # Buttons
        button_layout = QHBoxLayout()
        
        reset_btn = QPushButton("Reset to Default")
        reset_btn.clicked.connect(self.reset_prompt)
        reset_btn.setStyleSheet("""
            QPushButton {
                background-color: #666666;
                color: white;
                border: none;
                padding: 10px 20px;
                border-radius: 5px;
            }
            QPushButton:hover {
                background-color: #777777;
            }
        """)
        
        save_btn = QPushButton("Save Changes")
        save_btn.clicked.connect(self.save_changes)
        save_btn.setStyleSheet("""
            QPushButton {
                background-color: #4CAF50;
                color: white;
                border: none;
                padding: 10px 20px;
                border-radius: 5px;
            }
            QPushButton:hover {
                background-color: #45a049;
            }
        """)
        
        button_layout.addWidget(reset_btn)
        button_layout.addStretch()
        button_layout.addWidget(save_btn)
        
        layout.addLayout(button_layout)
        self.setLayout(layout)
    
    def reset_prompt(self):
        if self.prompt_manager.reset_to_default():
            self.prompt_editor.setPlainText(self.prompt_manager.get_current_prompt())
    
    def save_changes(self):
        new_prompt = self.prompt_editor.toPlainText()
        if self.prompt_manager.save_prompt(new_prompt):
            self.accept()
        else:
            # Show error message
            pass

class MainWindow(QMainWindow):
    def __init__(self, store, anthropic_client):
        super().__init__()
        self.store = store
        # Keep anthropic_client for backward compatibility 
        self.anthropic_client = anthropic_client
        
        # Initialize theme settings
        self.settings = QSettings('Grunty', 'Preferences')
        self.dark_mode = self.settings.value('dark_mode', True, type=bool)
        
        # Initialize voice control
        self.voice_controller = VoiceController()
        self.voice_controller.voice_input_signal.connect(self.handle_voice_input)
        self.voice_controller.status_signal.connect(self.update_status)
        
        # Status bar for voice feedback
        self.status_bar = QStatusBar()
        self.setStatusBar(self.status_bar)
        self.status_bar.showMessage("Voice control ready")
        
        # Check if API key is missing
        if self.store.error and "API_KEY not found" in self.store.error:
            self.show_api_key_dialog()
        
        self.setWindowTitle("Grunty 👨💻")
        self.setGeometry(100, 100, 400, 600)
        self.setMinimumSize(400, 500)  # Increased minimum size for better usability
        
        # Set rounded corners and border
        self.setWindowFlags(Qt.WindowType.FramelessWindowHint)
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground)
        
        self.setup_ui()
        self.setup_tray()
        self.setup_shortcuts()
        
    def show_api_key_dialog(self):
        dialog = QDialog(self)
        dialog.setWindowTitle("API Key Required")
        dialog.setFixedWidth(400)
        
        layout = QVBoxLayout()
        
        # Icon and title
        title_layout = QHBoxLayout()
        icon_label = QLabel()
        icon_label.setPixmap(qta.icon('fa5s.key', color='#4CAF50').pixmap(32, 32))
        title_layout.addWidget(icon_label)
        title_label = QLabel("Anthropic API Key Required")
        title_label.setStyleSheet("font-size: 16px; font-weight: bold; color: #4CAF50;")
        title_layout.addWidget(title_label)
        layout.addLayout(title_layout)
        
        # Description
        desc_label = QLabel("Please enter your Anthropic API key to continue. You can find this in your Anthropic dashboard.")
        desc_label.setWordWrap(True)
        desc_label.setStyleSheet("color: #666; margin: 10px 0;")
        layout.addWidget(desc_label)
        
        # API Key input
        self.api_key_input = QLineEdit()
        self.api_key_input.setPlaceholderText("sk-ant-...")
        self.api_key_input.setStyleSheet("""
            QLineEdit {
                padding: 10px;
                border: 2px solid #4CAF50;
                border-radius: 5px;
                font-size: 14px;
            }
        """)
        layout.addWidget(self.api_key_input)
        
        # Save button
        save_btn = QPushButton("Save API Key")
        save_btn.setStyleSheet("""
            QPushButton {
                background-color: #4CAF50;
                color: white;
                border: none;
                padding: 10px;
                border-radius: 5px;
                font-size: 14px;
                font-weight: bold;
            }
            QPushButton:hover {
                background-color: #45a049;
            }
        """)
        save_btn.clicked.connect(lambda: self.save_api_key(dialog))
        layout.addWidget(save_btn)
        
        dialog.setLayout(layout)
        dialog.exec()

    def save_api_key(self, dialog):
        api_key = self.api_key_input.text().strip()
        if not api_key:
            return
            
        # Save to .env file
        with open('.env', 'w') as f:
            f.write(f'API_KEY={api_key}')
            
        # Reinitialize the store and anthropic client
        self.store = Store()
        self.anthropic_client = AnthropicClient()
        dialog.accept()

    def setup_ui(self):
        # Main container with padding
        self.container = QWidget()
        self.container.setObjectName("container")
        
        self.setCentralWidget(self.container)
        
        # Main layout
        main_layout = QVBoxLayout(self.container)
        main_layout.setContentsMargins(10, 10, 10, 10)  # Padding inside the window
        
        # Header
        header_layout = QHBoxLayout()
        
        # App title and info
        title_layout = QVBoxLayout()
        title_label = QLabel("Grunty")
        title_label.setObjectName("titleLabel")
        subtitle_label = QLabel("AI Computer Control")
        subtitle_label.setObjectName("subtitleLabel")
        title_layout.addWidget(title_label)
        title_layout.addWidget(subtitle_label)
        
        # Add AI Provider/Model selector
        ai_selector_layout = QHBoxLayout()
        
        # Provider dropdown
        provider_layout = QVBoxLayout()
        provider_label = QLabel("AI Provider:")
        provider_label.setObjectName("selectorLabel")
        self.provider_dropdown = QComboBox()
        self.provider_dropdown.setObjectName("providerDropdown")
        self.provider_dropdown.addItems(self.store.get_available_providers())
        self.provider_dropdown.setCurrentText(self.store.current_provider_name)
        self.provider_dropdown.currentTextChanged.connect(self.change_provider)
        provider_layout.addWidget(provider_label)
        provider_layout.addWidget(self.provider_dropdown)
        
        # Model dropdown
        model_layout = QVBoxLayout()
        model_label = QLabel("Model:")
        model_label.setObjectName("selectorLabel") 
        self.model_dropdown = QComboBox()
        self.model_dropdown.setObjectName("modelDropdown")
        self.update_model_dropdown()
        self.model_dropdown.currentTextChanged.connect(self.change_model)
        model_layout.addWidget(model_label)
        model_layout.addWidget(self.model_dropdown)
        
        ai_selector_layout.addLayout(provider_layout)
        ai_selector_layout.addLayout(model_layout)
        
        # Settings button
        self.settings_btn = QPushButton()
        self.settings_btn.setIcon(qta.icon('fa5s.cog'))
        self.settings_btn.setObjectName("iconButton")
        self.settings_btn.setToolTip("Settings")
        self.settings_btn.clicked.connect(self.show_settings_menu)
        
        # Buttons layout
        buttons_layout = QHBoxLayout()
        buttons_layout.addStretch()
        
        # Voice control button (mic icon)
        self.voice_btn = QPushButton()
        self.voice_btn.setIcon(qta.icon('fa5s.microphone'))
        self.voice_btn.setCheckable(True)
        self.voice_btn.setObjectName("iconButton")
        self.voice_btn.setToolTip("Voice Control (Experimental)")
        self.voice_btn.clicked.connect(self.toggle_voice_control)
        buttons_layout.addWidget(self.voice_btn)
        
        self.theme_btn = QPushButton()
        self.theme_btn.setIcon(qta.icon('fa5s.moon' if self.dark_mode else 'fa5s.sun'))
        self.theme_btn.setObjectName("iconButton")
        self.theme_btn.setToolTip("Toggle Light/Dark Mode")
        self.theme_btn.clicked.connect(self.toggle_theme)
        buttons_layout.addWidget(self.theme_btn)
        
        # Settings button
        buttons_layout.addWidget(self.settings_btn)
        
        # Minimize button
        min_btn = QPushButton()
        min_btn.setIcon(qta.icon('fa5s.window-minimize'))
        min_btn.setObjectName("titlebarButton")
        min_btn.clicked.connect(self.showMinimized)
        buttons_layout.addWidget(min_btn)
        
        # Close button
        close_btn = QPushButton()
        close_btn.setIcon(qta.icon('fa5s.times'))
        close_btn.setObjectName("titlebarButton")
        close_btn.clicked.connect(self.minimize_to_tray)
        buttons_layout.addWidget(close_btn)
        
        header_layout.addLayout(title_layout)
        header_layout.addStretch()
        header_layout.addLayout(buttons_layout)
        
        main_layout.addLayout(header_layout)
        main_layout.addLayout(ai_selector_layout)
        
        # Action log with modern styling
        self.action_log = QTextEdit()
        self.action_log.setReadOnly(True)
        self.action_log.setStyleSheet("""
            QTextEdit {
                background-color: #262626;
                border: none;
                border-radius: 0;
                color: #ffffff;
                padding: 16px;
                font-family: Inter;
                font-size: 13px;
            }
        """)
        main_layout.addWidget(self.action_log, stretch=1)  # Give it flexible space
        
        # Progress bar - Now above input area
        self.progress_bar = QProgressBar()
        self.progress_bar.setRange(0, 0)
        self.progress_bar.setTextVisible(False)
        self.progress_bar.setStyleSheet("""
            QProgressBar {
                border: none;
                background-color: #262626;
                height: 2px;
                margin: 0;
            }
            QProgressBar::chunk {
                background-color: #4CAF50;
            }
        """)
        self.progress_bar.hide()
        main_layout.addWidget(self.progress_bar)

        # Input section container - Fixed height at bottom
        input_section = QWidget()
        input_section.setObjectName("input_section")
        input_section.setStyleSheet("""
            QWidget {
                background-color: #1e1e1e;
                border-top: 1px solid #333333;
            }
        """)
        input_layout = QVBoxLayout()
        input_layout.setContentsMargins(16, 16, 16, 16)
        input_layout.setSpacing(12)
        input_section.setLayout(input_layout)

        # Input area with modern styling
        self.input_area = QTextEdit()
        self.input_area.setPlaceholderText("What can I do for you today?")
        self.input_area.setFixedHeight(100)  # Fixed height for input
        self.input_area.setStyleSheet("""
            QTextEdit {
                background-color: #262626;
                border: 1px solid #333333;
                border-radius: 8px;
                color: #ffffff;
                padding: 12px;
                font-family: Inter;
                font-size: 14px;
                selection-background-color: #4CAF50;
            }
            QTextEdit:focus {
                border: 1px solid #4CAF50;
            }
        """)
        # Connect textChanged signal
        self.input_area.textChanged.connect(self.update_run_button)
        input_layout.addWidget(self.input_area)

        # Control buttons with modern styling
        control_layout = QHBoxLayout()
        
        self.run_button = QPushButton(qta.icon('fa5s.play', color='white'), "Start")
        self.stop_button = QPushButton(qta.icon('fa5s.stop', color='white'), "Stop")
        
        # Connect button signals
        self.run_button.clicked.connect(self.run_agent)
        self.stop_button.clicked.connect(self.stop_agent)
        
        # Initialize button states
        self.run_button.setEnabled(True)
        self.stop_button.setEnabled(False)
        
        for button in (self.run_button, self.stop_button):
            button.setFixedHeight(40)
            if button == self.run_button:
                button.setStyleSheet("""
                    QPushButton {
                        background-color: #4CAF50;
                        color: white;
                        border: none;
                        border-radius: 8px;
                        padding: 0 24px;
                        font-family: Inter;
                        font-size: 14px;
                        font-weight: bold;
                    }
                    QPushButton:hover {
                        background-color: #45a049;
                    }
                    QPushButton:disabled {
                        background-color: #333333;
                        color: #666666;
                    }
                """)
            else:  # Stop button
                button.setStyleSheet("""
                    QPushButton {
                        background-color: #ff4444;
                        color: white;
                        border: none;
                        border-radius: 8px;
                        padding: 0 24px;
                        font-family: Inter;
                        font-size: 14px;
                        font-weight: bold;
                    }
                    QPushButton:hover {
                        background-color: #ff3333;
                    }
                    QPushButton:disabled {
                        background-color: #333333;
                        color: #666666;
                    }
                """)
            control_layout.addWidget(button)
        
        # Add voice control button to control layout
        self.voice_button = QPushButton(qta.icon('fa5s.microphone', color='white'), "Voice")
        self.voice_button.setFixedHeight(40)
        self.voice_button.setStyleSheet("""
            QPushButton {
                background-color: #4CAF50;
                color: white;
                border: none;
                border-radius: 8px;
                padding: 0 24px;
                font-family: Inter;
                font-size: 14px;
                font-weight: bold;
            }
            QPushButton:hover {
                background-color: #45a049;
            }
            QPushButton:checked {
                background-color: #ff4444;
            }
        """)
        self.voice_button.setCheckable(True)
        self.voice_button.clicked.connect(self.toggle_voice_control)
        control_layout.addWidget(self.voice_button)
        
        input_layout.addLayout(control_layout)

        # Add input section to main container
        main_layout.addWidget(input_section)

        # Apply theme after all widgets are set up
        self.apply_theme()
        
    def update_theme_button(self):
        if self.dark_mode:
            self.theme_btn.setIcon(qta.icon('fa5s.sun', color='white'))
            self.theme_btn.setToolTip("Switch to Light Mode")
        else:
            self.theme_btn.setIcon(qta.icon('fa5s.moon', color='black'))
            self.theme_btn.setToolTip("Switch to Dark Mode")

    def toggle_theme(self):
        self.dark_mode = not self.dark_mode
        self.settings.setValue('dark_mode', self.dark_mode)
        self.update_theme_button()
        self.apply_theme()

    def apply_theme(self):
        # Apply styles based on theme
        colors = {
            'bg': '#1a1a1a' if self.dark_mode else '#ffffff',
            'text': '#ffffff' if self.dark_mode else '#000000',
            'button_bg': '#333333' if self.dark_mode else '#f0f0f0',
            'button_text': '#ffffff' if self.dark_mode else '#000000',
            'button_hover': '#4CAF50' if self.dark_mode else '#e0e0e0',
            'border': '#333333' if self.dark_mode else '#e0e0e0'
        }

        # Container style
        container_style = f"""
            QWidget#container {{
                background-color: {colors['bg']};
                border-radius: 12px;
                border: 1px solid {colors['border']};
            }}
        """
        self.container.setStyleSheet(container_style)  # Use instance variable

        # Update title label
        self.findChild(QLabel, "titleLabel").setStyleSheet(f"color: {colors['text']}; padding: 5px;")

        # Update action log
        self.action_log.setStyleSheet(f"""
            QTextEdit {{
                background-color: {colors['bg']};
                border: none;
                border-radius: 0;
                color: {colors['text']};
                padding: 16px;
                font-family: Inter;
                font-size: 13px;
            }}
        """)

        # Update input area
        self.input_area.setStyleSheet(f"""
            QTextEdit {{
                background-color: {colors['bg']};
                border: 1px solid {colors['border']};
                border-radius: 8px;
                color: {colors['text']};
                padding: 12px;
                font-family: Inter;
                font-size: 14px;
                selection-background-color: {colors['button_hover']};
            }}
            QTextEdit:focus {{
                border: 1px solid {colors['button_hover']};
            }}
        """)

        # Update progress bar
        self.progress_bar.setStyleSheet(f"""
            QProgressBar {{
                border: none;
                background-color: {colors['bg']};
                height: 2px;
                margin: 0;
            }}
            QProgressBar::chunk {{
                background-color: {colors['button_hover']};
            }}
        """)

        # Update input section
        input_section_style = f"""
            QWidget {{
                background-color: {colors['button_bg']};
                border-top: 1px solid {colors['border']};
            }}
        """
        self.findChild(QWidget, "input_section").setStyleSheet(input_section_style)

        # Update window controls style
        window_control_style = f"""
            QPushButton {{
                color: {colors['button_text']};
                background-color: transparent;
                border-radius: 8px;
                padding: 4px 12px;
                font-weight: bold;
            }}
            QPushButton:hover {{
                background-color: {colors['button_hover']};
            }}
        """

        # Apply to all window control buttons
        for button in [self.theme_btn, 
                       self.settings_btn,   # Replace with actual button references
                       self.voice_btn]:     # instead of findChild which might fail
            if button:
                button.setStyleSheet(f"""
                    QPushButton {{
                        background-color: {colors['button_bg']};
                        color: {colors['button_text']};
                        border: none;
                        border-radius: 4px;
                        padding: 8px;
                    }}
                    QPushButton:hover {{
                        background-color: {colors['button_hover']};
                    }}
                """)
        
        # Update theme button icon
        if self.dark_mode:
            self.theme_btn.setIcon(qta.icon('fa5s.sun', color=colors['button_text']))
        else:
            self.theme_btn.setIcon(qta.icon('fa5s.moon', color=colors['button_text']))

        # Update tray menu style if needed
        if hasattr(self, 'tray_icon') and self.tray_icon.contextMenu():
            self.tray_icon.contextMenu().setStyleSheet(f"""
                QMenu {{
                    background-color: {colors['bg']};
                    color: {colors['text']};
                    border: 1px solid {colors['border']};
                    border-radius: 6px;
                    padding: 5px;
                }}
                QMenu::item {{
                    padding: 8px 25px 8px 8px;
                    border-radius: 4px;
                }}
                QMenu::item:selected {{
                    background-color: {colors['button_hover']};
                    color: white;
                }}
                QMenu::separator {{
                    height: 1px;
                    background: {colors['border']};
                    margin: 5px 0px;
                }}
            """)
        
    def update_run_button(self):
        self.run_button.setEnabled(bool(self.input_area.toPlainText().strip()))
        
    def setup_tray(self):
        self.tray_icon = QSystemTrayIcon(self)
        # Make the icon larger and more visible
        icon = qta.icon('fa5s.robot', scale_factor=1.5, color='white')
        self.tray_icon.setIcon(icon)
        
        # Create the tray menu
        tray_menu = QMenu()
        
        # Add a title item (non-clickable)
        title_action = tray_menu.addAction("Grunty 👨🏽‍💻")
        title_action.setEnabled(False)
        tray_menu.addSeparator()
        
        # Add "New Task" option with icon
        new_task = tray_menu.addAction(qta.icon('fa5s.plus', color='white'), "New Task")
        new_task.triggered.connect(self.show)
        
        # Add "Show/Hide" toggle with icon
        toggle_action = tray_menu.addAction(qta.icon('fa5s.eye', color='white'), "Show/Hide")
        toggle_action.triggered.connect(self.toggle_window)
        
        tray_menu.addSeparator()
        
        # Add Quit option with icon
        quit_action = tray_menu.addAction(qta.icon('fa5s.power-off', color='white'), "Quit")
        quit_action.triggered.connect(self.quit_application)
        
        # Style the menu for dark mode
        tray_menu.setStyleSheet("""
            QMenu {
                background-color: #333333;
                color: white;
                border: 1px solid #444444;
                border-radius: 6px;
                padding: 5px;
            }
            QMenu::item {
                padding: 8px 25px 8px 8px;
                border-radius: 4px;
            }
            QMenu::item:selected {
                background-color: #4CAF50;
            }
            QMenu::separator {
                height: 1px;
                background: #444444;
                margin: 5px 0px;
            }
        """)
        
        self.tray_icon.setContextMenu(tray_menu)
        self.tray_icon.show()
        
        # Show a notification when the app starts
        self.tray_icon.showMessage(
            "Grunty is running",
            "Click the robot icon in the menu bar to get started!",
            QSystemTrayIcon.MessageIcon.Information,
            3000
        )
        
        # Connect double-click to toggle window
        self.tray_icon.activated.connect(self.tray_icon_activated)

    def tray_icon_activated(self, reason):
        if reason == QSystemTrayIcon.ActivationReason.DoubleClick:
            self.toggle_window()

    def toggle_window(self):
        if self.isVisible():
            self.hide()
        else:
            self.show()
            self.raise_()
            self.activateWindow()

    def minimize_to_tray(self):
        """Minimize the window to the system tray."""
        self.hide()
        # Show a notification
        self.tray_icon.showMessage(
            "Grunty is still running",
            "The application is minimized to the system tray.",
            QSystemTrayIcon.MessageIcon.Information,
            2000
        )
    
    def run_agent(self):
        instructions = self.input_area.toPlainText()
        if not instructions:
            self.update_log("Please enter instructions before running the agent.")
            return
        
        self.store.set_instructions(instructions)
        self.run_button.setEnabled(False)
        self.stop_button.setEnabled(True)
        self.progress_bar.show()
        self.action_log.clear()
        self.input_area.clear()  # Clear the input area after starting the agent
        
        self.agent_thread = AgentThread(self.store)
        self.agent_thread.update_signal.connect(self.update_log)
        self.agent_thread.finished_signal.connect(self.agent_finished)
        self.agent_thread.start()
        
    def stop_agent(self):
        self.store.stop_run()
        self.stop_button.setEnabled(False)
        
    def agent_finished(self):
        self.run_button.setEnabled(True)
        self.stop_button.setEnabled(False)
        self.progress_bar.hide()
        
        # Yellow completion message with sparkle emoji
        completion_message = '''
            <div style="margin: 6px 0;">
                <span style="
                    display: inline-flex;
                    align-items: center;
                    background-color: rgba(45, 45, 45, 0.95);
                    border: 1px solid rgba(255, 255, 255, 0.1);
                    border-radius: 100px;
                    padding: 4px 12px;
                    color: #FFD700;
                    font-family: Inter, -apple-system, system-ui, sans-serif;
                    font-size: 13px;
                    line-height: 1.4;
                    white-space: nowrap;
                ">✨ Agent run completed</span>
            </div>
        '''
        self.action_log.append(completion_message)
        
        # Notify voice controller that processing is complete
        if hasattr(self, 'voice_controller'):
            self.voice_controller.finish_processing()
        
        
    def update_log(self, message):
        if message.startswith("Performed action:"):
            action_text = message.replace("Performed action:", "").strip()
            
            # Pill-shaped button style with green text
            button_style = '''
                <div style="margin: 6px 0;">
                    <span style="
                        display: inline-flex;
                        align-items: center;
                        background-color: rgba(45, 45, 45, 0.95);
                        border: 1px solid rgba(255, 255, 255, 0.1);
                        border-radius: 100px;
                        padding: 4px 12px;
                        color: #4CAF50;
                        font-family: Inter, -apple-system, system-ui, sans-serif;
                        font-size: 13px;
                        line-height: 1.4;
                        white-space: nowrap;
                    ">{}</span>
                </div>
            '''
            
            try:
                import json
                action_data = json.loads(action_text)
                action_type = action_data.get('type', '').lower()
                
                if action_type == "type":
                    text = action_data.get('text', '')
                    msg = f'⌨️ <span style="margin: 0 4px; color: #4CAF50;">Typed</span> <span style="color: #4CAF50">"{text}"</span>'
                    self.action_log.append(button_style.format(msg))
                    
                elif action_type == "key":
                    key = action_data.get('text', '')
                    msg = f'⌨️ <span style="margin: 0 4px; color: #4CAF50;">Pressed</span> <span style="color: #4CAF50">{key}</span>'
                    self.action_log.append(button_style.format(msg))
                    
                elif action_type == "mouse_move":
                    x = action_data.get('x', 0)
                    y = action_data.get('y', 0)
                    msg = f'🖱️ <span style="margin: 0 4px; color: #4CAF50;">Moved to</span> <span style="color: #4CAF50">({x}, {y})</span>'
                    self.action_log.append(button_style.format(msg))
                    
                elif action_type == "screenshot":
                    msg = '📸 <span style="margin: 0 4px; color: #4CAF50;">Captured Screenshot</span>'
                    self.action_log.append(button_style.format(msg))
                    
                elif "click" in action_type:
                    x = action_data.get('x', 0)
                    y = action_data.get('y', 0)
                    click_map = {
                        "left_click": "Left Click",
                        "right_click": "Right Click",
                        "middle_click": "Middle Click",
                        "double_click": "Double Click"
                    }
                    click_type = click_map.get(action_type, "Click")
                    msg = f'👆 <span style="margin: 0 4px; color: #4CAF50;">{click_type}</span> <span style="color: #4CAF50">({x}, {y})</span>'
                    self.action_log.append(button_style.format(msg))
                    
            except json.JSONDecodeError:
                self.action_log.append(button_style.format(action_text))

        # Clean assistant message style without green background
        elif message.startswith("Assistant:"):
            message_style = '''
                <div style="
                    border-left: 2px solid #666;
                    padding: 8px 16px;
                    margin: 8px 0;
                    font-family: Inter, -apple-system, system-ui, sans-serif;
                    font-size: 13px;
                    line-height: 1.5;
                    color: #e0e0e0;
                ">{}</div>
            '''
            clean_message = message.replace("Assistant:", "").strip()
            self.action_log.append(message_style.format(f'💬 {clean_message}'))

        # Subtle assistant action style
        elif message.startswith("Assistant action:"):
            action_style = '''
                <div style="
                    color: #666;
                    font-style: italic;
                    padding: 4px 0;
                    font-size: 12px;
                    font-family: Inter, -apple-system, system-ui, sans-serif;
                    line-height: 1.4;
                ">🤖 {}</div>
            '''
            clean_message = message.replace("Assistant action:", "").strip()
            self.action_log.append(action_style.format(clean_message))

        # Regular message style
        else:
            regular_style = '''
                <div style="
                    padding: 4px 0;
                    color: #e0e0e0;
                    font-family: Inter, -apple-system, system-ui, sans-serif;
                    font-size: 13px;
                    line-height: 1.4;
                ">{}</div>
            '''
            self.action_log.append(regular_style.format(message))

        # Scroll to bottom
        self.action_log.verticalScrollBar().setValue(
            self.action_log.verticalScrollBar().maximum()
        )
        
    def log(self, message: str):
        """Log a message to the log area.
        
        Args:
            message: Message to log
        """
        if hasattr(self, 'action_log'):
            import datetime
            timestamp = datetime.datetime.now().strftime('%H:%M:%S')
            log_message = f"[{timestamp}] {message}"
            
            # Add message to log area
            if self.action_log.toPlainText():
                self.action_log.append(log_message)
            else:
                self.action_log.setPlainText(log_message)
        else:
            # Fallback to console logging if action_log doesn't exist
            logging.info(f"UI Log: {message}")
        
    def handle_voice_input(self, text):
        """Handle voice input by setting it in the input area and running the agent"""
        self.input_area.setText(text)
        if text.strip():  # Only run if there's actual text
            self.run_agent()
        
    def update_status(self, message):
        """Update status bar with voice control status"""
        self.status_bar.showMessage(message)
        
    def update_voice_status(self, status):
        """Update the action log with voice control status"""
        status_style = '''
            <div style="margin: 6px 0;">
                <span style="
                    display: inline-flex;
                    align-items: center;
                    background-color: rgba(45, 45, 45, 0.95);
                    border: 1px solid rgba(255, 255, 255, 0.1);
                    border-radius: 100px;
                    padding: 4px 12px;
                    color: #4CAF50;
                    font-family: Inter, -apple-system, system-ui, sans-serif;
                    font-size: 13px;
                    line-height: 1.4;
                    white-space: nowrap;
                ">🎤 {}</span>
            </div>
        '''
        self.action_log.append(status_style.format(status))
        
    def toggle_voice_control(self):
        """Toggle voice control on/off"""
        if self.voice_button.isChecked():
            self.voice_controller.toggle_voice_control()
        else:
            self.voice_controller.toggle_voice_control()
            
    def setup_shortcuts(self):
        # Essential shortcuts
        close_window = QShortcut(QKeySequence("Ctrl+W"), self)
        close_window.activated.connect(self.close)
        
        # Add Ctrl+C to stop agent
        stop_agent = QShortcut(QKeySequence("Ctrl+C"), self)
        stop_agent.activated.connect(self.stop_agent)
        
        # Add Ctrl+Enter to send message
        send_message = QShortcut(QKeySequence("Ctrl+Return"), self)
        send_message.activated.connect(self.run_agent)
        
        # Add Alt+V shortcut for voice control
        voice_shortcut = QShortcut(QKeySequence("Alt+V"), self)
        voice_shortcut.activated.connect(lambda: self.voice_button.click())
        
        # Allow tab for indentation
        self.input_area.setTabChangesFocus(False)
        
        # Custom text editing handlers
        self.input_area.keyPressEvent = self.handle_input_keypress

    def handle_input_keypress(self, event):
        # Handle tab key for indentation
        if event.key() == Qt.Key.Key_Tab:
            cursor = self.input_area.textCursor()
            cursor.insertText("    ")  # Insert 4 spaces for tab
            return
            
        # Handle Ctrl+Enter to run agent
        if event.key() == Qt.Key.Key_Return and event.modifiers() == Qt.KeyboardModifier.ControlModifier:
            self.run_agent()
            return
            
        # For all other keys, use default handling
        QTextEdit.keyPressEvent(self.input_area, event)
        
    def mousePressEvent(self, event):
        self.oldPos = event.globalPosition().toPoint()

    def mouseMoveEvent(self, event):
        delta = QPoint(event.globalPosition().toPoint() - self.oldPos)
        self.move(self.x() + delta.x(), self.y() + delta.y())
        self.oldPos = event.globalPosition().toPoint()
        
    def closeEvent(self, event):
        """Handle window close event - properly quit the application"""
        self.quit_application()
        event.accept()  # Allow the close
        
    def quit_application(self):
        """Clean up resources and quit the application"""
        # Stop any running agent
        self.store.stop_run()
        
        # Clean up voice control
        if hasattr(self, 'voice_controller'):
            self.voice_controller.cleanup()
        
        # Save settings
        self.settings.sync()
        
        # Hide tray icon before quitting
        if hasattr(self, 'tray_icon'):
            self.tray_icon.hide()
        
        # Actually quit the application
        QApplication.quit()

    def show_prompt_dialog(self):
        # Get the current provider
        provider_name = self.provider_dropdown.currentText().lower()
        
        # Create a prompt manager instance for the current provider
        prompt_manager = self.store.get_prompt_manager(provider_name)
        
        # Show dialog
        dialog = SystemPromptDialog(self, prompt_manager, provider_name)
        dialog.exec()
    
    def show_settings_menu(self):
        """Show the settings menu when the settings button is clicked."""
        # Create a menu
        menu = QMenu(self)
        
        # Add system prompt action
        prompt_action = QAction("Edit System Prompt", self)
        prompt_action.triggered.connect(self.show_prompt_dialog)
        menu.addAction(prompt_action)
        
        # Add a separator
        menu.addSeparator()
        
        # Add about action
        about_action = QAction("About Grunty", self)
        about_action.triggered.connect(lambda: QMessageBox.about(self, "About Grunty", 
                                                             "Grunty v1.0\nAI Computer Control\n\nAllows AI to control your computer."))
        menu.addAction(about_action)
        
        # Show the menu at the button's position
        menu.exec(self.settings_btn.mapToGlobal(QPoint(0, self.settings_btn.height())))
    
    # Update the model dropdown with models from the current provider.
    def update_model_dropdown(self):
        """Update the model dropdown with models from the current provider."""
        current_provider = self.provider_dropdown.currentText()
        self.model_dropdown.clear()
        
        models = self.store.get_available_models(current_provider)
        model_names = [f"{model['name']}" for model in models]
        model_ids = [model['id'] for model in models]
        
        self.model_dropdown.addItems(model_names)
        
        # Store model IDs as user data
        for i, model_id in enumerate(model_ids):
            self.model_dropdown.setItemData(i, model_id)
        
        # Select the current model if it exists
        if self.store.current_model_id:
            for i in range(self.model_dropdown.count()):
                if self.model_dropdown.itemData(i) == self.store.current_model_id:
                    self.model_dropdown.setCurrentIndex(i)
                    break
    
    def change_provider(self, provider_name):
        """Change the AI provider."""
        import logging
        logger = logging.getLogger(__name__)
        
        try:
            logger.info(f"Attempting to change AI provider to {provider_name}")
            
            if provider_name != self.store.current_provider_name:
                logger.info(f"Current provider is {self.store.current_provider_name}, changing to {provider_name}")
                
                if self.store.set_ai_provider(provider_name):
                    logger.info(f"Successfully changed provider to {provider_name}")
                    self.update_model_dropdown()
                    self.log(f"AI provider changed to {provider_name}")
                else:
                    logger.error(f"Failed to change AI provider: {self.store.error}")
                    self.log(f"Failed to change AI provider: {self.store.error}")
                    # Show error message box
                    from PyQt6.QtWidgets import QMessageBox
                    QMessageBox.critical(self, "Provider Change Error", 
                                      f"Failed to change AI provider to {provider_name}.\n\nReason: {self.store.error}")
                    # Revert selection
                    self.provider_dropdown.setCurrentText(self.store.current_provider_name)
            else:
                logger.info(f"Provider {provider_name} already selected, no change needed")
        except Exception as e:
            import traceback
            logger.error(f"Exception when changing provider: {str(e)}\n{traceback.format_exc()}")
            self.log(f"Error changing provider: {str(e)}")
            # Show error message box
            from PyQt6.QtWidgets import QMessageBox
            QMessageBox.critical(self, "Provider Change Error", 
                             f"An error occurred when changing to {provider_name}.\n\nError: {str(e)}")
            # Revert selection
            self.provider_dropdown.setCurrentText(self.store.current_provider_name)
    
    def change_model(self, model_name):
        """Change the model for the current provider."""
        current_index = self.model_dropdown.currentIndex()
        if current_index >= 0:
            model_id = self.model_dropdown.itemData(current_index)
            if self.store.current_model_id != model_id:
                if self.store.set_ai_provider(self.store.current_provider_name, model_id):
                    self.log(f"Model changed to {model_name} ({model_id})")
                else:
                    self.log(f"Failed to change model: {self.store.error}")
                    # Find and select the current model
                    for i in range(self.model_dropdown.count()):
                        if self.model_dropdown.itemData(i) == self.store.current_model_id:
                            self.model_dropdown.setCurrentIndex(i)
                            break