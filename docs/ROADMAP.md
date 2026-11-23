# Roadmap

Order matters: each phase leaves something running on a real device or in a window. Details
of the design are in [ARCHITECTURE.md](ARCHITECTURE.md). Status: `[x]` done, `[ ]` planned.

## Phase 0: foundation
- [x] Repo, CI, release tooling (GitHub, Gitea, Codeberg), nushell scripts
- [x] `hibari-os-core` (`no_std`): capabilities, touch gestures, `App` contract
- [x] `hibari-os-hal` (`no_std`): display, touch, backlight, clock, board traits
- [x] Architecture document, backup of the S3 board's native firmware
- [x] `hibari-os-kernel` (`no_std`, no `unsafe`): static cooperative scheduler, message queues, capability table with delegation, fuzzed
- [x] `hibari-os-shell-model` (`no_std`): launcher paging, hit testing, navigation, shared with Kawara
- [x] `hibari-os-host`: device profiles (S3 AMOLED, round watch, P4 7in, headless, desktop), host clock and framebuffer display

## Phase 1: first device, hosted preview (ESP32-S3 AMOLED 2.06)
- [x] `hibari-os-shell`: launcher grid, status bar and app page drawn with `embedded-graphics` (`no_std`), adapts to screen size
- [x] `hibari-os-host`: `--png` screenshots with `--tap`, and a `window` feature (minifb, mouse as touch)
- [ ] Theme from `dodai-core` tokens instead of the built-in dark colors
- [ ] Status bar content (time, WiFi, battery) and a swipe-down quick settings panel
- [ ] Round-screen layout (clip the grid to the circle on `Shape::Round`)
- [ ] Optional Slint view for rich UIs (Slint is GPL or commercial, so it stays opt-in, never the default)
- [ ] Run apps as kernel tasks: shell and apps talk to services by message over `hibari-os-kernel`
- [x] `hibari-os-drivers` (`no_std`, embedded-hal, host-tested with fake buses): CO5300 AMOLED and FT3168 touch
- [ ] Drivers: AXP2101 power, PCF85063 RTC, QMI8658 IMU
- [x] `hibari-os-s3-amoled-206` on `esp-hal` (`no_std`, no ESP-IDF): QSPI bus with DMA, I2C, pins, framebuffer in PSRAM
- [x] Firmware boots from an OTA slot next to the native firmware (`just native-install`, `just native-restore`)
- [ ] Confirm on the screen: launcher drawn correctly, taps and swipes work, even-aligned windows not needed
- [ ] Apps: weather (`weathertrack-core`), clock, settings (WiFi, brightness, time, about)
- [ ] Settings: Bluetooth, OTA updates, sleep and wake, battery

