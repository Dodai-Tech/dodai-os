# Roadmap

Hibari OS is a thin operating system for ESP32 boards (see [ARCHITECTURE.md](ARCHITECTURE.md)). Order
matters: each phase leaves something running on a real board. Status: `[x]` done, `[ ]` planned.

## Done

- [x] Repo, CI on GitHub, Gitea and Codeberg, Nushell scripts, fuzzing
- [x] `hibari-device`: capabilities, screen and firmware settings, loaded from `board.toml` (`toml` feature) and written out as constants for the firmware
- [x] `hibari-hal`: `Display`, `Touch`, `Backlight`, `Clock`; touch events and a gesture recogniser
- [x] `hibari-drivers`: CO5300 AMOLED (QSPI) and FT3168 touch, tested with fake buses
- [x] `hibari-launcher`: app manifest and `runnable`, paging and hit testing, an interim view through Fusuma (status bar, grid, 220 ms cubic slide transitions), a frame buffer that tracks changed rows
- [x] `hibari-host`: the launcher on a desktop, with device files, PNG screenshots (`--tap`, `--at`, `--renderer embedded|vello-cpu`) and a window
- [x] `boards/s3-amoled-206`: firmware on `esp-hal` (no ESP-IDF), installed in OTA slot 0 next to the native firmware, with `just native-install` and `just native-restore`; the profile comes from `board.toml`
- [x] The CO5300 memory-write fix: partial updates tore without an explicit write command after each window
- [x] Measured `vello_cpu` on the S3 (`tools/bench-s3`) and a touch demo that repaints only the touched card (`tools/vello-demo-s3`)
- [x] Fusuma 0.2 (renderers, animation) published and used here

## Phase 1: make the S3 board a device

