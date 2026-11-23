# hibari-host

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/Dodai-Tech/hibari-os/blob/main/LICENSE)

Part of [Hibari OS](https://github.com/Dodai-Tech/hibari-os), a thin operating system for ESP32 boards, written in Rust.

Runs the launcher on a desktop. It loads the device files (the real board from `boards/s3-amoled-206/board.toml` and the simulated ones in `devices/`), draws with the integer renderer or `vello_cpu`, writes PNGs and, with `--features window`, opens a window with the mouse as touch.

```sh
cargo run -p hibari-host -- --list
cargo run -p hibari-host -- --device waveshare-s3-amoled-2.06 --tap 300,56 --at 60 --png shot.png
cargo run -p hibari-host -- --device watch-round --renderer vello-cpu --png shot.png
cargo run -p hibari-host --features window -- --device p4-7in --window
```

Not published.
