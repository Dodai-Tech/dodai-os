//! Hibari OS hosted backend: implements the HAL on a normal desktop OS, so the shell and apps
//! run in development without a board. Device profiles stand in for real devices (size, shape and
//! the capabilities the device would have).

use std::convert::Infallible;
use std::sync::OnceLock;
use std::time::Instant;

use embedded_graphics::{
    Pixel,
    geometry::{OriginDimensions, Size},
    pixelcolor::{IntoStorage, Rgb565},
    prelude::DrawTarget,
};
use fusuma_render::Renderer;
use fusuma_render_embedded::Embedded;
pub use hibari_device::DeviceProfile;
use hibari_device::{Capability, CapabilitySet, Shape};
use hibari_hal::input::{Gesture, GestureRecognizer, Phase, Point, TouchEvent};
use hibari_hal::{Clock, Display, Rect};
use hibari_launcher::{AppInfo, Output, Ui};

#[cfg(feature = "window")]
pub mod window;

/// The device files: the real board, and the simulated devices of `tools/host/devices`.
const DEVICE_FILES: [&str; 5] = [
    include_str!("../../../boards/s3-amoled-206/board.toml"),
    include_str!("../devices/watch-round.toml"),
    include_str!("../devices/p4-7in.toml"),
    include_str!("../devices/headless.toml"),
    include_str!("../devices/desktop.toml"),
];

/// All devices the host can pretend to be: the real board first, then the simulated ones.
///
/// # Panics
/// When a built-in device file is invalid; a test checks that none is.
#[must_use]
pub fn profiles() -> &'static [DeviceProfile] {
    static ALL: OnceLock<Vec<DeviceProfile>> = OnceLock::new();
    ALL.get_or_init(|| {
        DEVICE_FILES
            .iter()
            .map(|text| {
                DeviceProfile::from_toml(text)
                    .unwrap_or_else(|e| panic!("a built-in device file is invalid: {e}"))
            })
            .collect()
    })
}

/// The device called `name` (the `name` in its device file).
#[must_use]
pub fn profile(name: &str) -> Option<&'static DeviceProfile> {
    profiles().iter().find(|p| p.name == name)
}

/// The monotonic host clock.
#[derive(Debug)]
pub struct HostClock(Instant);

impl HostClock {
    /// A clock that starts at zero now.
    #[must_use]
    pub fn new() -> Self {
        Self(Instant::now())
    }
}

impl Default for HostClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for HostClock {
    fn now_ms(&self) -> u32 {
        // Wraps after about 49 days, like the device clocks.
        (self.0.elapsed().as_millis() & u128::from(u32::MAX)) as u32
    }
}

/// A flush that does not fit the screen or has the wrong amount of pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BadFlush;

/// An in-memory display (RGB565); a window or a PNG writer reads `pixels`.
#[derive(Clone, Debug)]
pub struct HostDisplay {
    width: u16,
    height: u16,
    shape: Shape,
    pixels: Vec<u16>,
}

impl HostDisplay {
    /// A black display for `profile`.
    #[must_use]
    pub fn new(profile: &DeviceProfile) -> Self {
        let (width, height) = profile.screen_size();
        Self {
            width,
            height,
            shape: profile.display.map_or(Shape::Rect, |d| d.shape),
            pixels: vec![0; usize::from(width) * usize::from(height)],
        }
    }

    /// The whole framebuffer, row by row.
    #[must_use]
    pub fn pixels(&self) -> &[u16] {
        &self.pixels
    }
}

impl Display for HostDisplay {
    type Error = BadFlush;

    fn size(&self) -> (u16, u16) {
        (self.width, self.height)
    }

    fn shape(&self) -> Shape {
        self.shape
    }

    fn flush(&mut self, area: Rect, pixels: &[u16]) -> Result<(), BadFlush> {
        if !area.fits(self.width, self.height) || pixels.len() != area.area() {
            return Err(BadFlush);
        }
        let (stride, w) = (usize::from(self.width), usize::from(area.w));
        for (row, src) in pixels
            .chunks_exact(w.max(1))
            .enumerate()
            .take(usize::from(area.h))
        {
            let start = (usize::from(area.y) + row) * stride + usize::from(area.x);
            self.pixels[start..start + w].copy_from_slice(src);
        }
        Ok(())
    }
}

impl OriginDimensions for HostDisplay {
    fn size(&self) -> Size {
        Size::new(u32::from(self.width), u32::from(self.height))
    }
}

impl DrawTarget for HostDisplay {
    type Color = Rgb565;
    type Error = Infallible;

