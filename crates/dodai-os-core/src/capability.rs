//! What a board can do and what an app needs, as a tiny bit set.

/// One hardware or service feature a board may offer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Capability {
    /// A screen to draw on.
    Display = 0,
    /// A touch panel.
    Touch = 1,
    /// At least one physical button.
    Button = 2,
    /// A status LED.
    Led = 3,
    /// WiFi.
    Wifi = 4,
    /// Bluetooth LE.
    Ble = 5,
    /// A battery with a fuel gauge.
    Battery = 6,
    /// A battery-backed real-time clock.
    Rtc = 7,
    /// An inertial sensor.
    Imu = 8,
    /// Persistent key-value storage.
    Storage = 9,
}

impl Capability {
    /// Every capability, in bit order.
    pub const ALL: [Capability; 10] = [
        Capability::Display,
        Capability::Touch,
        Capability::Button,
        Capability::Led,
        Capability::Wifi,
        Capability::Ble,
        Capability::Battery,
        Capability::Rtc,
        Capability::Imu,
        Capability::Storage,
    ];

    const fn bit(self) -> u16 {
        1 << (self as u8)
    }
}

/// A set of [`Capability`] values (one `u16`, `Copy`, usable in `const`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CapabilitySet(u16);

impl CapabilitySet {
    /// The empty set.
    pub const EMPTY: Self = Self(0);

    /// Builds a set from a slice (usable in `const`).
    #[must_use]
    pub const fn of(caps: &[Capability]) -> Self {
        let mut bits = 0;
        let mut i = 0;
        while i < caps.len() {
            bits |= caps[i].bit();
            i += 1;
        }
        Self(bits)
    }

    /// This set plus `cap`.
    #[must_use]
    pub const fn with(self, cap: Capability) -> Self {
        Self(self.0 | cap.bit())
    }

    /// Whether `cap` is in the set.
    #[must_use]
    pub const fn contains(self, cap: Capability) -> bool {
        self.0 & cap.bit() != 0
    }

    /// Whether this set offers everything in `required`.
    #[must_use]
    pub const fn satisfies(self, required: Self) -> bool {
        self.0 & required.0 == required.0
    }

    /// Number of capabilities in the set.
    #[must_use]
    pub const fn len(self) -> u32 {
        self.0.count_ones()
    }

    /// Whether the set is empty.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The capabilities in the set, in bit order.
    pub fn iter(self) -> impl Iterator<Item = Capability> {
        Capability::ALL
            .into_iter()
            .filter(move |c| self.contains(*c))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WATCH: CapabilitySet =
        CapabilitySet::of(&[Capability::Display, Capability::Touch, Capability::Wifi]);

    #[test]
    fn builds_and_queries() {
        assert!(WATCH.contains(Capability::Touch));
        assert!(!WATCH.contains(Capability::Ble));
        assert_eq!(WATCH.len(), 3);
        assert!(CapabilitySet::EMPTY.is_empty());
    }

    #[test]
    fn satisfies_is_superset() {
        assert!(WATCH.satisfies(CapabilitySet::of(&[Capability::Display])));
        assert!(WATCH.satisfies(CapabilitySet::EMPTY));
        assert!(!WATCH.satisfies(CapabilitySet::of(&[Capability::Display, Capability::Ble])));
    }

    #[test]
    fn with_adds_and_iter_is_ordered() {
        let s = CapabilitySet::EMPTY
            .with(Capability::Imu)
            .with(Capability::Display);
        assert_eq!(
            s.iter().collect::<Vec<_>>(),
            [Capability::Display, Capability::Imu]
        );
    }

    #[test]
    fn every_capability_has_its_own_bit() {
        let all = CapabilitySet::of(&Capability::ALL);
        assert_eq!(all.len() as usize, Capability::ALL.len());
    }
}
