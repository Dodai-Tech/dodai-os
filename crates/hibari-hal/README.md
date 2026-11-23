# hibari-hal

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/Dodai-Tech/hibari-os/blob/main/LICENSE)

Part of [Hibari OS](https://github.com/Dodai-Tech/hibari-os), a thin operating system for ESP32 boards, written in Rust.

The traits that board crates implement: `Display` (RGB565 regions), `Touch`, `Backlight` and `Clock`. The `input` module holds what a touch panel reports (`TouchEvent`, `Phase`, `Point`) and the `GestureRecognizer` that turns it into taps, long presses and swipes. What a device is lives in `hibari-device`.

More traits (power, RTC, IMU, storage, network, Bluetooth, buttons) come with their drivers. `no_std`, no allocator.
