from wsl import pyautogui_client
from PIL import Image
import io
import base64
import time
from wsl import screenshot as sc

pyautogui = pyautogui_client.PyAutoGUIClient()

class ComputerControl:
    def __init__(self):
        self.screen_width, self.screen_height = pyautogui.size()
        pyautogui.PAUSE = 0.5  # Add a small delay between actions for stability
        
    def perform_action(self, action):
        action_type = action['type']
        
        if action_type == 'mouse_move':
            x, y = self.map_from_ai_space(action['x'], action['y'])
            pyautogui.moveTo(x, y)
        elif action_type == 'left_click':
            pyautogui.click()
            time.sleep(0.1)  # Add a small delay after clicking
        elif action_type == 'right_click':
            pyautogui.rightClick()
            time.sleep(0.1)
        elif action_type == 'middle_click':
            pyautogui.middleClick()
            time.sleep(0.1)
        elif action_type == 'double_click':
            pyautogui.doubleClick()
            time.sleep(0.1)
        elif action_type == 'left_click_drag':
            start_x, start_y = pyautogui.position()
            end_x, end_y = self.map_from_ai_space(action['x'], action['y'])
            pyautogui.dragTo(end_x, end_y, button='left', duration=0.5)
        elif action_type == 'type':
            pyautogui.write(action['text'], interval=0.1)  # Add a small delay between keystrokes
        elif action_type == 'key':
            pyautogui.press(action['text'])
            time.sleep(0.1)
        elif action_type == 'screenshot':
            return self.take_screenshot()
        elif action_type == 'cursor_position':
            x, y = pyautogui.position()
            return self.map_to_ai_space(x, y)
        else:
            raise ValueError(f"Unsupported action: {action_type}")
        
    def take_screenshot(self):
        #screenshot = pyautogui.screenshot()
        screenshot = sc.screenshot()
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
