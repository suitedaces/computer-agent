# Running in WSL


## Start pyautogui server in windows

1. Install python in windows
2. Install Flask and pyautogui
3. Run pyautogui_server.py in windows


You'll need to change screen resolution to a scale of 1280x800,
or change the resulotion in `computer.py` to match your screen ratio.

## Test the screenshot function

The `screenshot.py` function will take a screenshot of the
screen and save it in the windows "Pictures/Screenshots" folder.

## Test moving the pointer

The `test_move_pointer.py` function will move the pointer to the
center of the screen.

## Run the agent

You will need to get the IP address of your windows machine. Usually it is
`192.168.x.x`. You will also need to get an API key from the Anthropic website.
Note that you should not change `/etc/resolv.conf` in WSL, as it will break the
network connection.


In this directory, run the following command:

```
export PYAUTOGUI_SERVER_ADDRESS=192.168.x.x:5000
export ANTHROPIC_API_KEY=your_api_key
python ../run.py
```