    fn draw_iter<I: IntoIterator<Item = Pixel<Rgb565>>>(
        &mut self,
        pixels: I,
    ) -> Result<(), Infallible> {
        let (w, h) = (i32::from(self.width), i32::from(self.height));
        for Pixel(p, color) in pixels {
            if p.x >= 0 && p.y >= 0 && p.x < w && p.y < h {
                self.pixels[(p.y * w + p.x) as usize] = color.into_storage();
            }
        }
        Ok(())
    }
}

/// The apps the host offers while real ones are written. The launcher shows those a profile can run.
pub const DEMO_APPS: [AppInfo; 8] = [
    app("Weather", &[Capability::Display, Capability::Wifi]),
    app("Clock", &[Capability::Display]),
    app("Settings", &[Capability::Display, Capability::Touch]),
    app("Compass", &[Capability::Display, Capability::Imu]),
    app("Battery", &[Capability::Display, Capability::Battery]),
    app("Pairing", &[Capability::Display, Capability::Ble]),
    app(
        "Notes",
        &[Capability::Display, Capability::Touch, Capability::Storage],
    ),
    app("Files", &[Capability::Display, Capability::Storage]),
];

const fn app(name: &'static str, requires: &[Capability]) -> AppInfo {
    AppInfo {
        id: name,
        name,
        requires: CapabilitySet::of(requires),
    }
}

/// A device profile running the shell: display, gesture recognizer and UI.
#[derive(Debug)]
pub struct Session {
    profile: &'static DeviceProfile,
    renderer: Embedded<HostDisplay>,
    ui: Ui<'static>,
    gestures: GestureRecognizer,
    /// The latest time the session has seen, in milliseconds; transitions are drawn at it.
    now_ms: u32,
}

impl Session {
    /// A session for `profile`, drawn once. `None` for a device without a screen.
    #[must_use]
    pub fn new(profile: &'static DeviceProfile) -> Option<Self> {
        if !profile.has_screen() {
            return None;
        }
        let mut session = Self {
            profile,
            renderer: Embedded::new(HostDisplay::new(profile)),
            ui: Ui::new(profile.capabilities, &DEMO_APPS, profile.screen_size()),
            gestures: GestureRecognizer::default(),
            now_ms: 0,
        };
        session.render();
        Some(session)
    }

    /// The device being shown.
    #[must_use]
    pub const fn profile(&self) -> &'static DeviceProfile {
        self.profile
    }

    /// The framebuffer.
    #[must_use]
    pub const fn display(&self) -> &HostDisplay {
        self.renderer.target()
    }

    /// The shell, to inspect which screen is up.
    #[must_use]
    pub const fn ui(&self) -> &Ui<'static> {
        &self.ui
    }

    /// Paints the shell into the framebuffer, with the integer backend that the boards use.
    pub fn render(&mut self) {
        // Drawing into memory cannot fail; the only errors a renderer has are about its state stack,
        // which the shell does not use.
        let _ = self.ui.draw(&mut self.renderer, self.now_ms);
    }

    /// Paints the current screen with any other backend, for example the anti-aliased one.
    ///
    /// # Errors
    /// The renderer's error.
    pub fn draw_with<R: Renderer>(&self, renderer: &mut R) -> Result<(), R::Error> {
        self.ui.draw(renderer, self.now_ms)
    }

    fn apply(&mut self, gesture: Option<Gesture>) -> bool {
        let Some(gesture) = gesture else { return false };
        let changed = self.ui.on_gesture(gesture, self.now_ms) != Output::None;
        if changed {
            self.render();
        }
        changed
    }

    /// Feeds one touch sample (a mouse press, move or release); true when the screen changed.
    pub fn touch(&mut self, event: TouchEvent) -> bool {
        self.now_ms = event.t_ms;
        let gesture = self.gestures.feed(event);
        self.apply(gesture)
    }

    /// Lets time pass: a held touch becomes a long press and a running slide advances. True when
    /// the screen changed.
    pub fn tick(&mut self, now_ms: u32) -> bool {
        self.now_ms = now_ms;
        let gesture = self.gestures.tick(now_ms);
        let mut changed = self.apply(gesture);
        if self.ui.tick(now_ms) {
            self.render();
            changed = true;
        }
        changed
    }

    /// Moves the clock on by `ms` and runs [`tick`](Self::tick), as a window does each frame.
    pub fn advance(&mut self, ms: u32) -> bool {
        self.tick(self.now_ms.wrapping_add(ms))
    }

    /// A quick tap at `(x, y)` at the current time, as a test or a screenshot script would do.
    pub fn tap(&mut self, x: i16, y: i16) -> bool {
        let point = Point { x, y };
        let t = self.now_ms;
        self.touch(TouchEvent {
            phase: Phase::Down,
            point,
            t_ms: t,
        });
        self.touch(TouchEvent {
            phase: Phase::Up,
            point,
            t_ms: t.wrapping_add(50),
        })
    }
}

