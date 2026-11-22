//! The Dodai OS shell view: a status bar, the app grid and a foreground-app page, drawn through
//! Fusuma's `Renderer` trait. The same code draws on a microcontroller display (integer backend),
//! in a host window and into a PNG (anti-aliased backend). Logic (paging, hit testing, navigation)
//! lives in `dodai-os-shell-model`; this crate only lays out and paints it.
#![cfg_attr(not(test), no_std)]

use core::fmt::Write;

pub mod framebuf;
pub use framebuf::FrameBuf;

use dodai_os_core::{AppInfo, CapabilitySet, Gesture, Point as TouchPoint};
use dodai_os_shell_model::{Launcher, Output, Screen, Shell};
use fusuma_render::{Align, Color, Point, Rect, Renderer, TextStyle};

/// Colors of the shell. Dodai design tokens will fill these in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    /// Screen background.
    pub background: Color,
    /// Status bar and card surface.
    pub surface: Color,
    /// Highlight (icon discs).
    pub accent: Color,
    /// Text.
    pub text: Color,
}

impl Theme {
    /// The default dark theme.
    pub const DARK: Self = Self {
        background: Color::rgb(8, 8, 16),
        surface: Color::rgb(33, 38, 41),
        accent: Color::rgb(24, 162, 165),
        text: Color::WHITE,
    };
}

