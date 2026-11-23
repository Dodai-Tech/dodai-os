# hibari-drivers

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/Dodai-Tech/hibari-os/blob/main/LICENSE)

Part of [Hibari OS](https://github.com/Dodai-Tech/hibari-os), a thin operating system for ESP32 boards, written in Rust.

Chip drivers over `embedded-hal`: the CO5300 AMOLED display (QSPI, takes a `DisplayInfo` for its size and memory offset, starts a memory write after every window) and the FT3168 touch controller. Tested on the host with fake buses. `no_std`.