- [x] **A layered renderer choice** (now `fusuma-platform`, wrapped by `hibari_device::plan`) (`hibari_device::plan`, tested; `[render]` in `board.toml`, cargo features on the firmware, `--renderer` on the host; boot report). Not yet: the S3 firmware links only the integer renderer, so another choice is reported and the integer one draws
- [ ] **Link Vello into the S3 firmware** (`vello_cpu` behind a `render-vello-cpu` feature) and a runtime setting stored on the device; measured: `vello_cpu` is about 4 times slower than the old integer renderer for a full frame there, so partial redraws are a must
- [ ] **Vello on the S3 in the launcher (decided: Vello always, the integer renderer goes):** the `no_std` Vello backend exists in Fusuma (measured on the S3: 2 to 3 percent over raw `vello_cpu`, `tools/bench-s3/RESULT.md`, local path dependencies until it is released); what is left is dirty-rectangle redraws in the launcher (Fusuma's Phase 1b); the integer renderer repaints the whole screen in 61 ms, so slides run near 10 fps
- [ ] **Verify on the screen:** swipes, page changes and app open and close, now that partial updates are fixed; the frame rate of a slide is unmeasured
- [x] **PCF85063 RTC driver** and a shared `DateTime` + `Rtc` trait in `hibari-hal` (host-tested with a fake chip; not yet run on the board, and the board does not share its I2C bus with it yet)
- [ ] **Drivers:** QMI8658 IMU, AXP2101 power and battery, the two buttons
- [ ] **Status bar with real data:** time, battery, charging, later WiFi
- [ ] **HAL traits** for each of those, and the decision on async (display and touch stay blocking unless a reason appears)
- [ ] **Measure memory:** free internal RAM, free PSRAM, stack high-water mark and image size, printed by the running firmware, then again with the radio on
- [ ] **Storage and settings:** a Hibari partition layout on the 32 MB flash and a key-value store (`sequential-storage`) that survives a reboot

## Phase 2: connectivity

- [ ] **Runtime:** `esp-rtos` and Embassy under the firmware loop (the radio needs them, as I understand it)
- [ ] **WiFi** with `esp-radio` (a beta): scan, connect, credentials in storage, an NTP clock
- [ ] **A `hibari-services` crate** when the first service exists: network, storage and time as async tasks with typed messages
- [ ] **Settings app:** WiFi, brightness, time zone, about
- [ ] **Bluetooth** with `trouble-host`: advertising and pairing first, then something useful such as phone notifications
- [ ] **OTA updates** of Hibari itself, signed
- [ ] **Sleep, wake and battery behaviour**

## Phase 3: more boards

- [ ] **ESP32-P4 7 inch panel:** a `board.toml`, a board crate and drivers; run the Vello benchmark on it (400 MHz, FPU, vector extension)
- [x] **Measured Vello on an ESP32-C6** (`tools/bench-c6`, no screen needed): it runs up to about 240x240 on a 320 KB heap but is 10 to 20 times slower than the integer renderer and leaves 54 to 63 KB, so a C6 with a screen would have to give up the radio or use something lighter
- [x] **Measured `tiny-skia` on the C6 too:** 3 to 4 times faster and lighter than Vello there; all three renderers are kept and a board picks a default by its hardware (superseded: Fusuma now draws with Vello only; a board without PSRAM gets a text UI)
- [ ] **ESP32-C6 as a real board:** a `board.toml` for a specific board (the one tested is an unnamed 8 MB board), a terminal or serial front end for the headless case (a Fusuma platform crate), background services
- [ ] **Host profiles from board files:** every real board's `board.toml` built into the host preview (the S3 is)
- [ ] A JSON schema for `board.toml`, committed and checked

## Phase 4: apps and the toolkit

- [ ] **The launcher on Fusuma widgets,** replacing the interim view; gestures move into Fusuma
- [ ] **The app model,** designed from the first real app: probably a Fusuma tree plus a manifest and a lifecycle
- [ ] **Apps:** weather (`weathertrack-core`), clock, settings
- [ ] **Dodai theming** through `dodai-fusuma` (in the design-system repository), once Fusuma has widgets
- [ ] **Text:** the bitmap fonts stay on the chips; a better pixel font and a larger character set; Parley only on desktop and phone (see Fusuma's roadmap)

## A flash and configuration wizard

The first part exists: `hibari-device` has the hardware facts (`[hardware]`), the per-backend memory cost
(`Backend::memory`), the layered choice (`plan`) and a one-line explanation of it, all with no UI, so a wizard
is a front end over them. Not written: the wizard itself, the costs of features other than the renderer, and
the flashing steps.

A terminal (and later graphical) wizard that flashes Hibari onto a board and lets the user choose what goes
in, within the limits of the hardware. Nothing is written; what it needs first:

- [x] **Hardware facts in `board.toml`:** `[hardware]` with RAM, PSRAM, FPU and GPU, validated, generated into the firmware's profile; partly done (flash size is in `[firmware]`, the partition layout and the radio's needs are not)
- [ ] **A cost for every option:** the renderer's RAM is done (`Backend::memory`, from the benchmarks); flash size per backend, the radio, a service and an app are not measured, and the wizard needs them to add things up and refuse what does not fit
- [x] **The renderer choice with defaults from the board** (`plan`: build, board file, automatic rule, memory check, what is built in)
- [ ] **The other choices:** features (WiFi, Bluetooth, storage), apps, theme
- [ ] **Backup, flash and restore:** back up what is on the board first (as `just backup` does), flash into an OTA slot next to the native firmware, and offer to restore
- [ ] **Build or download:** build the chosen set from source, or fetch a prebuilt image for known combinations
- [ ] A text-mode interface (ratatui) first, a graphical one later on Fusuma

## Maybe, if a need appears

- [ ] A preemptive scheduler and memory-protection isolation (`experiments/kernel` is the start); only on a chip with a real MPU, and only if apps are ever loaded rather than compiled in
- [ ] Apps as sandboxed WebAssembly modules (`wasmi`), which would need a store and signing
- [ ] Other chip families (RP2350, nRF52, STM32) through Embassy
- [ ] An ESP32-S3 benchmark of Vello with text, once Fusuma has a glyph call

## Not part of Hibari OS

The Kawara desktop environment, the Noren app store and the Dodai Linux distribution were ideas for the
family; none has code. A phone or desktop product would be its own repository and would use Fusuma, not
this OS.
