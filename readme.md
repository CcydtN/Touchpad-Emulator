# Touchpad Emulator
## Goal
This is a repo for me learning about USB HID protocol, especially report descriptor for a touchpad. (There is no such resource from the internet, so I have to do it my own)


## Usage
> If you want to know more about the command that is used, please look at `justfile` for more detail

> Tested on Linux machine only, not sure for others.

To try using it, open two terminal, and prepare a smart phone.
```bash
# First terminal
just dev_touchpad

# Second terminal
just attach
```

Then, open browser on smartphone. (The browser need to support [Touch event](https://developer.mozilla.org/en-US/docs/Web/API/Touch_events))

Go to `http://YOUR_COMPUTER_IP_ADDRESS:3000`

You should see a ugly webpage. The canvas in the middle can be use as a touchpad.

![screenshot](picture/screenshot.png)
