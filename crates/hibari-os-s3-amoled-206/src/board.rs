//! Pin map and facts of the Waveshare ESP32-S3-Touch-AMOLED-2.06.
//!
//! Source: Waveshare's `pin_config.h` (QSPI display, I2C, touch). GPIO numbers are documented
//! here; the typed pins are taken in `main.rs`, because esp-hal wants one owned handle per pin.

use hibari_os_core::{Capability, CapabilitySet};
use hibari_os_drivers::co5300::Panel;

/// Board name used in logs and board profiles.
pub const NAME: &str = "waveshare-s3-amoled-2.06";

/// What the board offers (drivers for some of them are still missing; see the roadmap).
pub const CAPABILITIES: CapabilitySet = CapabilitySet::of(&[
    Capability::Display,
    Capability::Touch,
    Capability::Button,
    Capability::Wifi,
    Capability::Ble,
    Capability::Battery,
    Capability::Rtc,
    Capability::Imu,
    Capability::Storage,
]);

/// The AMOLED panel: 410x502, with 22 hidden columns before the first visible one.
pub const PANEL: Panel = Panel::WAVESHARE_206;

/// QSPI clock in MHz.
pub const LCD_QSPI_MHZ: u32 = 80;

// GPIO map, for reference (typed in main.rs):
//   LCD  SDIO0..3 = 4,5,6,7   SCLK = 11   CS = 12   RESET = 8   TE = 13
//   I2C  SDA = 15  SCL = 14   (touch 0x38, PMIC 0x34, RTC 0x51, IMU 0x6B)
//   Touch INT = 38  RESET = 9
//   Buttons: BOOT = 0, PWR = 10
