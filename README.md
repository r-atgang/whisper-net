# whisper-net

`whisper-net` captures audio with an ESP32 + INMP441 microphone and streams the data to a host computer over TCP **or** UDP.  
The rust listener program (`listener_tcp` and `listener_udp`) receives and saves or processes the incoming audio.  
The ESP32 is used in station mode and connects to a Wi-Fi access point started on the host machine using a Bash script.

---

## Features

- ESP32 + INMP441 I²S audio capture  
- Choice of TCP or UDP transport  
- Rust listeners (`cargo run`) for both protocols  
- Bash script (`access-point/`) creates a local Wi-Fi AP using `nmcli`  

---

## Requirements

### On the host PC

- Linux system with `nmcli` (NetworkManager CLI tool)
- Rust toolchain (`rustup`, `cargo`)
- `arduino-cli` (or Arduino IDE for flashing sketches)

### On the ESP32

- ESP32 development board
- INMP441 I²S microphone
- One of the two `.ino` files flashed:
  - `whisperer_tcp.ino` for TCP
  - `whisperer_udp.ino` for UDP

---

## Hardware

| Part | Notes |
|------|-------|
| ESP32 development board | Any with sufficient GPIO |
| INMP441 I²S microphone  | Connect to 3.3 V logic |
| Micro-USB cable         | Power and flashing |

Mic wiring (default pins used in sketches):

| INMP441 | ESP32 |
|---------|-------|
| VCC     | 3.3 V |
| GND     | GND   |
| SD      | GPIO32 |
| SCK     | GPIO14 |
| WS      | GPIO15 |

---

## Directory Layout

```
whisper-net/
├── access-point/            # Bash script: start Wi-Fi AP with nmcli
├── whisperer_tcp.ino        # ESP32 sketch: send audio via TCP
├── whisperer_udp.ino        # ESP32 sketch: send audio via UDP
├── listener_tcp/            # Rust project: `cargo run` to receive TCP stream
└── listener_udp/            # Rust project: `cargo run` to receive UDP stream
```

---

## Quick Start

### 1. Create a Wi-Fi Access Point on the host

```bash
cd access-point
./start-ap.sh
```

This script uses `nmcli` to configure and bring up a Wi-Fi access point.  
Make sure your wireless card supports AP mode.

### 2. Flash the ESP32 with a transmitter sketch

| Transport | Flash this file |
|-----------|-----------------|
| TCP       | `whisperer_tcp.ino` |
| UDP       | `whisperer_udp.ino` |

Example using `arduino-cli`:

```bash
arduino-cli compile --fqbn esp32:esp32:esp32 whisperer_tcp
arduino-cli upload -p /dev/ttyUSB0 --fqbn esp32:esp32:esp32 whisperer_tcp
```

### 3. Run the corresponding Rust listener

```bash
cd listener_tcp   # or listener_udp
cargo run
```

Only one listener and one ESP32 sketch should be used at a time.  
The listener prints packet statistics or saves raw audio depending on its implementation.

---

## License

Licensed under the [Apache License, Version 2.0](LICENSE).

---

## Contribution

Contributions are welcome. Feel free to open issues or submit pull requests to improve functionality or documentation.
