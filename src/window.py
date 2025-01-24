from PyQt6.QtWidgets import (QMainWindow, QVBoxLayout, QHBoxLayout, QWidget, QTextEdit, 
                             QPushButton, QLabel, QProgressBar, QSystemTrayIcon, QMenu, QApplication, QDialog, QLineEdit, QMenuBar)
from PyQt6.QtCore import Qt, QPoint, pyqtSignal, QThread, QUrl, QSettings
from PyQt6.QtGui import QFont, QKeySequence, QShortcut, QAction, QTextCursor, QDesktopServices
from .store import Store
from .anthropic import AnthropicClient  
from .voice_control import VoiceController
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

class MainWindow(QMainWindow):
    def __init__(self, store, anthropic_client):
        super().__init__()
        self.store = store
        self.anthropic_client = anthropic_client
        
        # Initialize theme settings
        self.settings = QSettings('NOVA', 'Preferences')
        self.dark_mode = self.settings.value('dark_mode', True, type=bool)
        
        # Initialize voice control
        self.voice_controller = VoiceController()
        self.voice_controller.voice_input_signal.connect(self.handle_voice_input)
        self.voice_controller.status_signal.connect(self.update_voice_status)
        
        # Check if API key is missing
        if self.store.error and "ANTHROPIC_API_KEY not found" in self.store.error:
            self.show_api_key_dialog()
        
        self.setWindowTitle("NOVA 👨💻")
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
            f.write(f'ANTHROPIC_API_KEY={api_key}')
            
        # Reinitialize the store and anthropic client
        self.store = Store()
        self.anthropic_client = AnthropicClient()
        dialog.accept()

    def setup_ui(self):
        central_widget = QWidget()
        self.setCentralWidget(central_widget)
        
        # Create main layout
        main_layout = QVBoxLayout()
        main_layout.setContentsMargins(15, 15, 15, 15)
        central_widget.setLayout(main_layout)
        
        # Container widget for rounded corners
        self.container = QWidget()  # Make it an instance variable
        self.container.setObjectName("container")
        container_layout = QVBoxLayout()
        container_layout.setSpacing(0)  # Remove spacing between elements
        self.container.setLayout(container_layout)
        
        # Create title bar
        title_bar = QWidget()
        title_bar.setObjectName("titleBar")
        title_bar_layout = QHBoxLayout(title_bar)
        title_bar_layout.setContentsMargins(10, 5, 10, 5)
        
        # Add NOVA title with robot emoji
        title_label = QLabel("NOVA 🤖")
        title_label.setObjectName("titleLabel")
        title_bar_layout.addWidget(title_label)
        
        # Add File Menu
        file_menu = QMenu("File")
        new_task_action = QAction("New Task", self)
        new_task_action.setShortcut("Ctrl+N")
        quit_action = QAction("Quit", self)
        quit_action.setShortcut("Ctrl+Q")
        quit_action.triggered.connect(self.quit_application)
        file_menu.addAction(new_task_action)
        file_menu.addAction(quit_action)
        
        file_button = QPushButton("File")
        file_button.setObjectName("menuButton")
        file_button.clicked.connect(lambda: file_menu.exec(file_button.mapToGlobal(QPoint(0, file_button.height()))))
        title_bar_layout.addWidget(file_button)
        
        # Add spacer to push remaining items to the right
        title_bar_layout.addStretch()
        
        # Theme toggle button
        self.theme_button = QPushButton()
        self.theme_button.setObjectName("titleBarButton")
        self.theme_button.clicked.connect(self.toggle_theme)
        self.update_theme_button()
        title_bar_layout.addWidget(self.theme_button)
        
        # Minimize and close buttons
        minimize_button = QPushButton("−")
        minimize_button.setObjectName("titleBarButton")
        minimize_button.clicked.connect(self.showMinimized)
        title_bar_layout.addWidget(minimize_button)
        
        close_button = QPushButton("×")
        close_button.setObjectName("titleBarButton")
        close_button.clicked.connect(self.close)
        title_bar_layout.addWidget(close_button)
        
        container_layout.addWidget(title_bar)
        
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
        container_layout.addWidget(self.action_log, stretch=1)  # Give it flexible space
        
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
        container_layout.addWidget(self.progress_bar)

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
        control_layout.setSpacing(8)
        
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
        container_layout.addWidget(input_section)

        # Add the container to the main layout
        main_layout.addWidget(self.container)
        
        # Apply theme after all widgets are set up
        self.apply_theme()
        
    def update_theme_button(self):
        if self.dark_mode:
            self.theme_button.setIcon(qta.icon('fa5s.sun', color='white'))
            self.theme_button.setToolTip("Switch to Light Mode")
        else:
            self.theme_button.setIcon(qta.icon('fa5s.moon', color='black'))
            self.theme_button.setToolTip("Switch to Dark Mode")

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
        for button in [self.theme_button, 
                      self.findChild(QPushButton, "menuButton"),
                      self.findChild(QPushButton, "titleBarButton")]:
            if button:
                button.setStyleSheet(window_control_style)

        # Update theme button icon
        if self.dark_mode:
            self.theme_button.setIcon(qta.icon('fa5s.sun', color=colors['button_text']))
        else:
            self.theme_button.setIcon(qta.icon('fa5s.moon', color=colors['button_text']))

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
        title_action = tray_menu.addAction("NOVA 👨🏽‍💻")
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
            "NOVA is running",
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
        
    def handle_voice_input(self, text):
        """Handle voice input by setting it in the input area and running the agent"""
        self.input_area.setText(text)
        if text.strip():  # Only run if there's actual text
            self.run_agent()
        
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
        # Override close event to minimize to tray instead of quitting
        event.ignore()
        self.hide()
        self.tray_icon.showMessage(
            "NOVA 👨🏽‍💻",
            "Application minimized to tray",
            QSystemTrayIcon.MessageIcon.Information,
            2000
        )
        
    def quit_application(self):
        # Actually quit the application
        QApplication.quit()