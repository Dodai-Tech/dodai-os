# Hibari OS architecture

Hibari OS is a thin operating system for ESP32 boards, written in Rust on `esp-hal`, with no ESP-IDF.
It is a platform, not a kernel: a hardware layer, drivers, a launcher and an app model, running on the
runtime that Espressif's own radio libraries need. The goal is a small footprint and a good, smooth
interface on a board with a screen. It is part of the Dodai family (see the end).

## What it is not, and why

* **Not a microkernel.** I built a cooperative scheduler (`experiments/kernel`) and nothing uses it,
  because a kernel buys nothing on these chips. The ESP32-S3 has no memory protection and the C6 only a
  basic one, so there is nothing to isolate; apps are compiled into one firmware image. And WiFi and
  Bluetooth sit on Espressif's closed libraries, which (as I understand it) run on `esp-rtos`, so Hibari
  would not own the scheduler on the radio chips anyway.
* **Not for any hardware.** It targets the ESP32 series. Other chip families are possible later through
  Embassy, but nothing here is written for them.
* **Not a toolkit.** Drawing and widgets are [Fusuma](https://github.com/Dodai-Tech/fusuma); the launcher
  view in this repository is an interim stand-in for Fusuma widgets.

## Layout

```
crates/                      the libraries, all no_std
  hibari-device              what a device is
  hibari-hal                 device traits, touch input and gestures
  hibari-drivers             chip drivers
  hibari-launcher            app manifest, paging and navigation, interim view, frame buffer
boards/<board>/              board.toml, the board library and its firmware (esp-hal); not published
tools/                       host preview, benchmark and demo firmware; not published
experiments/kernel           the unused scheduler
```

Dependencies point one way:

```
hibari-device            (no dependencies)
   ↑
hibari-hal               device traits; input events and gestures
   ↑            ↑
hibari-drivers  hibari-launcher ──→ fusuma-render, fusuma-anim
   ↑                  ↑
   └──── boards/*, tools/host
```

The libraries are `no_std` and need no allocator. A board crate wires drivers to pins and runs the loop;
it is the only place that knows the chip.

## A device is a file

Each device has one description, `board.toml`: its name, its screen (size, shape, the panel's memory
offset), its capabilities and, for a real board, how to build and flash it (chip, target, flash size).

* **The firmware** gets a `const PROFILE` generated from it by `build.rs` (through `hibari-device`'s
  `toml` feature), so the capability list and the screen size are written once. The display driver takes
  its size and offset from the same `DisplayInfo`.
* **The host preview** loads the same file for the real board, plus simulated devices in
  `tools/host/devices/` (a round watch, the P4 panel, a headless board, a desktop window).
* **`scripts/native.nu`** reads chip, target and flash size from its `[firmware]` table.

Unknown fields and capabilities are errors with a message, and a profile is validated (a screen and the
`display` capability go together; a touch panel needs a screen). Not done: a committed JSON schema, and
pins in the file (esp-hal wants one typed handle per pin, so they stay in the board's source).

A new board is a folder with a `board.toml`, a board library and a firmware crate.

## Capabilities

A device lists what it offers (`display`, `touch`, `wifi`, `ble`, `battery`, `rtc`, `imu`, `storage`, ...)
and an app lists what it needs. The launcher shows `runnable(device, apps)`, so a headless board lists no
screen apps and a watch without WiFi lists no weather app. It is a compile-time and start-up filter, not a
security boundary.

## Rendering

Everything draws through Fusuma's `Renderer` trait, so the choice of backend is one line per firmware.

Rendering is always Vello: `vello_cpu` on every board, and the GPU backend on hardware that has a usable
GPU (a desktop, not an ESP). There is no integer renderer in the plan.

| Board | Renderer | Note |
|---|---|---|
| ESP32-S3, ESP32-P4 (PSRAM, a screen) | `vello_cpu`, with partial redraws | needs PSRAM: a full 410x502 frame is about 1 MB, a card-sized region 200 to 230 KB |
| Boards without PSRAM (ESP32-C6, ESP32) | none | not supported for rendering, except possibly a very small display (unmeasured) |
| Headless boards | none | a terminal or serial front end is planned in Fusuma |

What I measured on the S3 (240 MHz, 8 MB PSRAM): `vello_cpu` builds `no_std` with `libm` and runs. A
full-screen launcher frame takes 267 ms through Fusuma's trait (the integer renderer took 61 ms), so slides
need partial redraws. A demo that repaints only the touched card (41 ms to render, 11 ms to send) felt richer
and more responsive on the real screen. Until Fusuma has dirty-rectangle redraws and the integer renderer is
removed, the shipping launcher still uses the integer renderer and repaints the whole screen. The P4 is
untested. Numbers: `tools/bench-s3/RESULT.md`.

A bug worth knowing: the CO5300 needs an explicit memory-write command after each address window. Without
it a partial update lands in the wrong place and tears. Waveshare's own driver sends it; mine did not.

## Runtime, and what comes next

Today the firmware is one bare-metal loop: poll touch, recognise gestures, update the launcher, draw. WiFi
and Bluetooth need more. The plan, none of it started:

* **`esp-rtos` and Embassy** as the runtime, with `esp-radio` (a beta) for WiFi and BLE, `trouble-host`
  for BLE and `embassy-net` for TCP/IP.
* **Services** (network, storage, time) as async tasks that talk through typed messages; a `hibari-services`
  crate when the first one exists.
* **Async HAL traits** for storage and network. Whether display and touch go async too is undecided.
* **Storage:** key-value settings in a flash partition (`sequential-storage`), and a partition layout for
  Hibari on the 32 MB flash.
* **Drivers:** PCF85063 (RTC, no crate exists), QMI8658 (a crate exists), AXP2101 (a 2024 crate exists),
  buttons. Then a status bar with real time and battery.
* **The app model** waits for a first real app and for Fusuma's widget tree. An app is probably a Fusuma tree
  plus a manifest and a lifecycle; the earlier `App`, `Event` and `Effect` types were deleted because
  nothing used them.
* **Gestures** probably move into Fusuma, which has its own pointer events and needs them on desktops and
  phones too.

Memory is the real constraint and is only partly measured: the 410x502 RGB565 frame buffer is 412 KB in
PSRAM; the toolkit tree is a few KB; internal RAM, stack use and what the radio takes are not measured.

## The Dodai family

Names follow a Japanese house. Hibari OS (雲雀, skylark) is this repository.

| Part | Name | Where |
|---|---|---|
| UI toolkit and renderer | **Fusuma** | github.com/Dodai-Tech/fusuma |
| Design system | **Dodai UI** | github.com/Dodai-Tech/dodai-ui |
| Desktop environment, app store, Linux distribution | Kawara, Noren, Dodai Linux | ideas, no code; not part of Hibari OS |

Hibari depends on Fusuma, and Fusuma never depends on Hibari or on Dodai UI. `dodai-fusuma`, in the
design-system repository, will apply the Dodai themes to Fusuma.

## Rules

* The libraries stay `no_std`, with `unsafe_code = "forbid"`. `unsafe` belongs to board crates, each use
  with a safety comment.
* Every parser and state machine is unit-tested and fuzzed (`fuzz/`), and `just check-all` stays green.
* A board is a file plus a crate, never an edit of a shared list.
* Radio code is Espressif's closed library; networking wraps it.
