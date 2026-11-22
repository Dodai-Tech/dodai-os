# Roadmap

Order matters: each phase leaves something running on a real device or in a window. Details
of the design are in [ARCHITECTURE.md](ARCHITECTURE.md). Status: `[x]` done, `[ ]` planned.

## Phase 0: foundation
- [x] Repo, CI, release tooling (GitHub, Gitea, Codeberg), nushell scripts
- [x] `dodai-os-core` (`no_std`): capabilities, touch gestures, `App` contract
- [x] `dodai-os-hal` (`no_std`): display, touch, backlight, clock, board traits
- [x] Architecture document, backup of the S3 board's native firmware

## Phase 1: first device, hosted preview (ESP32-S3 AMOLED 2.06)
- [ ] `dodai-os-host`: hosted backend (window, mouse as touch, OS clock) for development
- [ ] `dodai-os-shell` with Slint: home screen, app grid, status bar, navigation, theme from `dodai-core`
- [ ] ESP-IDF backend for the S3 board: SH8601 display, touch, PMU, RTC, IMU (pins from Waveshare's BSP)
- [ ] Apps: weather (`weathertrack-core`), clock, settings (WiFi, brightness, time, about)
- [ ] Settings: Bluetooth, OTA updates, sleep and wake, battery

## Phase 2: more boards
- [ ] ESP32-P4 7" touch display (`riscv32imafc`, esp_hosted WiFi)
- [ ] ESP32-C6, headless: console shell and background services
- [ ] Board profiles in TOML; the launcher lists only apps a board can run

## Phase 3: native kernel (standalone), RISC-V ESPs first
- [ ] `dodai-os-kernel`: static scheduler, IPC, capability table (host-testable)
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
