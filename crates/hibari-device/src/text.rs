//! Device files: loading a [`DeviceProfile`] from TOML and writing it out as Rust constants.

use std::string::{String, ToString};
use std::vec::Vec;

use serde::Deserialize;

use crate::{
    Backend, Capability, CapabilitySet, DeviceProfile, DisplayInfo, Firmware, Hardware,
    ProfileError, Render, Shape,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProfile<'a> {
    name: &'a str,
    #[serde(borrow)]
    about: Option<&'a str>,
    #[serde(default)]
    capabilities: Vec<&'a str>,
    // The parts and their wiring, for a firmware's drivers: checked by Fusuma's board-file reader, not used here.
    touch: Option<toml::Value>,
    backlight: Option<toml::Value>,
    display: Option<RawDisplay<'a>>,
    firmware: Option<RawFirmware<'a>>,
    hardware: Option<RawHardware>,
    render: Option<RawRender<'a>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHardware {
    ram_kb: u32,
    #[serde(default)]
    psram_kb: u32,
    #[serde(default)]
    fpu: bool,
    #[serde(default)]
    simd: bool,
    #[serde(default)]
    gpu: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRender<'a> {
    backend: Option<&'a str>,
    #[serde(default)]
    fallback: Option<bool>,
    #[serde(default)]
    min_free_kb: Option<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDisplay<'a> {
    width: u16,
    height: u16,
    shape: Option<&'a str>,
    offset: Option<[u16; 2]>,
    // The panel's outline and wiring, for a firmware's drivers: checked by Fusuma's board-file reader, not used here.
    radius: Option<u32>,
    grid: Option<u32>,
    controller: Option<&'a str>,
    bus: Option<&'a str>,
    pins: Option<toml::Value>,
    options: Option<toml::Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFirmware<'a> {
    chip: &'a str,
    target: Option<&'a str>,
    flash_mb: u32,
    boot_limit_mb: Option<u32>,
}

/// The bare-metal (esp-hal) Rust target of an ESP chip, for a device file that does not say its own.
fn bare_metal_target(chip: &str) -> Option<&'static str> {
    Some(match chip {
        "esp32" => "xtensa-esp32-none-elf",
        "esp32s2" => "xtensa-esp32s2-none-elf",
        "esp32s3" => "xtensa-esp32s3-none-elf",
        "esp32c2" | "esp32c3" => "riscv32imc-unknown-none-elf",
        "esp32c6" | "esp32h2" => "riscv32imac-unknown-none-elf",
        "esp32p4" => "riscv32imafc-unknown-none-elf",
        _ => return None,
    })
}

fn bad(msg: String) -> ProfileError {
    ProfileError::Parse(msg)
}