## Toolkit track: Fusuma (renderer, animation and toolkit)
The full plan, crate structure and decisions are in the Fusuma repository: `docs/ROADMAP.md`.
- [x] Fusuma repo (`~/Projects/fusuma`): `fusuma-render` (the `Renderer` trait), `fusuma-render-embedded` (integer, `no_std`), `fusuma-render-vello-cpu` and `-vello-common` (`vello_cpu`), a shared conformance suite, fuzzed
- [x] `fusuma-anim`: easing, tweens, keyframes, springs and fling decay in fixed point (`no_std`)
- [x] `hibari-os-shell` draws through `fusuma-render`'s `Renderer`: the S3 uses the integer backend, the host can use the anti-aliased one
- [x] Time-driven transitions in the shell (220 ms cubic ease-out)
- [x] Fusuma 0.2.0 published (`fusuma-render`, `-embedded`, `-vello-common`, `-vello-cpu`, `-vello`, `fusuma-anim`); the host's `--renderer` is now `embedded` or `vello-cpu` (`fusuma-render-skia` 0.1.0 stays on crates.io, unmaintained)
- [ ] Host: `--renderer vello-gpu` behind a `gpu` feature, using `fusuma-render-vello`
- [x] `hibari-os-core` re-exports `fusuma-anim`; its own animation module is gone and the shell uses `fusuma-anim`'s `Tween` and `Easing`
- [ ] Spring-driven transitions: a spring needs per-frame state (stepped with the frame time), so `Ui::draw` stops being a pure function of the clock; design that together with the Fusuma animator, and with drag-to-swipe so a finger can pull a page
- [x] `fusuma-render-vello` (GPU backend) exists and passes the conformance suite; effects (shadow, blur, gradients) and shaders come next as optional capabilities, and the shader engine reuses `dodai-shaders`
- [x] Measured `vello_cpu` on the S3 (`crates/hibari-os-s3-bench`): about 4x slower than the integer renderer (261 ms against 61 ms for a full-screen launcher frame, 68 against 14 ms for a 410x120 strip) and 1.4 MB; it works, but the ESP keeps the integer renderer. Open: the P4, and text
- [ ] Fusuma toolkit: widget tree, `taffy` layout, events, text, widgets; rebuild the launcher and settings on them
- [ ] `dodai-fusuma`: Dodai UI for Fusuma, as one more `dodai-<toolkit>` adapter, with a `ShaderView`
- [ ] `fusuma-platform-mcu` (Hibari OS boards), `-desktop`, `-android`, `-ios`, `-tui`, and later `-web`
- [ ] `dodai-fx`: its `Painter` trait overlaps the `Renderer` of `fusuma-render`; draw effects through it
- [ ] Vello on the S3: tearing fix, a `no_std` Vello backend, partial redraws in the shell (see Fusuma's Phase 1b); the demo `hibari-os-s3-vello-demo` repaints only the touched card and felt richer and more responsive. The integer renderer stays for the C6 and as a fallback, and its full-screen frame takes 61 ms before the flush (the frame buffer is in slow PSRAM and a clear alone writes 412 KB), so a slide runs near 10 fps; options are redrawing only the strip that changed, filling spans instead of per-pixel writes, and keeping hot rows in internal RAM

## Phase 2: more boards
- [ ] ESP32-P4 7" touch display (`riscv32imafc`, esp_hosted WiFi)
- [ ] ESP32-C6, headless: console shell and background services
- [ ] Board profiles in TOML; the launcher lists only apps a board can run

## Phase 3: native kernel (standalone), RISC-V ESPs first
- [x] `hibari-os-kernel` stage 1: cooperative scheduler, IPC, capabilities (host-testable)
- [ ] Kernel stage 2: timers, preemptive threads and per-task stacks behind the `Port` trait
- [ ] `hibari-os-port-riscv32` (excluded crate, `unsafe` with safety comments): context switch, traps, PMP
- [ ] QEMU RISC-V port in CI with a hello-world service
- [ ] `esp-hal` + `esp-wifi` backend on the C6 and P4, replacing ESP-IDF
- [ ] Xtensa port (S3) and Cortex-M port (RP2350, nRF52, STM32)
- [ ] Isolation with PMP and MPU where the hardware has it

## Phase 4: desktop, SBC and x86
- [ ] Hosted on Linux, macOS and Windows (x86 and ARM) as a product, not only a preview
- [ ] Standalone on Raspberry Pi and QEMU ARM (`virt`)
- [ ] Standalone on x86_64 (QEMU `q35`): UEFI framebuffer, VirtIO, then real PCs

### Kawara track (the Dodai desktop environment, parallel to phase 4)
- [x] `hibari-os-shell` split into `hibari-os-shell-model` (pure logic) and the view
- [ ] Rename the shared model to a neutral `shell-model` when Kawara starts to use it
- [ ] Wayland compositor prototype (Smithay), panel and launcher on the shell model
- [ ] Session that starts from a display manager; settings app shared with Hibari OS
- [ ] Own repos: `kawara`; `hibari-os` keeps only the portable crates

### Noren (the app store)
- [ ] Catalogue and install logic, with a Fusuma front end; backends as separate crates (`noren-flatpak`, `noren-pacman`)
- [ ] Research: apps for Hibari OS on microcontrollers (signed OTA images or sandboxed WebAssembly modules); apps are compiled in today
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
- [ ] Linux mode: Hibari as the userspace shell on that phone's kernel
- [ ] Standalone own kernel on that phone: display, touch and storage first, radio later
- [ ] Wearables: open or unlockable watches (Pine Time, Bangle.js and similar)

## Out of scope
- iPhone, Apple Watch and other locked devices
- Writing GPU, modem or Wi-Fi firmware: closed vendor blobs are linked or hosted, not rewritten
