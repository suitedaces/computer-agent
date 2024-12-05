from flask import Flask, request, jsonify
import pyautogui
from pyautogui import FailSafeException

app = Flask(__name__)
app.config['CORS_HEADERS'] = 'Content-Type'

from flask_cors import CORS
CORS(app, resources={r'/*': {'origins': '*'}})

@app.route('/mouse/move', methods=['POST'])
def mouse_move():
    data = request.get_json()
    print(f"Moving mouse to: {data['x']}, {data['y']}")
    try:
        pyautogui.moveTo(data['x'], data['y'], duration=data.get('duration', 0))
    except FailSafeException:
        print("Mouse moved to a corner, fail-safe guard detected.")
    return jsonify({'status': 'success'})

@app.route('/mouse/click', methods=['POST'])
def mouse_click():
    data = request.get_json()
    data['x'] = data.get('x', None)
    data['y'] = data.get('y', None)
    print(f"Clicking at: {data['x']}, {data['y']}")
    pyautogui.click(data.get('x'), data.get('y'), button=data.get('button', 'left'))
    return jsonify({'status': 'success'})

@app.route('/keyboard/write', methods=['POST'])
def keyboard_write():
    data = request.get_json()
    print(f"Writing text: {data['text']}")
    pyautogui.write(data['text'], interval=data.get('interval', 0))
    return jsonify({'status': 'success'})

@app.route('/keyboard/press', methods=['POST'])
def keyboard_press():
    data = request.get_json()
    key = data['key']
    if key.lower() == 'super_l':
        key = 'winleft'
    # If shortcut divided by +
    if '+' in key:
        keys = key.split('+')
        print(f"Pressing keys: {keys}")
        pyautogui.hotkey(*keys)
    else:
        print(f"Pressing key: {key}")
        pyautogui.press(key)
    return jsonify({'status': 'success'})

@app.route('/screen/screenshot', methods=['GET'])
def screenshot():
    screenshot = pyautogui.screenshot()
    screenshot.save('screenshot.png')
    return jsonify({'status': 'success', 'file': 'screenshot.png'})

@app.route('/mouse/position', methods=['GET'])
def mouse_position():
    x, y = pyautogui.position()
    print(f"Mouse position: {x}, {y}")
    return jsonify({'x': x, 'y': y})

@app.route('/screen/size', methods=['GET'])
def screen_size():
    width, height = pyautogui.size()
    print(f"Screen size: {width}, {height}")
    return jsonify({'width': width, 'height': height})

if __name__ == '__main__':
    app.run(debug=True, host='0.0.0.0')
