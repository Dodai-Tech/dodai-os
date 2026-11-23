# hibari-board-s3-amoled-206

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/Dodai-Tech/hibari-os/blob/main/LICENSE)

Part of [Hibari OS](https://github.com/Dodai-Tech/hibari-os), a thin operating system for ESP32 boards, written in Rust.

The Waveshare ESP32-S3-Touch-AMOLED-2.06 (ESP32-S3, 32 MB flash, 8 MB PSRAM, 410x502 AMOLED with a CO5300 controller, FT3168 touch). A library (`PROFILE` generated from `board.toml` by `build.rs`, the QSPI bus) and the firmware that uses it. It needs the Xtensa toolchain (`espup install`), so it is not a workspace member.

```sh
just native-install      # build, install in OTA slot 0 and boot it; the native firmware stays
just native-restore      # boot the native firmware again (erases only the otadata partition)
just native-monitor
```

`board.toml` is the one description of this device. Edit it and the firmware, the host preview and `scripts/native.nu` all follow.