/// The framebuffer as 8-bit RGB, three bytes per pixel.
#[must_use]
pub fn rgb888(display: &HostDisplay) -> Vec<u8> {
    display
        .pixels()
        .iter()
        .flat_map(|&p| {
            let (r, g, b) = ((p >> 11) & 0x1f, (p >> 5) & 0x3f, p & 0x1f);
            [
                (r << 3 | r >> 2) as u8,
                (g << 2 | g >> 4) as u8,
                (b << 3 | b >> 2) as u8,
            ]
        })
        .collect()
}

/// The framebuffer as a PNG file.
///
/// # Errors
/// Returns the encoder error if writing fails.
pub fn png_bytes(display: &HostDisplay) -> Result<Vec<u8>, png::EncodingError> {
    let (w, h) = Display::size(display);
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, u32::from(w), u32::from(h));
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&rgb888(display))?;
    Ok(out)
}

/// The current screen drawn with the anti-aliased backend (`fusuma-render-vello-cpu`), as a PNG file. The
/// shell code is the same as on the boards; only the renderer differs.
///
/// # Errors
/// A message when the surface cannot exist or the encoder fails.
pub fn png_vello_cpu(session: &Session) -> Result<Vec<u8>, String> {
    let p = session.profile();
    let mut vello =
        fusuma_render_vello_cpu::new(u32::from(p.screen_size().0), u32::from(p.screen_size().1))
            .ok_or_else(|| "the device has no screen".to_owned())?;
    session
        .draw_with(&mut vello)
        .map_err(|e| format!("{e:?}"))?;
    fusuma_render_vello_cpu::png(&vello).map_err(|e| e.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hibari_launcher::SLIDE_MS;

    #[test]
    fn every_built_in_device_file_loads_and_names_are_unique() {
        let all = profiles();
        assert_eq!(all.len(), DEVICE_FILES.len());
        for (i, p) in all.iter().enumerate() {
            assert!(
                all[i + 1..].iter().all(|q| q.name != p.name),
                "{} twice",
                p.name
            );
            assert_eq!(
                p.has_screen(),
                p.capabilities.contains(Capability::Display),
                "{}",
                p.name
            );
        }
        assert_eq!(
            profile("p4-7in").map(DeviceProfile::screen_size),
            Some((1024, 600))
        );
        assert_eq!(profile("nope"), None);
    }

    #[test]
    fn the_real_board_comes_from_its_own_device_file() {
        let board = profile("waveshare-s3-amoled-2.06").expect("the S3 board is built in");
        assert_eq!(board.screen_size(), (410, 502));
        assert_eq!(board.display.map(|d| d.offset), Some((22, 0)));
        assert!(
            board.firmware.is_some(),
            "a real board has firmware settings"
        );
        assert!(
            profiles().iter().filter(|p| p.firmware.is_some()).count() == 1,
            "simulated devices have none"
        );
    }

    #[test]
    fn clock_is_monotonic() {
        let c = HostClock::new();
        let a = c.now_ms();
        assert!(c.now_ms() >= a);
    }

    #[test]
    fn display_flush_copies_rows_and_rejects_bad_areas() {
        let mut d = HostDisplay::new(profile("watch-round").unwrap());
        assert_eq!(Display::size(&d), (466, 466));
        assert_eq!(Display::shape(&d), Shape::Round);
        let area = Rect {
            x: 1,
            y: 2,
            w: 2,
            h: 2,
        };
        assert_eq!(d.flush(area, &[1, 2, 3, 4]), Ok(()));
        assert_eq!(d.pixels()[2 * 466 + 1], 1);
        assert_eq!(d.pixels()[2 * 466 + 2], 2);
        assert_eq!(d.pixels()[3 * 466 + 1], 3);
        assert_eq!(d.pixels()[3 * 466 + 2], 4);
        assert_eq!(d.flush(area, &[1, 2, 3]), Err(BadFlush));
        assert_eq!(
            d.flush(
                Rect {
                    x: 465,
                    y: 0,
                    w: 2,
                    h: 1
                },
                &[0, 0]
            ),
            Err(BadFlush)
        );
    }

    /// The rows below the 36 px status bar of a 410 px wide display, where slides happen.
    fn below_bar(s: &Session) -> Vec<u16> {
        s.display().pixels()[36 * 410..].to_vec()
    }

    #[test]
    fn session_launches_an_app_from_a_tap_and_goes_back() {
        let mut s = Session::new(profile("waveshare-s3-amoled-2.06").unwrap()).unwrap();
        let launcher = below_bar(&s);
        assert!(s.tap(300, 36 + 20), "second card: Clock");
        assert!(
            matches!(s.ui().shell().screen(), hibari_launcher::Screen::App(a) if a.name == "Clock")
        );
        assert!(
            below_bar(&s) == launcher,
            "the app has not slid in yet, the launcher is still there"
        );
        s.advance(SLIDE_MS / 2);
        let mid = below_bar(&s);
        assert!(mid != launcher, "the app is part-way in");
        s.advance(SLIDE_MS);
        let open = below_bar(&s);
        assert!(open != launcher && open != mid, "the app has settled");
        assert!(s.tap(10, 10), "status bar goes back");
        s.advance(SLIDE_MS * 2);
        assert!(below_bar(&s) == launcher, "back on the launcher");
        assert!(!s.tap(10, 10), "nothing to go back from");
    }

    #[test]
    fn a_session_draws_slides_at_its_own_clock_and_settles() {
        let mut s = Session::new(profile("waveshare-s3-amoled-2.06").unwrap()).unwrap();
        assert!(!s.advance(1000), "nothing is running");
        s.touch(TouchEvent {
            phase: Phase::Down,
            point: Point { x: 380, y: 200 },
            t_ms: 5000,
        });
        s.touch(TouchEvent {
            phase: Phase::Up,
            point: Point { x: 30, y: 200 },
            t_ms: 5080,
        });
        assert!(s.ui().is_animating(), "a swipe started a slide");
        assert!(s.advance(SLIDE_MS / 3), "a frame is drawn while it runs");
        assert!(s.advance(SLIDE_MS), "and the settled one at the end");
        assert!(!s.ui().is_animating());
        assert!(!s.advance(10), "then nothing more");
    }

    #[test]
    fn launcher_only_lists_apps_the_device_can_run() {
        let watch = Session::new(profile("watch-round").unwrap()).unwrap();
        let names: Vec<_> = watch
            .ui()
            .shell()
            .launcher()
            .page_apps()
            .map(|a| a.name)
            .collect();
        assert!(
            names.contains(&"Compass") && !names.contains(&"Weather"),
            "{names:?}"
        );
    }

    #[test]
    fn long_press_is_swallowed_without_changing_the_screen() {
        let mut s = Session::new(profile("desktop").unwrap()).unwrap();
        let point = Point { x: 50, y: 100 };
        s.touch(TouchEvent {
            phase: Phase::Down,
            point,
            t_ms: 0,
        });
        assert!(!s.tick(600));
        assert!(!s.touch(TouchEvent {
            phase: Phase::Up,
            point,
            t_ms: 700
        }));
    }

    #[test]
    fn headless_has_no_session() {
        assert!(Session::new(profile("headless").unwrap()).is_none());
    }

    #[test]
    fn png_has_signature_and_decodes_to_the_screen_size() {
        let s = Session::new(profile("watch-round").unwrap()).unwrap();
        let bytes = png_bytes(s.display()).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        let decoder = png::Decoder::new(std::io::Cursor::new(&bytes));
        let reader = decoder.read_info().unwrap();
        assert_eq!((reader.info().width, reader.info().height), (466, 466));
    }

    #[test]
    fn the_anti_aliased_png_has_the_screen_size_and_differs_from_the_panel_one() {
        let mut s = Session::new(profile("waveshare-s3-amoled-2.06").unwrap()).unwrap();
        s.tap(300, 56);
        let smooth = png_vello_cpu(&s).unwrap();
        assert_eq!(&smooth[..8], b"\x89PNG\r\n\x1a\n");
        let decoder = png::Decoder::new(std::io::Cursor::new(&smooth));
        let reader = decoder.read_info().unwrap();
        assert_eq!((reader.info().width, reader.info().height), (410, 502));
        assert_ne!(smooth, png_bytes(s.display()).unwrap());
    }

    #[test]
    fn rgb888_expands_565_to_full_range() {
        let mut d = HostDisplay::new(profile("p4-7in").unwrap());
        d.flush(
            Rect {
                x: 0,
                y: 0,
                w: 2,
                h: 1,
            },
            &[0xffff, 0x0000],
        )
        .unwrap();
        assert_eq!(&rgb888(&d)[..6], &[255, 255, 255, 0, 0, 0]);
    }

    #[test]
    fn headless_display_is_empty() {
        let d = HostDisplay::new(profile("headless").unwrap());
        assert!(d.pixels().is_empty());
    }
}
