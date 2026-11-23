//! Which renderer a device draws with. The logic (the backends, the hardware facts, the layers, the memory
//! check) lives in Fusuma's `fusuma-platform`, so that every platform shares it; this module re-exports it
//! and lets a [`DeviceProfile`] be asked the question.

use crate::DeviceProfile;

pub use fusuma_platform::{
    Backend, Choice, Fit, Hardware, Machine, Plan, RAM_SHARE_PERCENT, Reason, Render, budget, fits,
    nearest,
};

/// What the choice needs to know about a device: its screen, its hardware and its render settings.
#[must_use]
pub fn machine(profile: &DeviceProfile) -> Machine {
    Machine {
        screen: profile
            .display
            .map(|d| (u32::from(d.width), u32::from(d.height))),
        hardware: profile.hardware,
        render: profile.render,
    }
}

/// The automatic rule for `profile`; `None` for a device without a screen.
#[must_use]
pub fn automatic(profile: &DeviceProfile) -> Option<Choice> {
    fusuma_platform::automatic(&machine(profile))
}

/// The layers (build, board file, automatic rule); `None` for a device without a screen.
#[must_use]
pub fn choose(profile: &DeviceProfile, build: Option<Backend>) -> Option<Choice> {
    fusuma_platform::choose(&machine(profile), build)
}

/// Checks a choice against the device's memory.
#[must_use]
pub fn check(profile: &DeviceProfile, choice: Choice) -> (Choice, Fit) {
    fusuma_platform::check(&machine(profile), choice)
}

/// The whole decision for `profile`: layers, memory check and what is `available` in this program.
#[must_use]
pub fn plan(
    profile: &DeviceProfile,
    build: Option<Backend>,
    available: &[Backend],
) -> Option<Plan> {
    fusuma_platform::plan(&machine(profile), build, available)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Capability, CapabilitySet, DisplayInfo, Shape};

    fn device(hw: Option<Hardware>, screen: bool) -> DeviceProfile {
        DeviceProfile {
            name: "t",
            display: screen.then_some(DisplayInfo {
                width: 410,
                height: 502,
                shape: Shape::Rect,
                offset: (0, 0),
            }),
            capabilities: CapabilitySet::of(&[Capability::Display]),
            firmware: None,
            hardware: hw,
            render: Render::default(),
        }
    }

    #[test]
    fn a_profile_becomes_the_machine_the_chooser_reads() {
        let hw = Hardware {
            ram_kb: 512,
            psram_kb: 8192,
            fpu: true,
            gpu: false,
        };
        let m = machine(&device(Some(hw), true));
        assert_eq!(
            (m.screen, m.hardware, m.render),
            (Some((410, 502)), Some(hw), Render::default())
        );
        assert_eq!(machine(&device(None, false)).screen, None);
    }

    #[test]
    fn the_wrappers_answer_for_a_profile() {
        let hw = Hardware {
            ram_kb: 512,
            psram_kb: 8192,
            fpu: true,
            gpu: false,
        };
        let p = device(Some(hw), true);
        assert_eq!(automatic(&p).unwrap().backend, Backend::VelloCpu);
        assert_eq!(
            choose(&p, Some(Backend::Skia)).unwrap().reason,
            Reason::Build
        );
        let (c, fit) = check(&p, choose(&p, None).unwrap());
        assert_eq!((c.backend, fit), (Backend::VelloCpu, Fit::Fits));
        assert_eq!(
            plan(&p, None, &Backend::ALL).unwrap().uses,
            Backend::VelloCpu
        );
        assert_eq!(
            plan(&p, None, &[Backend::Integer]).unwrap().uses,
            Backend::Integer
        );
        assert!(plan(&device(Some(hw), false), None, &Backend::ALL).is_none());
    }
}
