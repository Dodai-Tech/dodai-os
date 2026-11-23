# hibari-device

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/Dodai-Tech/hibari-os/blob/main/LICENSE)

Part of [Hibari OS](https://github.com/Dodai-Tech/hibari-os), a thin operating system for ESP32 boards, written in Rust.

What a device **is**: its name, its screen and what it can do. One description serves the firmware, the host preview and the build scripts, so a device is written down once, in a `board.toml`.

```toml
name = "waveshare-s3-amoled-2.06"
capabilities = ["display", "touch", "button", "wifi", "ble", "battery", "rtc", "imu", "storage"]

[display]
width = 410
height = 502
offset = [22, 0]          # columns and rows before the visible area in the panel's memory

[firmware]                # only for a real board
chip = "esp32s3"
target = "xtensa-esp32s3-none-elf"
flash_mb = 32

[hardware]                # what the chip has; the renderer choice reads it
ram_kb = 512
psram_kb = 8192
fpu = true

[render]                  # optional: pin a backend (integer, skia, vello-cpu, vello-gpu)
backend = "vello-cpu"
fallback = true           # step down if it does not fit
```

* `Capability`, `CapabilitySet`: what a device offers and what an app needs (`const`, one `u16`).
* `DeviceProfile`, `DisplayInfo`, `Shape`, `Firmware`: the device, with `validate()` (a screen and the
  `display` capability go together, a touch panel needs a screen).
* `Backend`, `Hardware`, `Render`, `choose`, `check`, `plan`: which renderer a device draws with. A
  `[hardware]` table (RAM, PSRAM, FPU, GPU) and an optional `[render]` table feed it; the layers are the
  build, the board file and an automatic rule, followed by a memory check and what is built in, and `Display`
  on a `Plan` says why in one line.
* With the `toml` feature (tools and build scripts only): `DeviceProfile::from_toml` rejects unknown
  fields and capabilities with a message, and `to_rust` writes the profile out as a `const`.

`no_std`, no allocator without the `toml` feature.
