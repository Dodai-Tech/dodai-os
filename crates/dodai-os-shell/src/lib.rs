//! The Dodai OS shell view: a status bar, the app grid and a foreground-app page, drawn with
//! `embedded-graphics` into any RGB565 target. The same code draws on a microcontroller display,
//! in a host window and into a PNG. Logic (paging, hit testing, navigation) lives in
//! `dodai-os-shell-model`; this crate only lays out and paints it.
#![cfg_attr(not(test), no_std)]

use core::fmt::Write;

pub mod framebuf;
pub use framebuf::FrameBuf;

use dodai_os_core::{AppInfo, CapabilitySet, Gesture, Point as TouchPoint};
use dodai_os_shell_model::{Launcher, Output, Screen, Shell};
use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Circle, PrimitiveStyle, Rectangle, RoundedRectangle},
    text::{Alignment, Baseline, Text},
};

/// Colors of the shell. Dodai design tokens will fill these in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    /// Screen background.
    pub background: Rgb565,
    /// Status bar and card surface.
    pub surface: Rgb565,
    /// Highlight (icon discs).
    pub accent: Rgb565,
    /// Text.
    pub text: Rgb565,
}

impl Theme {
    /// The default dark theme.
    pub const DARK: Self = Self {
        background: Rgb565::new(1, 2, 2),
        surface: Rgb565::new(4, 9, 5),
        accent: Rgb565::new(3, 40, 20),
        text: Rgb565::new(31, 63, 31),
    };
}

const MARGIN: i32 = 8;

/// A fixed-capacity text buffer, so labels need no allocation.
#[derive(Default)]
struct Label {
    buf: [u8; 24],
    len: usize,
}

impl Write for Label {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let n = s.len().min(self.buf.len() - self.len);
        self.buf[self.len..self.len + n].copy_from_slice(&s.as_bytes()[..n]);
        self.len += n;
        Ok(())
    }
}

impl Label {
    fn as_str(&self) -> &str {
        // Only whole `&str`s are copied, but a cut can split a multi-byte character: show the
        // valid prefix then.
        core::str::from_utf8(&self.buf[..self.len]).unwrap_or_else(|e| {
            // `valid_up_to` is a char boundary, so this cannot fail.
            core::str::from_utf8(&self.buf[..e.valid_up_to()]).unwrap_or("")
        })
    }
}

/// The whole shell on a screen of a given size.
#[derive(Clone, Copy, Debug)]
pub struct Ui<'a> {
    shell: Shell<'a>,
    size: (u16, u16),
    bar: u16,
    cols: u8,
    rows: u8,
    theme: Theme,
}

impl<'a> Ui<'a> {
    /// A shell for `board` showing the runnable `apps` on a `size` screen (`width`, `height`).
    #[must_use]
    pub fn new(board: CapabilitySet, apps: &'a [AppInfo], size: (u16, u16)) -> Self {
        let (w, h) = size;
        let bar: u16 = if h >= 200 { 36 } else { 28 };
        let cols = (w / 140).clamp(1, 6) as u8;
        let rows = (h.saturating_sub(bar) / 140).clamp(1, 4) as u8;
        let grid = (w, h.saturating_sub(bar));
        let shell = Shell::new(Launcher::new(board, apps, cols, rows), grid);
        Self {
            shell,
            size,
            bar,
            cols,
            rows,
            theme: Theme::DARK,
        }
    }

