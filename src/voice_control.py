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
        self.is_processing = False  # Flag to track if we're processing a command
        self.listening_thread = None
        self.wake_word = "hey grunty"  # Wake word to activate voice control
        
        # Configure text-to-speech
        self.engine.setProperty('rate', 150)  # Speed of speech
        voices = self.engine.getProperty('voices')
        self.engine.setProperty('voice', voices[1].id)  # Use female voice
        
    def speak(self, text):
        """Text-to-speech output with enhanced status updates"""
        if not text:
            return
            
        self.status_signal.emit("Initializing speech...")
        try:
            # Configure voice settings for this utterance
            self.engine.setProperty('rate', 150)
            self.status_signal.emit("Starting to speak...")
            
            # Break text into sentences for better status updates
            sentences = text.split('.')
            for i, sentence in enumerate(sentences, 1):
                if sentence.strip():
                    self.status_signal.emit(f"Speaking {i}/{len(sentences)}: {sentence.strip()}")
                    self.engine.say(sentence)
                    self.engine.runAndWait()
                    
            self.status_signal.emit("Finished speaking")
        except Exception as e:
            self.status_signal.emit(f"Speech error: {str(e)}")
        finally:
            self.status_signal.emit("Ready")
            
    def listen_for_command(self):
        """Listen for voice input with enhanced status updates"""
        with sr.Microphone() as source:
            try:
                self.status_signal.emit("Adjusting for ambient noise...")
                self.recognizer.adjust_for_ambient_noise(source, duration=0.5)
                
                self.status_signal.emit("Listening for wake word...")
                audio = self.recognizer.listen(source, timeout=5, phrase_time_limit=5)
                
                self.status_signal.emit("Processing audio...")
                text = self.recognizer.recognize_google(audio).lower().strip()
                self.status_signal.emit(f"Heard: {text}")  # Debug what was heard
                
                # More flexible wake word detection
                if any(text.startswith(word) for word in ["hey grunty", "hey gruny", "hi grunty", "hi gruny"]):
                    self.status_signal.emit("Wake word detected! Listening for command...")
                    audio = self.recognizer.listen(source, timeout=5, phrase_time_limit=5)
                    self.status_signal.emit("Processing command...")
                    command = self.recognizer.recognize_google(audio).lower()
                    self.status_signal.emit(f"Command received: {command}")
                    return command
                else:
                    self.status_signal.emit("Wake word not detected, continuing to listen...")
                    return None
                
            except sr.WaitTimeoutError:
                self.status_signal.emit("Listening timed out")
            except sr.UnknownValueError:
                self.status_signal.emit("Could not understand audio")
            except sr.RequestError as e:
                self.status_signal.emit(f"Speech recognition error: {str(e)}")
            except Exception as e:
                self.status_signal.emit(f"Error: {str(e)}")
            finally:
                self.status_signal.emit("Ready")
            
            return None
            
    def voice_control_loop(self):
        """Main loop for voice control"""
        while self.is_listening:
            if not self.is_processing:
                try:
                    self.is_processing = True
                    command = self.listen_for_command()
                    if command:
                        self.voice_input_signal.emit(command)
                finally:
                    self.is_processing = False
            time.sleep(0.1)  # Small delay to prevent CPU hogging
                
    def toggle_voice_control(self):
        """Toggle voice control on/off"""
        if not self.is_listening:
            self.is_listening = True
            self.listening_thread = threading.Thread(target=self.voice_control_loop)
            self.listening_thread.daemon = True
            self.listening_thread.start()
            self.status_signal.emit("Voice control activated - Say 'hey Grunty' to start")
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

    def cleanup(self):
        """Clean up voice control resources"""
        # Stop voice control if it's running
        if self.is_listening:
            self.toggle_voice_control()  # This will stop the listening thread
            
        # Stop any pending speech
        if hasattr(self, 'speak_queue'):
            self.speak_queue.put(None)  # Signal speak thread to stop
            if hasattr(self, 'speak_thread'):
                self.speak_thread.join(timeout=1.0)