impl DeviceProfile {
    /// Reads a device file. Unknown fields and capabilities are errors, and the result is
    /// [validated](Self::validate). The text must live for the whole program (`include_str!`), because
    /// the profile borrows its name and firmware strings from it.
    ///
    /// ```toml
    /// name = "waveshare-s3-amoled-2.06"
    /// capabilities = ["display", "touch", "wifi"]
    ///
    /// [display]
    /// width = 410
    /// height = 502
    /// shape = "rect"      # rect, rounded (with radius) or circle; default "rect"
    /// offset = [22, 0]    # default [0, 0]
    ///
    /// [firmware]          # only for a real board
    /// chip = "esp32s3"
    /// target = "xtensa-esp32s3-none-elf"
    /// flash_mb = 32
    ///
    /// [hardware]          # what the chip has; the renderer choice reads it
    /// ram_kb = 512
    /// psram_kb = 8192     # default 0
    /// fpu = true          # default false
    /// simd = false        # whether Vello has SIMD here (x86, aarch64, wasm); default false, as on every chip
    /// gpu = false         # default false
    ///
    /// [render]            # optional
    /// backend = "integer" # integer, vello-cpu or vello-gpu; default: automatic
    /// fallback = false    # step down to a lighter backend if this one does not fit; default true
    /// min_free_kb = 100   # internal RAM kept free after the frame; default 100 (a guess)
    /// ```
    ///
    /// # Errors
    /// [`ProfileError::Parse`] for text that is not a device file, or the error of
    /// [`validate`](Self::validate).
    pub fn from_toml(text: &'static str) -> Result<Self, ProfileError> {
        // The format is Fusuma's board file: its reader checks every key, the parts' controllers and their pins.
        fusuma_platform::board::BoardFile::from_toml(text, "device file").map_err(|e| bad(e.0))?;
        let raw: RawProfile<'static> = toml::from_str(text).map_err(|e| bad(e.to_string()))?;
        let _ = (&raw.about, &raw.touch, &raw.backlight);
        let mut capabilities = CapabilitySet::EMPTY;
        for name in &raw.capabilities {
            let cap = Capability::from_name(name).ok_or_else(|| {
                let known: Vec<&str> = Capability::ALL.iter().map(|c| c.name()).collect();
                bad(std::format!(
                    "unknown capability '{name}'; known: {}",
                    known.join(", ")
                ))
            })?;
            if capabilities.contains(cap) {
                return Err(bad(std::format!("capability '{name}' is listed twice")));
            }
            capabilities = capabilities.with(cap);
        }
        let display = raw
            .display
            .map(|d| {
                // Hibari draws square or round screens; rounded corners are drawn square for now.
                let shape = match d.shape {
                    None | Some("rect" | "rounded") => Shape::Rect,
                    Some("circle") => Shape::Round,
                    Some(other) => {
                        return Err(bad(std::format!(
                            "unknown shape '{other}'; use rect, rounded or circle"
                        )));
                    }
                };
                let _ = (d.radius, d.grid, d.controller, d.bus, &d.pins, &d.options);
                let [x, y] = d.offset.unwrap_or([0, 0]);
                Ok(DisplayInfo {
                    width: d.width,
                    height: d.height,
                    shape,
                    offset: (x, y),
                })
            })
            .transpose()?;
        let firmware = raw
            .firmware
            .map(|f| {
                let _ = f.boot_limit_mb;
                let target = f
                    .target
                    .or_else(|| bare_metal_target(f.chip))
                    .ok_or_else(|| {
                        bad(std::format!(
                            "firmware.target: none known for chip '{}'",
                            f.chip
                        ))
                    })?;
                Ok(Firmware {
                    chip: f.chip,
                    target,
                    flash_mb: f.flash_mb,
                })
            })
            .transpose()?;
        let hardware = raw.hardware.map(|h| Hardware {
            ram_kb: h.ram_kb,
            psram_kb: h.psram_kb,
            fpu: h.fpu,
            simd: h.simd,
            gpu: h.gpu,
        });
        let render = match raw.render {
            None => Render::default(),
            Some(r) => Render {
                backend: r
                    .backend
                    .map(|name| {
                        Backend::from_name(name).ok_or_else(|| {
                            let known: Vec<&str> = Backend::ALL.iter().map(|b| b.name()).collect();
                            bad(std::format!(
                                "unknown render backend '{name}'; known: {}",
                                known.join(", ")
                            ))
                        })
                    })
                    .transpose()?,
                fallback: r.fallback.unwrap_or(Render::default().fallback),
                min_free_kb: r.min_free_kb.unwrap_or(Render::default().min_free_kb),
            },
        };
        let profile = Self {
            name: raw.name,
            display,
            capabilities,
            firmware,
            hardware,
            render,
        };
        profile.validate()?;
        Ok(profile)
    }