const MARGIN: i32 = 8;
/// The line height of every text in the shell, in pixels.
const TEXT: i32 = 20;
/// Distance from the top of a 20 px glyph row to its baseline.
const BASELINE: i32 = 15;

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
    /// Returns the renderer's error when drawing fails.
    pub fn draw<R: Renderer>(&self, r: &mut R) -> Result<(), R::Error> {
        let t = self.theme;
        r.clear(t.background)?;
        let text = TextStyle::new(TEXT as u32, t.text);
        let (w, h) = (i32::from(self.size.0), i32::from(self.size.1));
        let bar = i32::from(self.bar);

        r.fill_rect(
            Rect::new(0, 0, u32::from(self.size.0), u32::from(self.bar)),
            0,
            t.surface,
        )?;
        let mut title = Label::default();
        let _ = match self.shell.screen() {
            Screen::Launcher => write!(title, "Dodai"),
            Screen::App(app) => write!(title, "< {}", app.name),
        };
        r.draw_text(title.as_str(), Point::new(MARGIN, (bar - TEXT) / 2), &text)?;

        match self.shell.screen() {
            Screen::App(app) => r.draw_text(
                app.name,
                Point::new(w / 2, h / 2 - BASELINE),
                &text.aligned(Align::Center),
            ),
            Screen::Launcher => self.draw_grid(r, &text, (w, h - bar)),
        }
    }

    fn draw_grid<R: Renderer>(
        &self,
        r: &mut R,
        text: &TextStyle,
        (w, h): (i32, i32),
    ) -> Result<(), R::Error> {
        let t = self.theme;
        let centered = text.aligned(Align::Center);
        let launcher = self.shell.launcher();
        let (cols, rows) = (i32::from(self.cols), i32::from(self.rows));
        let top = i32::from(self.bar);

        for (i, app) in launcher.page_apps().enumerate() {
            let i = i as i32;
            let (col, row) = (i % cols, i / cols);
            let (x0, y0) = (col * w / cols, top + row * h / rows);
            let (cw, ch) = ((col + 1) * w / cols - x0, top + (row + 1) * h / rows - y0);
            let card = Rect::new(
                x0 + MARGIN,
                y0 + MARGIN,
                (cw - 2 * MARGIN).max(1) as u32,
                (ch - 2 * MARGIN).max(1) as u32,
            );
            r.fill_rect(card, 14, t.surface)?;
            let (cx, cy) = (x0 + cw / 2, y0 + ch / 2);
            r.fill_circle(Point::new(cx, cy - 12), 22, t.accent)?;
            let mut initial = Label::default();
            let _ = write!(initial, "{}", app.name.chars().next().unwrap_or('?'));
            r.draw_text(
                initial.as_str(),
                Point::new(cx, cy - 5 - BASELINE),
                &centered,
            )?;
            r.draw_text(app.name, Point::new(cx, cy + 34 - BASELINE), &centered)?;
        }

        let pages = launcher.page_count();
        if pages > 1 {
            let mut label = Label::default();
            let _ = write!(label, "{}/{}", launcher.page() + 1, pages);
            r.draw_text(
                label.as_str(),
                Point::new(
                    i32::from(self.size.0) - MARGIN,
                    i32::from(self.bar) / 2 + 5 - BASELINE,
                ),
                &text.aligned(Align::Right),
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dodai_os_core::{
        Capability::{Display, Touch},
        Direction,
    };
    use fusuma_render::Probe;
    use fusuma_render_embedded::{Embedded, Frame};

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

    type Panel = Embedded<Frame<Vec<u16>>>;

    /// The integer backend on a 16-bit framebuffer, as on the board.
    fn screen(w: u16, h: u16) -> Panel {
        Embedded::new(Frame::new(w, h, vec![0u16; usize::from(w) * usize::from(h)]).unwrap())
    }

    /// What a color looks like on the 16-bit panel.
    fn on_panel(c: Color) -> Color {
        Color::from_rgb565(c.to_rgb565())
    }

    fn at(s: &Panel, x: i32, y: i32) -> Color {
        s.pixel(x, y).unwrap()
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
        let mut s = screen(410, 502);
        ui().draw(&mut s).unwrap();
        let t = Theme::DARK;
        assert_eq!(at(&s, 300, 20), on_panel(t.surface), "status bar");
        assert_eq!(
            at(&s, 84, 101),
            on_panel(t.accent),
            "icon disc of the first app"
        );
        assert_eq!(at(&s, 2, 495), on_panel(t.background), "margin");
        assert_eq!(
            at(&s, 8, 44),
            on_panel(t.background),
            "the card's corner is rounded"
        );
        assert_eq!(at(&s, 60, 60), on_panel(t.surface), "inside the card");
    }

    #[test]
    fn tap_below_the_bar_launches_the_app_under_the_finger_and_bar_goes_back() {
        let mut u = ui();
        // Second column, first row: grid starts below the 36 px bar.
        let out = u.on_gesture(tap(300, 36 + 20));
        assert!(matches!(out, Output::Launch(a) if a.name == "Clock"));
        assert!(matches!(u.shell().screen(), Screen::App(a) if a.name == "Clock"));
        let mut s = screen(410, 502);
        u.draw(&mut s).unwrap();
        let white = on_panel(Theme::DARK.text);
        let text = (180..230).any(|x| (230..255).any(|y| at(&s, x, y) == white));
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
        let mut s = screen(410, 502);
        u.draw(&mut s).unwrap();
        let white = on_panel(Theme::DARK.text);
        let counter = (340..410).any(|x| (8..30).any(|y| at(&s, x, y) == white));
        assert!(counter, "the page counter is in the top bar, not clipped");
    }

    #[test]
    fn long_names_and_tiny_screens_do_not_panic() {
        const LONG: [AppInfo; 1] = [app("A name much longer than the label buffer can hold")];
        for size in [(1, 1), (30, 30), (466, 466), (2000, 2000)] {
            let u = Ui::new(BOARD, &LONG, size);
            let mut s = screen(size.0, size.1);
            u.draw(&mut s).unwrap();
        }
    }

    #[test]
    fn label_cuts_on_a_char_boundary() {
        let mut l = Label::default();
        let _ = write!(l, "{}", "é".repeat(20));
        assert!(l.as_str().chars().all(|c| c == 'é'));
        assert!(l.as_str().len() <= 24);
    }

    #[test]
    fn the_embedded_and_the_anti_aliased_backends_draw_the_same_layout() {
        let u = ui();
        let mut a = screen(410, 502);
        u.draw(&mut a).unwrap();
        let mut b = fusuma_render_skia::Skia::new(410, 502).unwrap();
        u.draw(&mut b).unwrap();
        // Compare the flat areas; rounded corners and discs differ at their edges only.
        let near = |p: Color, q: Color| {
            p.r.abs_diff(q.r) <= 10 && p.g.abs_diff(q.g) <= 10 && p.b.abs_diff(q.b) <= 10
        };
        for (x, y) in [(300, 20), (60, 60), (2, 495), (205, 300), (84, 101)] {
            let (p, q) = (a.pixel(x, y).unwrap(), b.pixel(x, y).unwrap());
            assert!(near(p, q), "({x}, {y}): {p:?} vs {q:?}");
        }
        let differing = (0..502)
            .flat_map(|y| (0..410).map(move |x| (x, y)))
            .filter(|&(x, y)| !near(a.pixel(x, y).unwrap(), b.pixel(x, y).unwrap()))
            .count();
        assert!(
            differing < 410 * 502 / 25,
            "{differing} pixels differ between the backends"
        );
    }
}
