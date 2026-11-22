# Roadmap

Order matters: each phase leaves something running on a real device or in a window. Details
of the design are in [ARCHITECTURE.md](ARCHITECTURE.md). Status: `[x]` done, `[ ]` planned.

## Phase 0: foundation
- [x] Repo, CI, release tooling (GitHub, Gitea, Codeberg), nushell scripts
- [x] `dodai-os-core` (`no_std`): capabilities, touch gestures, `App` contract
- [x] `dodai-os-hal` (`no_std`): display, touch, backlight, clock, board traits
- [x] Architecture document, backup of the S3 board's native firmware
- [x] `dodai-os-kernel` (`no_std`, no `unsafe`): static cooperative scheduler, message queues, capability table with delegation, fuzzed
- [x] `dodai-os-shell-model` (`no_std`): launcher paging, hit testing, navigation, shared with Kawara
- [x] `dodai-os-host`: device profiles (S3 AMOLED, round watch, P4 7in, headless, desktop), host clock and framebuffer display

## Phase 1: first device, hosted preview (ESP32-S3 AMOLED 2.06)
- [x] `dodai-os-shell`: launcher grid, status bar and app page drawn with `embedded-graphics` (`no_std`), adapts to screen size
- [x] `dodai-os-host`: `--png` screenshots with `--tap`, and a `window` feature (minifb, mouse as touch)
- [ ] Theme from `dodai-core` tokens instead of the built-in dark colors
- [ ] Status bar content (time, WiFi, battery) and a swipe-down quick settings panel
- [ ] Round-screen layout (clip the grid to the circle on `Shape::Round`)
- [ ] Optional Slint view for rich UIs (Slint is GPL or commercial, so it stays opt-in, never the default)
- [ ] Run apps as kernel tasks: shell and apps talk to services by message over `dodai-os-kernel`
- [x] `dodai-os-drivers` (`no_std`, embedded-hal, host-tested with fake buses): CO5300 AMOLED and FT3168 touch
- [ ] Drivers: AXP2101 power, PCF85063 RTC, QMI8658 IMU
- [x] `dodai-os-s3-amoled-206` on `esp-hal` (`no_std`, no ESP-IDF): QSPI bus with DMA, I2C, pins, framebuffer in PSRAM
- [x] Firmware boots from an OTA slot next to the native firmware (`just native-install`, `just native-restore`)
- [ ] Confirm on the screen: launcher drawn correctly, taps and swipes work, even-aligned windows not needed
- [ ] Apps: weather (`weathertrack-core`), clock, settings (WiFi, brightness, time, about)
- [ ] Settings: Bluetooth, OTA updates, sleep and wake, battery

## Toolkit track: Fusuma (renderer and toolkit)
- [x] Fusuma repo (`~/Projects/fusuma`), with the renderer as separate `fusuma-render*` crates: `Renderer` trait, `fusuma-render-embedded` (integer, `no_std`), `fusuma-render-skia` (anti-aliased), shared conformance suite, fuzzed
- [ ] Publish `fusuma-render` 0.1 to crates.io, then replace the path dependencies on `../fusuma` here (CI cannot build this repo until then)
- [x] `dodai-os-shell` draws through `fusuma-render`'s `Renderer`: the S3 uses the integer backend, the host can use the anti-aliased one (`--renderer skia`)
- [ ] Time-driven transitions in the shell: page slide, app open and close (`dodai-os-core::anim` exists)
- [ ] `fusuma-render-skia`: anti-aliased vector fonts for the `tiny-skia` backend, and glyph atlases for MCUs
- [ ] `fusuma-render-vello`, GPU backend behind a feature, passing the same conformance suite
- [ ] `fusuma-*` crates in the same repo: widget tree, `taffy` layout, event and focus routing, theme from the design tokens
- [ ] Fusuma widgets: label, button, toggle, slider, list with scrolling, dialog; rebuild the launcher and settings on them
- [ ] Text stack in pure Rust (`parley` + `skrifa`, or `cosmic-text` + `swash`): shaping, fonts, wrapping
- [ ] Text input with an input method (`winit` events) and accessibility (`AccessKit`) in the widgets
- [ ] `fusuma-platform-mcu` (Dodai OS boards) and `fusuma-platform-desktop` (`winit`)
- [ ] `fusuma-platform-android` and `fusuma-platform-ios`: native windows, no web wrapper
- [ ] `dodai-ui-fusuma`: the Dodai UI design system for Fusuma, as one more `dodai-ui-<toolkit>` adapter
- [ ] `fusuma-platform-web` (later, optional): WebAssembly drawn into a canvas, no DOM widgets
- [ ] `dodai-ui-fx`: its `Painter` trait overlaps the `Renderer` of `fusuma-render`; draw effects through it

