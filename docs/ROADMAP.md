# Roadmap

Order matters: each phase leaves something running on a real device or in a window. Details
of the design are in [ARCHITECTURE.md](ARCHITECTURE.md). Status: `[x]` done, `[ ]` planned.

## Phase 0: foundation
- [x] Repo, CI, release tooling (GitHub, Gitea, Codeberg), nushell scripts
- [x] `dodai-os-core` (`no_std`): capabilities, touch gestures, `App` contract
- [x] `dodai-os-hal` (`no_std`): display, touch, backlight, clock, board traits
- [x] Architecture document, backup of the S3 board's native firmware
- [x] `dodai-os-kernel` (`no_std`, no `unsafe`): static cooperative scheduler, message queues, capability table with delegation, fuzzed
- [x] `dodai-os-shell-model` (`no_std`): launcher paging, hit testing, navigation, shared with Dodai DE
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
- [ ] `dodai-os-board-s3-amoled-206` on `esp-hal` (`no_std`, no ESP-IDF): QSPI bus with DMA, I2C, pins, framebuffer in PSRAM
- [ ] Firmware that shows the shell on the S3 board and reacts to touch (needs the `esp` Rust toolchain, `espup install`)
- [ ] Apps: weather (`weathertrack-core`), clock, settings (WiFi, brightness, time, about)
- [ ] Settings: Bluetooth, OTA updates, sleep and wake, battery

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

### Dodai DE track (parallel to phase 4)
- [ ] Split `dodai-os-shell` into `dodai-os-shell-model` (pure logic) and the Slint view
- [ ] Wayland compositor prototype (Smithay), panel and launcher on the shell model
- [ ] Session that starts from a display manager; settings app shared with Dodai OS
- [ ] Own repos: `dodai-de`; `dodai-os` keeps only the portable crates

### Dodai Linux (Arch-based)
- [ ] `archiso` image that boots into Dodai DE, built by a nightly in CI
- [ ] PKGBUILD repo and `pacman` repository for the DE and the shared apps
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