    /// The shell state, to read the current screen.
    #[must_use]
    pub const fn shell(&self) -> &Shell<'a> {
        &self.shell
    }

    /// Grid columns and rows chosen for this screen.
    #[must_use]
    pub const fn grid(&self) -> (u8, u8) {
        (self.cols, self.rows)
    }

    /// Handles a gesture in screen coordinates. A tap on the status bar goes back from an app.
    pub fn on_gesture(&mut self, gesture: Gesture) -> Output<'a> {
        match gesture {
            Gesture::Tap(p) if p.y < i16::try_from(self.bar).unwrap_or(i16::MAX) => {
                self.shell.back()
            }
            Gesture::Tap(p) => {
                let y =
                    p.y.saturating_sub(i16::try_from(self.bar).unwrap_or(i16::MAX));
                self.shell
                    .on_gesture(Gesture::Tap(TouchPoint { x: p.x, y }))
            }
            other => self.shell.on_gesture(other),
        }
    }

    /// Paints the current screen.
    ///
    /// # Errors
    /// Returns the target's error when drawing fails.
    pub fn draw<D: DrawTarget<Color = Rgb565>>(&self, target: &mut D) -> Result<(), D::Error> {
        let t = self.theme;
        target.clear(t.background)?;
        let style = MonoTextStyle::new(&FONT_10X20, t.text);
        let (w, h) = (i32::from(self.size.0), i32::from(self.size.1));
        let bar = i32::from(self.bar);

        Rectangle::new(
            Point::zero(),
            Size::new(u32::from(self.size.0), u32::from(self.bar)),
        )
        .into_styled(PrimitiveStyle::with_fill(t.surface))
        .draw(target)?;
        let mut title = Label::default();
        let _ = match self.shell.screen() {
            Screen::Launcher => write!(title, "Dodai"),
            Screen::App(app) => write!(title, "< {}", app.name),
        };
        Text::with_baseline(
            title.as_str(),
            Point::new(MARGIN, (bar - 20) / 2),
            style,
            Baseline::Top,
        )
        .draw(target)?;

        match self.shell.screen() {
            Screen::App(app) => {
                Text::with_alignment(app.name, Point::new(w / 2, h / 2), style, Alignment::Center)
                    .draw(target)
                    .map(drop)
            }
            Screen::Launcher => self.draw_grid(target, style, (w, h - bar)),
        }
    }

    fn draw_grid<D: DrawTarget<Color = Rgb565>>(
        &self,
        target: &mut D,
        style: MonoTextStyle<'_, Rgb565>,
        (w, h): (i32, i32),
    ) -> Result<(), D::Error> {
        let t = self.theme;
        let launcher = self.shell.launcher();
        let (cols, rows) = (i32::from(self.cols), i32::from(self.rows));
        let top = i32::from(self.bar);

        for (i, app) in launcher.page_apps().enumerate() {
            let i = i as i32;
            let (col, row) = (i % cols, i / cols);
            let (x0, y0) = (col * w / cols, top + row * h / rows);
            let (cw, ch) = ((col + 1) * w / cols - x0, top + (row + 1) * h / rows - y0);
            let card = Rectangle::new(
                Point::new(x0 + MARGIN, y0 + MARGIN),
                Size::new(
                    (cw - 2 * MARGIN).max(1) as u32,
                    (ch - 2 * MARGIN).max(1) as u32,
                ),
            );
            RoundedRectangle::with_equal_corners(card, Size::new(14, 14))
                .into_styled(PrimitiveStyle::with_fill(t.surface))
                .draw(target)?;
            let (cx, cy) = (x0 + cw / 2, y0 + ch / 2);
            Circle::with_center(Point::new(cx, cy - 12), 44)
                .into_styled(PrimitiveStyle::with_fill(t.accent))
                .draw(target)?;
            let mut initial = Label::default();
            let _ = write!(initial, "{}", app.name.chars().next().unwrap_or('?'));
            Text::with_alignment(
                initial.as_str(),
                Point::new(cx, cy - 5),
                style,
                Alignment::Center,
            )
            .draw(target)?;
            Text::with_alignment(app.name, Point::new(cx, cy + 34), style, Alignment::Center)
                .draw(target)?;
        }

        let pages = launcher.page_count();
        if pages > 1 {
            let mut label = Label::default();
            let _ = write!(label, "{}/{}", launcher.page() + 1, pages);
            Text::with_alignment(
                label.as_str(),
                Point::new(i32::from(self.size.0) - MARGIN, i32::from(self.bar) / 2 + 5),
                style,
                Alignment::Right,
            )
            .draw(target)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::convert::Infallible;
    use dodai_os_core::{
        Capability::{Display, Touch},
        Direction,
    };

    const fn app(name: &'static str) -> AppInfo {
        AppInfo {
            id: name,
            name,
            requires: CapabilitySet::of(&[Display]),
        }
    }
    const APPS: [AppInfo; 8] = [
        app("Weather"),
        app("Clock"),
        app("Settings"),
        app("Maps"),
        app("Music"),
        app("Notes"),
        app("Files"),
        app("Games"),
    ];
    const BOARD: CapabilitySet = CapabilitySet::of(&[Display, Touch]);

    /// A target that records pixels in a small buffer.
    struct Grid {
        w: u32,
        px: std::vec::Vec<Rgb565>,
    }
    impl Grid {
        fn new(w: u32, h: u32) -> Self {
            Self {
                w,
                px: std::vec![Rgb565::BLACK; (w * h) as usize],
            }
        }
        fn at(&self, x: u32, y: u32) -> Rgb565 {
            self.px[(y * self.w + x) as usize]
        }
    }
    impl OriginDimensions for Grid {
        fn size(&self) -> Size {
            Size::new(self.w, self.px.len() as u32 / self.w)
        }
    }
    impl DrawTarget for Grid {
        type Color = Rgb565;
        type Error = Infallible;
        fn draw_iter<I: IntoIterator<Item = Pixel<Rgb565>>>(
            &mut self,
            pixels: I,
        ) -> Result<(), Infallible> {
            for Pixel(p, c) in pixels {
                if p.x >= 0
                    && p.y >= 0
                    && (p.x as u32) < self.w
                    && ((p.y as u32) * self.w + p.x as u32) < self.px.len() as u32
                {
                    let i = (p.y as u32 * self.w + p.x as u32) as usize;
                    self.px[i] = c;
                }
            }
            Ok(())
        }
    }

    fn ui() -> Ui<'static> {
        Ui::new(BOARD, &APPS, (410, 502))
    }
    const fn tap(x: i16, y: i16) -> Gesture {
        Gesture::Tap(TouchPoint { x, y })
    }

    #[test]
    fn grid_adapts_to_the_screen() {
        assert_eq!(ui().grid(), (2, 3));
        assert_eq!(Ui::new(BOARD, &APPS, (1024, 600)).grid(), (6, 4));
        assert_eq!(Ui::new(BOARD, &APPS, (100, 100)).grid(), (1, 1));
    }

    #[test]
    fn draws_bar_cards_and_background() {
        let mut g = Grid::new(410, 502);
        ui().draw(&mut g).unwrap();
        assert_eq!(g.at(300, 20), Theme::DARK.surface, "status bar");
        assert_eq!(
            g.at(84, 101),
            Theme::DARK.accent,
            "icon disc of the first app"
        );
        assert_eq!(g.at(2, 495), Theme::DARK.background, "margin");
    }

    #[test]
    fn tap_below_the_bar_launches_the_app_under_the_finger_and_bar_goes_back() {
        let mut u = ui();
        // Second column, first row: grid starts below the 36 px bar.
        let out = u.on_gesture(tap(300, 36 + 20));
        assert!(matches!(out, Output::Launch(a) if a.name == "Clock"));
        assert!(matches!(u.shell().screen(), Screen::App(a) if a.name == "Clock"));
        let mut g = Grid::new(410, 502);
        u.draw(&mut g).unwrap();
        let text = (180..230).any(|x| (230..255).any(|y| g.at(x, y) == Theme::DARK.text));
        assert!(text, "app page shows the app name");
        assert_eq!(u.on_gesture(tap(10, 10)), Output::Redraw);
        assert!(matches!(u.shell().screen(), Screen::Launcher));
    }

    #[test]
    fn tap_on_the_bar_in_the_launcher_does_nothing_and_swipes_page() {
        let mut u = ui();
        assert_eq!(u.on_gesture(tap(10, 10)), Output::None);
        assert_eq!(
            u.on_gesture(Gesture::Swipe(Direction::Left)),
            Output::Redraw
        );
        let mut g = Grid::new(410, 502);
        u.draw(&mut g).unwrap();
    }

    #[test]
    fn long_names_and_tiny_screens_do_not_panic() {
        const LONG: [AppInfo; 1] = [app("A name much longer than the label buffer can hold")];
        for size in [(1, 1), (30, 30), (466, 466), (2000, 2000)] {
            let u = Ui::new(BOARD, &LONG, size);
            let mut g = Grid::new(u32::from(size.0), u32::from(size.1));
            u.draw(&mut g).unwrap();
        }
    }

    #[test]
    fn label_cuts_on_a_char_boundary() {
        let mut l = Label::default();
        let _ = write!(l, "{}", "é".repeat(20));
        assert!(l.as_str().chars().all(|c| c == 'é'));
        assert!(l.as_str().len() <= 24);
    }
}