    /// Rust source for a `pub const` of this profile named `const_name`, for a build script to write
    /// into `OUT_DIR`. It refers to the `hibari_device` crate by name.
    #[must_use]
    pub fn to_rust(&self, const_name: &str) -> String {
        let caps: Vec<String> = Capability::ALL
            .iter()
            .filter(|c| self.capabilities.contains(**c))
            .map(|c| std::format!("hibari_device::Capability::{}", c.variant()))
            .collect();
        let display = match self.display {
            None => String::from("None"),
            Some(d) => std::format!(
                "Some(hibari_device::DisplayInfo {{ width: {}, height: {}, shape: hibari_device::Shape::{:?}, offset: ({}, {}) }})",
                d.width,
                d.height,
                d.shape,
                d.offset.0,
                d.offset.1
            ),
        };
        let firmware = match self.firmware {
            None => String::from("None"),
            Some(f) => std::format!(
                "Some(hibari_device::Firmware {{ chip: {:?}, target: {:?}, flash_mb: {} }})",
                f.chip,
                f.target,
                f.flash_mb
            ),
        };
        let hardware = match self.hardware {
            None => String::from("None"),
            Some(h) => std::format!(
                "Some(hibari_device::Hardware {{ ram_kb: {}, psram_kb: {}, fpu: {}, simd: {}, gpu: {} }})",
                h.ram_kb,
                h.psram_kb,
                h.fpu,
                h.simd,
                h.gpu
            ),
        };
        let backend = match self.render.backend {
            None => String::from("None"),
            Some(b) => std::format!("Some(hibari_device::Backend::{})", b.variant()),
        };
        std::format!(
            "pub const {const_name}: hibari_device::DeviceProfile = hibari_device::DeviceProfile {{\n    name: {:?},\n    display: {display},\n    capabilities: hibari_device::CapabilitySet::of(&[{}]),\n    firmware: {firmware},\n    hardware: {hardware},\n    render: hibari_device::Render {{ backend: {backend}, fallback: {}, min_free_kb: {} }},\n}};\n",
            self.name,
            caps.join(", "),
            self.render.fallback,
            self.render.min_free_kb
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WATCH: &str = r#"
name = "test-watch"
capabilities = ["display", "touch", "wifi"]

[display]
width = 410
height = 502
offset = [22, 0]

[firmware]
chip = "esp32s3"
target = "xtensa-esp32s3-none-elf"
flash_mb = 32
"#;

    fn err(text: &'static str) -> String {
        DeviceProfile::from_toml(text).unwrap_err().to_string()
    }

    #[test]
    fn a_device_file_becomes_a_profile() {
        let p = DeviceProfile::from_toml(WATCH).unwrap();
        assert_eq!(p.name, "test-watch");
        let d = p.display.unwrap();
        assert_eq!(
            (d.width, d.height, d.shape, d.offset),
            (410, 502, Shape::Rect, (22, 0))
        );
        assert!(
            p.capabilities.contains(Capability::Touch) && !p.capabilities.contains(Capability::Ble)
        );
        let f = p.firmware.unwrap();
        assert_eq!(
            (f.chip, f.target, f.flash_mb),
            ("esp32s3", "xtensa-esp32s3-none-elf", 32)
        );
    }

    #[test]
    fn defaults_and_a_headless_device() {
        let p = DeviceProfile::from_toml("name = \"box\"\ncapabilities = [\"wifi\", \"led\"]\n")
            .unwrap();
        assert!(p.display.is_none() && p.firmware.is_none());
        let round = DeviceProfile::from_toml("name = \"r\"\ncapabilities = [\"display\"]\n[display]\nwidth = 466\nheight = 466\nshape = \"circle\"\n").unwrap();
        assert_eq!(round.display.unwrap().shape, Shape::Round);
        assert_eq!(round.display.unwrap().offset, (0, 0));
        let none = DeviceProfile::from_toml("name = \"n\"\n").unwrap();
        assert!(none.capabilities.is_empty());
    }

    #[test]
    fn bad_files_say_what_is_wrong() {
        assert!(
            err("name = \"x\"\nfoo = 1\n").contains("foo"),
            "unknown fields are errors"
        );
        assert!(
            err("name = \"x\"\ncapabilities = [\"gpu\"]\n").contains("unknown capability 'gpu'")
        );
        assert!(
            err("name = \"x\"\ncapabilities = [\"wifi\", \"wifi\"]\n").contains("listed twice")
        );
        assert!(err("name = \"x\"\ncapabilities = [\"display\"]\n[display]\nwidth = 5\nheight = 5\nshape = \"oval\"\n").contains("oval"));
        assert!(err("name = \"x\"\ncapabilities = [\"display\"]\n[display]\nwidth = 5\nheight = 5\nbogus = 1\n").contains("bogus"));
        assert!(
            err("capabilities = []\n").contains("name"),
            "the name is required"
        );
        assert!(err("name = \"\"\n").contains("name ''"), "an empty name");
        assert!(
            err("name = \"x\"\ncapabilities = [\"display\"]\n").contains("display"),
            "a display capability needs a screen"
        );
        assert!(
            err("name = \"x\"\n[display]\nwidth = 5\nheight = 5\n").contains("display"),
            "a screen needs the capability"
        );
        assert!(
            err("name = \"x\"\ncapabilities = [\"display\"]\n[display]\nwidth = 0\nheight = 5\n")
                .contains("display.width")
        );
        assert!(
            err(
                "name = \"x\"\ncapabilities = [\"display\"]\n[display]\nwidth = 70000\nheight = 5\n"
            )
            .contains("display.width")
        );
        assert!(!err("this is not toml").is_empty());
        assert!(!err("").is_empty());
    }

    #[test]
    fn the_names_of_the_capabilities_are_listed_in_the_error() {
        let e = err("name = \"x\"\ncapabilities = [\"gpu\"]\n");
        for c in Capability::ALL {
            assert!(e.contains(c.name()), "{e}");
        }
    }

    #[test]
    fn generated_source_names_everything() {
        let p = DeviceProfile::from_toml(WATCH).unwrap();
        let src = p.to_rust("PROFILE");
        for want in [
            "pub const PROFILE: hibari_device::DeviceProfile",
            "name: \"test-watch\"",
            "width: 410, height: 502, shape: hibari_device::Shape::Rect, offset: (22, 0)",
            "hibari_device::Capability::Display, hibari_device::Capability::Touch, hibari_device::Capability::Wifi",
            "chip: \"esp32s3\", target: \"xtensa-esp32s3-none-elf\", flash_mb: 32",
        ] {
            assert!(src.contains(want), "missing {want:?} in:\n{src}");
        }
        let plain = DeviceProfile::from_toml("name = \"box\"\n")
            .unwrap()
            .to_rust("BOX");
        assert!(
            plain.contains("display: None")
                && plain.contains("firmware: None")
                && plain.contains("CapabilitySet::of(&[])")
        );
    }

    const WITH_RENDER: &str = r#"
name = "r"
capabilities = ["display"]

[display]
width = 160
height = 160

[hardware]
ram_kb = 512

[render]
backend = "vello-cpu"
fallback = true
"#;

    #[test]
    fn hardware_and_render_tables_are_read_with_defaults() {
        let p = DeviceProfile::from_toml(WITH_RENDER).unwrap();
        assert_eq!(
            p.hardware,
            Some(Hardware {
                ram_kb: 512,
                psram_kb: 0,
                fpu: false,
                simd: false,
                gpu: false
            })
        );
        assert_eq!(
            p.render,
            Render {
                backend: Some(Backend::VelloCpu),
                fallback: true,
                min_free_kb: Render::default().min_free_kb
            }
        );
        let none = DeviceProfile::from_toml("name = \"n\"\n").unwrap();
        assert_eq!((none.hardware, none.render), (None, Render::default()));
        let only_fallback = DeviceProfile::from_toml("name = \"n\"\ncapabilities = [\"display\"]\n[display]\nwidth = 5\nheight = 5\n[render]\nfallback = true\n").unwrap();
        assert_eq!(
            only_fallback.render,
            Render {
                backend: None,
                fallback: true,
                min_free_kb: Render::default().min_free_kb
            }
        );
    }

    #[test]
    fn bad_render_and_hardware_tables_say_what_is_wrong() {
        let screen = "capabilities = [\"display\"]\n[display]\nwidth = 5\nheight = 5\n";
        let with = |extra: &str| -> String {
            let text: &'static str = std::boxed::Box::leak(
                std::format!("name = \"x\"\n{screen}{extra}").into_boxed_str(),
            );
            DeviceProfile::from_toml(text).unwrap_err().to_string()
        };
        assert!(with("[render]\nbackend = \"gpu\"\n").contains("unknown render backend 'gpu'"));
        assert!(
            with("[render]\nbackend = \"gpu\"\n").contains("vello-cpu"),
            "the known names are listed"
        );
        assert!(with("[render]\nbogus = 1\n").contains("bogus"));
        assert!(with("[hardware]\nram_kb = 0\n").contains("ram_kb"));
        assert!(
            with("[hardware]\npsram_kb = 4\n").contains("ram_kb"),
            "ram_kb is required"
        );
        assert!(with("[hardware]\nram_kb = 512\ncpu = 1\n").contains("cpu"));
        let headless = "name = \"h\"\ncapabilities = [\"wifi\"]\n[render]\nbackend = \"integer\"\n";
        assert!(
            DeviceProfile::from_toml(headless)
                .unwrap_err()
                .to_string()
                .contains("[render] needs a [display]")
        );
    }

    #[test]
    fn generated_source_carries_hardware_and_render() {
        let src = DeviceProfile::from_toml(WITH_RENDER).unwrap().to_rust("P");
        for want in [
            "hardware: Some(hibari_device::Hardware { ram_kb: 512, psram_kb: 0, fpu: false, simd: false, gpu: false })",
            "render: hibari_device::Render { backend: Some(hibari_device::Backend::VelloCpu), fallback: true, min_free_kb: 100 }",
        ] {
            assert!(src.contains(want), "{want}\n{src}");
        }
        let plain = DeviceProfile::from_toml("name = \"b\"\n")
            .unwrap()
            .to_rust("B");
        assert!(
            plain.contains("hardware: None")
                && plain
                    .contains("render: hibari_device::Render { backend: None, fallback: true, min_free_kb: 100 }")
        );
    }

    #[test]
    fn the_real_s3_board_chooses_what_was_measured_to_suit_it() {
        let p = DeviceProfile::from_toml(include_str!("../../../boards/s3-amoled-206/board.toml"))
            .unwrap();
        assert_eq!(p.hardware.unwrap().psram_kb, 8192);
        let c = crate::choose(&p, None).unwrap();
        // Vello has no SIMD on the S3: measured on the watch, a tap takes 7 ms on integer against 47 on Vello.
        assert_eq!(c.backend, Backend::Integer);
        assert!(matches!(c.reason, crate::Reason::Automatic(_)));
        assert_eq!(crate::check(&p, c).1, crate::Fit::Fits);
    }

    /// Fusuma's board file for the same watch (its ESP-IDF firmware builds from it) is the same format: Hibari reads
    /// it as it is, with its wiring, and gets the bare-metal target from the chip.
    #[test]
    fn fusumas_board_file_for_the_watch_reads_here() {
        let p = DeviceProfile::from_toml(include_str!(
            "../../../../../fusuma/crates/fusuma-platform-esp/boards/s3-amoled-206/board.toml"
        ))
        .unwrap();
        assert_eq!(p.screen_size(), (410, 502));
        let f = p.firmware.unwrap();
        assert_eq!(
            (f.chip, f.target, f.flash_mb),
            ("esp32s3", "xtensa-esp32s3-none-elf", 32)
        );
        assert_eq!(p.display.unwrap().offset, (22, 0));
        assert!(p.capabilities.contains(Capability::Touch));
    }

    /// The device files of this repository, which the firmware and the host both load.
    const REAL: [&str; 1] = [include_str!("../../../boards/s3-amoled-206/board.toml")];

    #[test]
    fn the_real_boards_are_valid_and_have_firmware_settings() {
        for text in REAL {
            let p = DeviceProfile::from_toml(text).unwrap_or_else(|e| panic!("{e}"));
            assert!(p.firmware.is_some(), "{} needs a [firmware] table", p.name);
        }
    }
}
