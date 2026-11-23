//! Device files: loading a [`DeviceProfile`] from TOML and writing it out as Rust constants.

use std::string::{String, ToString};
use std::vec::Vec;

use serde::Deserialize;

use crate::{Capability, CapabilitySet, DeviceProfile, DisplayInfo, Firmware, ProfileError, Shape};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProfile<'a> {
    name: &'a str,
    #[serde(default)]
    capabilities: Vec<&'a str>,
    display: Option<RawDisplay<'a>>,
    firmware: Option<RawFirmware<'a>>,
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
        let profile = Self {
            name: raw.name,
            display,
            capabilities,
            firmware,
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
        std::format!(
            "pub const {const_name}: hibari_device::DeviceProfile = hibari_device::DeviceProfile {{\n    name: {:?},\n    display: {display},\n    capabilities: hibari_device::CapabilitySet::of(&[{}]),\n    firmware: {firmware},\n}};\n",
            self.name,
            caps.join(", ")
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
