import speech_recognition as sr
import pyttsx3
import keyboard
import threading
import time
from PyQt6.QtCore import QObject, pyqtSignal

class VoiceController(QObject):
    voice_input_signal = pyqtSignal(str)  # Signal to emit when voice input is received
    status_signal = pyqtSignal(str)  # Signal to emit status updates
    
    def __init__(self):
        super().__init__()
        self.recognizer = sr.Recognizer()
        self.engine = pyttsx3.init()
        self.is_listening = False
        self.is_processing = False  # New flag to track if we're processing a command
        self.listening_thread = None
        self.wake_word = "hey nova"  # Wake word to activate voice control
        
        # Configure text-to-speech
        self.engine.setProperty('rate', 150)  # Speed of speech
        voices = self.engine.getProperty('voices')
        self.engine.setProperty('voice', voices[1].id)  # Use female voice
        
    def speak(self, text):
        """Text-to-speech output"""
        self.engine.say(text)
        self.engine.runAndWait()
        
    def listen_for_command(self):
        """Listen for voice input and convert to text"""
        with sr.Microphone() as source:
            self.recognizer.adjust_for_ambient_noise(source)
            try:
                self.status_signal.emit("Listening...")
                audio = self.recognizer.listen(source, timeout=5, phrase_time_limit=10)
                self.status_signal.emit("Processing...")
                
                text = self.recognizer.recognize_google(audio).lower()
                self.status_signal.emit(f"Recognized: {text}")
                return text
            except sr.WaitTimeoutError:
                return None
            except sr.UnknownValueError:
                self.status_signal.emit("Could not understand audio")
                return None
            except sr.RequestError:
                self.status_signal.emit("Could not request results")
                return None
                
    def voice_control_loop(self):
        """Main loop for voice control"""
        while self.is_listening:
            try:
                if not self.is_processing:  # Only listen for new commands if not processing
                    command = self.listen_for_command()
                    if command:
                        if self.wake_word in command:
                            # Wake word detected, listen for the actual command
                            self.speak("Yes, I'm listening")
                            command = self.listen_for_command()
                            if command:
                                # Don't include the wake word in the command
                                clean_command = command.replace(self.wake_word, "").strip()
                                if clean_command:
                                    self.is_processing = True  # Set processing flag
                                    self.voice_input_signal.emit(clean_command)
                                    self.speak("Processing your request")
                                else:
                                    self.speak("I didn't catch that. Please try again.")
                        else:
                            # If we're already listening and get a command without wake word,
                            # process it directly
                            self.is_processing = True  # Set processing flag
                            self.voice_input_signal.emit(command)
                            self.speak("Processing your request")
                time.sleep(0.1)  # Small delay to prevent CPU hogging
            except Exception as e:
                self.status_signal.emit(f"Error: {str(e)}")
                
    def toggle_voice_control(self):
        """Toggle voice control on/off"""
        if not self.is_listening:
            self.is_listening = True
            self.listening_thread = threading.Thread(target=self.voice_control_loop)
            self.listening_thread.daemon = True
            self.listening_thread.start()
            self.status_signal.emit("Voice control activated")
            self.speak("Voice control activated")
        else:
            self.is_listening = False
            if self.listening_thread:
                self.listening_thread.join(timeout=1)
            self.status_signal.emit("Voice control deactivated")
            self.speak("Voice control deactivated")
            
    def finish_processing(self):
        """Call this when command processing is complete"""
        self.is_processing = False
        self.speak("Ready for next command")
