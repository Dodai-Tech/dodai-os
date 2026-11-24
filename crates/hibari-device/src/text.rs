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
    #[serde(default)]
    capabilities: Vec<&'a str>,
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
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFirmware<'a> {
    chip: &'a str,
    target: &'a str,
    flash_mb: u32,
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
    /// shape = "rect"      # or "round"; default "rect"
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
    /// gpu = false         # default false
    ///
    /// [render]            # optional
    /// backend = "skia"    # integer, skia, vello-cpu or vello-gpu; default: automatic
    /// fallback = false    # step down to a lighter backend if this one does not fit; default true
    /// min_free_kb = 100   # internal RAM kept free after the frame; default 100 (a guess)
    /// ```
    ///
    /// # Errors
    /// [`ProfileError::Parse`] for text that is not a device file, or the error of
    /// [`validate`](Self::validate).
    pub fn from_toml(text: &'static str) -> Result<Self, ProfileError> {
        let raw: RawProfile<'static> = toml::from_str(text).map_err(|e| bad(e.to_string()))?;
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
                let shape = match d.shape {
                    None | Some("rect") => Shape::Rect,
                    Some("round") => Shape::Round,
                    Some(other) => {
                        return Err(bad(std::format!(
                            "unknown shape '{other}'; use \"rect\" or \"round\""
                        )));
                    }
                };
                let [x, y] = d.offset.unwrap_or([0, 0]);
                Ok(DisplayInfo {
                    width: d.width,
                    height: d.height,
                    shape,
                    offset: (x, y),
                })
            })
            .transpose()?;
        let firmware = raw.firmware.map(|f| Firmware {
            chip: f.chip,
            target: f.target,
            flash_mb: f.flash_mb,
        });
        let hardware = raw.hardware.map(|h| Hardware {
            ram_kb: h.ram_kb,
            psram_kb: h.psram_kb,
            fpu: h.fpu,
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
                "Some(hibari_device::Hardware {{ ram_kb: {}, psram_kb: {}, fpu: {}, gpu: {} }})",
                h.ram_kb,
                h.psram_kb,
                h.fpu,
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
        let round = DeviceProfile::from_toml("name = \"r\"\ncapabilities = [\"display\"]\n[display]\nwidth = 466\nheight = 466\nshape = \"round\"\n").unwrap();
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
        assert!(err("name = \"\"\n").contains("no name"));
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
                .contains("zero")
        );
        assert!(err("name = \"x\"\ncapabilities = [\"display\"]\n[display]\nwidth = 70000\nheight = 5\n").contains("70000") || err("name = \"x\"\ncapabilities = [\"display\"]\n[display]\nwidth = 70000\nheight = 5\n").contains("invalid"));
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
backend = "skia"
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
                gpu: false
            })
        );
        assert_eq!(
            p.render,
            Render {
                backend: Some(Backend::Skia),
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
        let headless = "name = \"h\"\ncapabilities = [\"wifi\"]\n[render]\nbackend = \"skia\"\n";
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
            "hardware: Some(hibari_device::Hardware { ram_kb: 512, psram_kb: 0, fpu: false, gpu: false })",
            "render: hibari_device::Render { backend: Some(hibari_device::Backend::Skia), fallback: true, min_free_kb: 100 }",
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
        assert_eq!(c.backend, Backend::VelloCpu);
        assert!(matches!(c.reason, crate::Reason::Automatic(_)));
        assert_eq!(crate::check(&p, c).1, crate::Fit::Fits);
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
