# hibari-launcher

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/Dodai-Tech/hibari-os/blob/main/LICENSE)

Part of [Hibari OS](https://github.com/Dodai-Tech/hibari-os), a thin operating system for ESP32 boards, written in Rust.

The launcher, in four modules:

* `app`: the app manifest (`AppInfo`) and `runnable(device, apps)`, which lists the apps a device can run.
* `model`: paging, hit testing and navigation, with no drawing.
* `view`: an interim view, drawn through Fusuma's `Renderer`: status bar, app grid and slide transitions with `fusuma-anim`. Fusuma widgets are meant to replace it.
* `framebuf`: an RGB565 frame buffer over a caller's slice that tracks which rows changed.

`no_std`, no allocator.
