from pyautogui_client import PyAutoGUIClient

# Create a client instance
client = PyAutoGUIClient()

# Get screen size from server
size_data = client.get_screen_size()

# Calculate center
center_x = size_data['width'] // 2
center_y = size_data['height'] // 2

# Send request to move mouse to center
response = client.move_mouse(center_x, center_y, duration=1.0)

# Print response
print(response)
