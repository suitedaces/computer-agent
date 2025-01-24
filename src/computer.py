import pyautogui
from PIL import Image
import io
import base64
import time

class ComputerControl:
    def __init__(self):
        self.screen_width, self.screen_height = pyautogui.size()
        pyautogui.PAUSE = 0.5  # Add a small delay between actions for stability
        self.last_click_position = None
        
    def perform_action(self, action):
        action_type = action['type']
        
        # Take a screenshot before the action
        before_screenshot = self.take_screenshot()
        
        try:
            if action_type == 'mouse_move':
                x, y = self.map_from_ai_space(action['x'], action['y'])
                pyautogui.moveTo(x, y)
                time.sleep(0.2)  # Wait for move to complete
                
            elif action_type == 'left_click':
                pyautogui.click()
                time.sleep(0.2)  # Wait for click to register
                self.last_click_position = pyautogui.position()
                
            elif action_type == 'right_click':
                pyautogui.rightClick()
                time.sleep(0.2)
                
            elif action_type == 'middle_click':
                pyautogui.middleClick()
                time.sleep(0.2)
                
            elif action_type == 'double_click':
                pyautogui.doubleClick()
                time.sleep(0.2)
                self.last_click_position = pyautogui.position()
                
            elif action_type == 'left_click_drag':
                start_x, start_y = pyautogui.position()
                end_x, end_y = self.map_from_ai_space(action['x'], action['y'])
                pyautogui.dragTo(end_x, end_y, button='left', duration=0.5)
                time.sleep(0.2)
                
            elif action_type == 'type':
                # If we have a last click position, ensure we're still there
                if self.last_click_position:
                    current_pos = pyautogui.position()
                    if current_pos != self.last_click_position:
                        pyautogui.click(self.last_click_position)
                        time.sleep(0.2)
                
                pyautogui.write(action['text'], interval=0.1)
                time.sleep(0.2)
                
            elif action_type == 'key':
                pyautogui.press(action['text'])
                time.sleep(0.2)
                
            elif action_type == 'screenshot':
                return self.take_screenshot()
                
            elif action_type == 'cursor_position':
                x, y = pyautogui.position()
                return self.map_to_ai_space(x, y)
                
            else:
                raise ValueError(f"Unsupported action: {action_type}")
            
            # Take a screenshot after the action
            after_screenshot = self.take_screenshot()
            return after_screenshot
            
        except Exception as e:
            raise Exception(f"Action failed: {action_type} - {str(e)}")
        
    def take_screenshot(self):
        screenshot = pyautogui.screenshot()
        ai_screenshot = self.resize_for_ai(screenshot)
        buffered = io.BytesIO()
        ai_screenshot.save(buffered, format="PNG")
        return base64.b64encode(buffered.getvalue()).decode('utf-8')
        
    def map_from_ai_space(self, x, y):
        ai_width, ai_height = 1280, 800
        return (x * self.screen_width / ai_width, y * self.screen_height / ai_height)
        
    def map_to_ai_space(self, x, y):
        ai_width, ai_height = 1280, 800
        return (x * ai_width / self.screen_width, y * ai_height / self.screen_height)
        
    def resize_for_ai(self, screenshot):
        return screenshot.resize((1280, 800), Image.LANCZOS)

    def cleanup(self):
        """Clean up any resources or running processes"""
        # Add cleanup code here if needed
        pass
