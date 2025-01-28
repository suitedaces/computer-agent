import requests
import os

ADDRESS = os.getenv("PYAUTOGUI_SERVER_ADDRESS", "http://localhost:5000")


class PyAutoGUIClient:
    def __init__(self, base_url=None):
        if base_url is None:
            # check if the protocol is set
            base_url = ADDRESS
            if ADDRESS.startswith("http://") or ADDRESS.startswith("https://"):
                pass
            else:
                # add the protocol http://
                base_url = f"http://{ADDRESS}"
        self.base_url = base_url

    def size(self):
        response = requests.get(f"{self.base_url}/screen/size")
        # convert string to int
        response_json = response.json()
        print(f"Screen size: {response_json}")
        return int(response_json['width']), int(response_json['height'])

    def position(self):
        response = requests.get(f"{self.base_url}/mouse/position")
        print(f"Mouse position: {response.json()}")
        return response.json()

    def moveTo(self, x, y, duration=0):
        response = requests.post(f"{self.base_url}/mouse/move", json={"x": x, "y": y, "duration": duration})
        print(f"Moving mouse to: {x}, {y}")
        return response.json()

    def click(self, x=None, y=None, button='left'):
        payload = {"button": button}
        if x is not None and y is not None:
            payload.update({"x": x, "y": y})
        print(f"Clicking at: {x}, {y}")
        response = requests.post(f"{self.base_url}/mouse/click", json=payload)
        return response.json()

    def write(self, text, interval=0):
        print(f"Writing text: {text}")
        response = requests.post(f"{self.base_url}/keyboard/write", json={"text": text, "interval": interval})
        return response.json()

    def press(self, key):
        print(f"Pressing key: {key}")
        response = requests.post(f"{self.base_url}/keyboard/press", json={"key": key})
        return response.json()

    def screenshot(self):
        response = requests.get(f"{self.base_url}/screen/screenshot")
        return response.json()
