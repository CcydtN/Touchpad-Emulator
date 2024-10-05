PORT:="2024"
export RUST_LOG:="debug"

# The default option when running `just`
default:
    just --list

# Attaching to the usbip server, created with the last command, and list the device.
# Make sure the usbip service, and some kernel module is enable.
attach:
    sudo usbip --tcp-port {{PORT}} attach -r 0.0.0.0 -b 0-0-0
    @sleep 1
    usbip port

# Detaching the device
detach:
    sudo usbip detach -p 00

# Command for keyboard emulation

# Creating a usbip server, that provide a fake device.
# Use `just attach` to connect to the server
dev_keyboard:
    cargo run --bin keyboard

# Send char to the server
send_key char:
    curl http://0.0.0.0:3000/send?key={{char}}

dev_touchpad:
    cargo run --bin touchpad