## Phase 2: more boards
- [ ] ESP32-P4 7" touch display (`riscv32imafc`, esp_hosted WiFi)
- [ ] ESP32-C6, headless: console shell and background services
- [ ] Board profiles in TOML; the launcher lists only apps a board can run

## Phase 3: native kernel (standalone), RISC-V ESPs first
- [x] `dodai-os-kernel` stage 1: cooperative scheduler, IPC, capabilities (host-testable)
- [ ] Kernel stage 2: timers, preemptive threads and per-task stacks behind the `Port` trait
- [ ] `dodai-os-port-riscv32` (excluded crate, `unsafe` with safety comments): context switch, traps, PMP
- [ ] QEMU RISC-V port in CI with a hello-world service
- [ ] `esp-hal` + `esp-wifi` backend on the C6 and P4, replacing ESP-IDF
- [ ] Xtensa port (S3) and Cortex-M port (RP2350, nRF52, STM32)
- [ ] Isolation with PMP and MPU where the hardware has it

## Phase 4: desktop, SBC and x86
- [ ] Hosted on Linux, macOS and Windows (x86 and ARM) as a product, not only a preview
- [ ] Standalone on Raspberry Pi and QEMU ARM (`virt`)
- [ ] Standalone on x86_64 (QEMU `q35`): UEFI framebuffer, VirtIO, then real PCs

### Kawara track (the Dodai desktop environment, parallel to phase 4)
- [x] `dodai-os-shell` split into `dodai-os-shell-model` (pure logic) and the view
- [ ] Rename the shared model to a neutral `dodai-shell-model` when Kawara starts to use it
- [ ] Wayland compositor prototype (Smithay), panel and launcher on the shell model
- [ ] Session that starts from a display manager; settings app shared with Dodai OS
- [ ] Own repos: `kawara`; `dodai-os` keeps only the portable crates

### Noren (the app store)
- [ ] Catalogue and install logic, with a Fusuma front end; backends as separate crates (`noren-flatpak`, `noren-pacman`)
- [ ] Research: apps for Dodai OS on microcontrollers (signed OTA images or sandboxed WebAssembly modules); apps are compiled in today
- [ ] Open: publishing and review, signing, who may submit, paid apps

### Dodai Linux (Arch-based)
- [ ] `archiso` image that boots into Kawara, built by a nightly in CI
- [ ] PKGBUILD repo and `pacman` repository for Kawara and the shared apps
- [ ] First-boot setup and installer
- [ ] Own repo: `dodai-linux`

## Phase 5: phones and wearables
- [ ] Hosted on Android and iOS as an app
- [ ] Pick the first phone: the best-supported unlocked device in the owner's collection
      (probe with `adb` and `fastboot`; check the postmarketOS device tier)
- [ ] Linux mode: Dodai as the userspace shell on that phone's kernel
- [ ] Standalone own kernel on that phone: display, touch and storage first, radio later
- [ ] Wearables: open or unlockable watches (Pine Time, Bangle.js and similar)

## Out of scope
- iPhone, Apple Watch and other locked devices
- Writing GPU, modem or Wi-Fi firmware: closed vendor blobs are linked or hosted, not rewritten
