# hibari-os

[![CI](https://github.com/Dodai-Tech/hibari-os/actions/workflows/ci.yml/badge.svg)](https://github.com/Dodai-Tech/hibari-os/actions/workflows/ci.yml)
[![Security](https://github.com/Dodai-Tech/hibari-os/actions/workflows/security.yml/badge.svg)](https://github.com/Dodai-Tech/hibari-os/actions/workflows/security.yml)
[![MSRV](https://img.shields.io/badge/MSRV-1.90-blue.svg)](Cargo.toml)
[![no_std](https://img.shields.io/badge/libraries-no__std-success.svg)](crates)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

**A thin operating system for ESP32 boards, written in Rust.** A launcher, touch input, drivers and
an app model on `esp-hal`, with no ESP-IDF and a small footprint. Hibari (雲雀) is the skylark: small,
fast and light. It is part of the Dodai family, next to
[Fusuma](https://github.com/Dodai-Tech/fusuma) for the interface and
[Dodai UI](https://github.com/Dodai-Tech/dodai-ui) for the design system.

**What runs today.** On the **ESP32-S3** (Waveshare ESP32-S3-Touch-AMOLED-2.06): a launcher with a
status bar, an app grid, touch gestures and slide transitions, installed next to the board's own
firmware so you can go back. A demo that draws the cards anti-aliased with `vello_cpu` and repaints only
the touched card also runs there.

**What does not exist yet.** WiFi, Bluetooth, storage and settings, the power, RTC and IMU drivers, real
apps, and the other chips (the ESP32-P4 and the ESP32-C6 are planned). See the [roadmap](docs/ROADMAP.md).

**A device is a file.** `boards/<board>/board.toml` names the device, its screen, its capabilities and how
to flash it. The firmware, the host preview and the build scripts all read it. Everything that is not
hardware is `no_std`, host-testable and fuzzed, and the launcher also runs in a window or a PNG on your
computer with the same code.

*Not affiliated with Espressif Systems. ESP32 is a trademark of Espressif Systems.*

## Docs

* [Architecture](docs/ARCHITECTURE.md)
* [Roadmap](docs/ROADMAP.md)

## Layout

```
crates/                      the libraries, all no_std
  hibari-device              what a device is: capabilities, screen, board.toml
  hibari-hal                 device traits (display, touch, backlight, clock), touch input and gestures
  hibari-drivers             chip drivers: CO5300 display, FT3168 touch
  hibari-launcher            app manifest, paging and navigation, an interim view, a frame buffer
boards/                      one folder per device, not published
  s3-amoled-206              board.toml, the board library and the firmware (esp-hal)
  esp32s3-idf                a sample ESP-IDF firmware
tools/                       not published
  host                       the launcher on a desktop: PNG screenshots, a window
  bench-s3                   benchmark firmware: vello_cpu against the integer renderer
  vello-demo-s3              demo firmware: an anti-aliased launcher on the S3
experiments/
  kernel                     a cooperative scheduler that nothing uses
```

The workspace members (`crates/*`, `tools/host`, `experiments/kernel`) build and test on any machine.
Everything that targets the ESP needs the Xtensa toolchain (`espup install`) and is built on its own, with
its own `.cargo/config.toml` and `rust-toolchain.toml`. The renderers come from
[Fusuma](https://github.com/Dodai-Tech/fusuma) on crates.io: `fusuma-render-embedded` on the board,
`fusuma-render-vello-cpu` for the host's anti-aliased screenshots.

## Prerequisites

- [Rust](https://rustup.rs), [just](https://github.com/casey/just), [nu](https://www.nushell.sh), [git-cliff](https://github.com/orhun/git-cliff)
- `cargo install espup espflash`; for Xtensa chips (esp32, s2, s3) run `espup install`
- Only the sample ESP-IDF firmware also needs `ldproxy`; its pinned nightly installs itself on first build (ESP-IDF downloads to `~/.espressif`: 10+ min, ~350 MB)

## Use

```sh
just backup                 # save the board's native firmware before the first install
just native-install         # build the esp-hal firmware, install it in OTA slot 0 and boot it
just native-install board=s3-vello-demo   # the vello_cpu demo instead
just native-restore         # boot the native firmware again (erases only the otadata partition)
just native-monitor         # serial monitor
cargo run -p hibari-host -- --list
cargo run -p hibari-host -- --device waveshare-s3-amoled-2.06 --tap 300,56 --at 60 --png shot.png
cargo run -p hibari-host -- --device watch-round --renderer vello-cpu --png shot.png   # --renderer auto is the default
cargo run -p hibari-host --features window -- --device watch-round --window
```

Probe the board first: `espflash board-info --port /dev/ttyACM0` (chip, revision, flash size must match the crate).
Troubleshooting lives in the `esp-boards-rust` skill (`references/pitfalls.md`).

## Development

| Command | Description |
|---|---|
| `just check-all` | The CI gate: fmt, clippy, tests, docs, version check, unused deps, nu tests |
| `just fuzz-all` | Short run of every fuzz target (nightly + cargo-fuzz) |
| `just audit` / `just deny` | Known vulnerabilities / licenses, bans, sources |
| `just bump X.Y.Z` | Bump the version, update the changelog, commit and tag |

Firmware is not built in CI (the ESP toolchain is slow and large): run `just native-build` before a release.
Commits follow [Conventional Commits](https://www.conventionalcommits.org); `CHANGELOG.md` is generated by git-cliff.

## License

MIT
