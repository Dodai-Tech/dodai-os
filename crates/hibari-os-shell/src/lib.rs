//! The Hibari OS shell view: a status bar, the app grid and a foreground-app page, drawn through
//! Fusuma's `Renderer` trait. The same code draws on a microcontroller display (integer backend),
//! in a host window and into a PNG (anti-aliased backend). Logic (paging, hit testing, navigation)
//! lives in `hibari-os-shell-model`; this crate only lays out and paints it.
#![cfg_attr(not(test), no_std)]

use core::fmt::Write;

pub mod framebuf;
pub use framebuf::FrameBuf;

use fusuma_render::{Align, Color, Point, Rect, Renderer, TextStyle};
use hibari_os_core::anim::{Easing, Fx, Tween};
use hibari_os_core::{AppInfo, CapabilitySet, Gesture, Point as TouchPoint};
use hibari_os_shell_model::{Launcher, Output, Screen, Shell};

/// Colors of the shell. Design tokens will fill these in.
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

/// How long a slide takes, in milliseconds.
pub const SLIDE_MS: u32 = 220;

/// What moves during a transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Motion<'a> {
    /// The launcher changes page: the old one slides out, the new one in.
    Page { from: usize, to: usize },
    /// An app slides in over the launcher.
    Open,
    /// The app slides out to reveal the launcher.
    Close(&'a AppInfo),
}

/// A transition in progress.
#[derive(Clone, Copy, Debug)]
struct Anim<'a> {
    motion: Motion<'a>,
    tween: Tween,
}

/// `from` to `to` at progress `p` (0 to 1), in whole pixels.
fn slide(from: i32, to: i32, p: Fx) -> i32 {
    Fx::from_int(from).lerp(Fx::from_int(to), p).round()
}

/// The whole shell on a screen of a given size.
#[derive(Clone, Copy, Debug)]
pub struct Ui<'a> {
    anim: Option<Anim<'a>>,
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
            anim: None,
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

    /// Handles a gesture in screen coordinates at time `now_ms` (monotonic milliseconds). A tap on
    /// the status bar goes back from an app. A change of page or of app starts a slide.
    pub fn on_gesture(&mut self, gesture: Gesture, now_ms: u32) -> Output<'a> {
        let was_in = match self.shell.screen() {
            Screen::App(app) => Some(app),
            Screen::Launcher => None,
        };
        let page = self.shell.launcher().page();
        let bar = i16::try_from(self.bar).unwrap_or(i16::MAX);
        let out = match gesture {
            Gesture::Tap(p) if p.y < bar => self.shell.back(),
            Gesture::Tap(p) => {
                let y = p.y.saturating_sub(bar);
                self.shell
                    .on_gesture(Gesture::Tap(TouchPoint { x: p.x, y }))
            }
            other => self.shell.on_gesture(other),
        };
        let tween = Tween::new(Fx::ZERO, Fx::ONE, now_ms, SLIDE_MS, Easing::CubicOut);
        let motion = match (out, was_in) {
            (Output::Launch(_), _) => Some(Motion::Open),
            (Output::Redraw, Some(app)) => Some(Motion::Close(app)),
            (Output::Redraw, None) => {
                let to = self.shell.launcher().page();
                (to != page).then_some(Motion::Page { from: page, to })
            }
            _ => None,
        };
        if let Some(motion) = motion {
            self.anim = Some(Anim { motion, tween });
        }
        out
    }

    /// Whether a transition is running (or has one last frame to show).
    #[must_use]
    pub const fn is_animating(&self) -> bool {
        self.anim.is_some()
    }

    /// Advances time. Returns true while a frame must be drawn: during a slide, and once more after
    /// it ends so the final picture is the settled one.
    pub fn tick(&mut self, now_ms: u32) -> bool {
        let Some(anim) = self.anim else { return false };
        if anim.tween.is_done(now_ms) {
            self.anim = None;
        }
        true
    }

    /// Eased progress of the running transition, 0 to 1 (1 when there is none).
    fn progress(&self, now_ms: u32) -> Fx {
        self.anim.map_or(Fx::ONE, |a| a.tween.value(now_ms))
    }

    /// Paints the screen as it looks at `now_ms`: settled, or part-way through a slide.
    ///
    /// # Errors
    /// Returns the renderer's error when drawing fails.
    pub fn draw<R: Renderer>(&self, r: &mut R, now_ms: u32) -> Result<(), R::Error> {
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
            Screen::Launcher => write!(title, "Hibari"),
            Screen::App(app) => write!(title, "< {}", app.name),
        };
        r.draw_text(title.as_str(), Point::new(MARGIN, (bar - TEXT) / 2), &text)?;

        let grid = (w, h - bar);
        let (p, page) = (self.progress(now_ms), self.shell.launcher().page());
        match (self.shell.screen(), self.anim.map(|a| a.motion)) {
            (Screen::Launcher, Some(Motion::Page { from, to })) => {
                let (dir, shift) = (if to > from { 1 } else { -1 }, slide(0, w, p));
                self.in_layer(r, -dir * shift, |r| self.draw_page(r, &text, grid, from))?;
                self.in_layer(r, dir * (w - shift), |r| self.draw_page(r, &text, grid, to))?;
            }
            (Screen::Launcher, Some(Motion::Close(app))) => {
                self.in_layer(r, 0, |r| self.draw_page(r, &text, grid, page))?;
                self.in_layer(r, slide(0, w, p), |r| self.draw_app(r, &text, app, (w, h)))?;
            }
            (Screen::App(app), Some(Motion::Open)) => {
                self.in_layer(r, 0, |r| self.draw_page(r, &text, grid, page))?;
                self.in_layer(r, slide(w, 0, p), |r| self.draw_app(r, &text, app, (w, h)))?;
            }
            (Screen::Launcher, _) => self.draw_page(r, &text, grid, page)?,
            (Screen::App(app), _) => self.draw_app(r, &text, app, (w, h))?,
        }
        if matches!(self.shell.screen(), Screen::Launcher) {
            self.draw_counter(r, &text)?;
        }
        Ok(())
    }

    /// Runs `draw` limited to the area below the status bar and moved `dx` pixels sideways.
    fn in_layer<R: Renderer>(
        &self,
        r: &mut R,
        dx: i32,
        draw: impl FnOnce(&mut R) -> Result<(), R::Error>,
    ) -> Result<(), R::Error> {
        r.save()?;
        let below = Rect::new(
            0,
            i32::from(self.bar),
            u32::from(self.size.0),
            u32::from(self.size.1.saturating_sub(self.bar)),
        );
        let result = r
            .clip(below)
            .and_then(|()| r.translate(dx, 0))
            .and_then(|()| draw(r));
        r.restore()?;
        result
    }

    /// The page of an app: its name, centered, on the background.
    fn draw_app<R: Renderer>(
        &self,
        r: &mut R,
        text: &TextStyle,
        app: &AppInfo,
        (w, h): (i32, i32),
    ) -> Result<(), R::Error> {
        let bar = i32::from(self.bar);
        r.fill_rect(
            Rect::new(0, bar, w.max(0) as u32, (h - bar).max(0) as u32),
            0,
            self.theme.background,
        )?;
        r.draw_text(
            app.name,
            Point::new(w / 2, h / 2 - BASELINE),
            &text.aligned(Align::Center),
        )
    }

    /// The "page / pages" counter in the status bar.
    fn draw_counter<R: Renderer>(&self, r: &mut R, text: &TextStyle) -> Result<(), R::Error> {
        let launcher = self.shell.launcher();
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

    /// One page of the app grid (the background is cleared by the caller).
    fn draw_page<R: Renderer>(
        &self,
        r: &mut R,
        text: &TextStyle,
        (w, h): (i32, i32),
        page: usize,
    ) -> Result<(), R::Error> {
        let t = self.theme;
        let centered = text.aligned(Align::Center);
        let launcher = self.shell.launcher();
        let (cols, rows) = (i32::from(self.cols), i32::from(self.rows));
        let top = i32::from(self.bar);

        for (i, app) in launcher.apps_on(page).enumerate() {
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

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fusuma_render::Probe;
    use fusuma_render_embedded::{Embedded, Frame};
    use hibari_os_core::{
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
        ui().draw(&mut s, 10_000).unwrap();
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
        let out = u.on_gesture(tap(300, 36 + 20), 0);
        assert!(matches!(out, Output::Launch(a) if a.name == "Clock"));
        assert!(matches!(u.shell().screen(), Screen::App(a) if a.name == "Clock"));
        let mut s = screen(410, 502);
        u.draw(&mut s, 10_000).unwrap();
        let white = on_panel(Theme::DARK.text);
        let text = (180..230).any(|x| (230..255).any(|y| at(&s, x, y) == white));
        assert!(text, "app page shows the app name");
        assert_eq!(u.on_gesture(tap(10, 10), 0), Output::Redraw);
        assert!(matches!(u.shell().screen(), Screen::Launcher));
    }

    #[test]
    fn tap_on_the_bar_in_the_launcher_does_nothing_and_swipes_page() {
        let mut u = ui();
        assert_eq!(u.on_gesture(tap(10, 10), 0), Output::None);
        assert_eq!(
            u.on_gesture(Gesture::Swipe(Direction::Left), 0),
            Output::Redraw
        );
        let mut s = screen(410, 502);
        u.draw(&mut s, 10_000).unwrap();
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
            u.draw(&mut s, 10_000).unwrap();
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
        u.draw(&mut a, 10_000).unwrap();
        let mut b = fusuma_render_vello_cpu::new(410, 502).unwrap();
        u.draw(&mut b, 10_000).unwrap();
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

    /// The rows below the status bar, which is where a slide happens.
    fn below_bar(s: &Panel) -> Vec<u16> {
        s.target().pixels()[36 * 410..].to_vec()
    }

    fn render(u: &Ui<'_>, now: u32) -> Panel {
        let mut s = screen(410, 502);
        u.draw(&mut s, now).unwrap();
        s
    }

    /// The settled picture of a shell that has run `gestures` and then waited.
    fn settled(gestures: &[Gesture]) -> Vec<u16> {
        let mut u = ui();
        for g in gestures {
            u.on_gesture(*g, 0);
        }
        u.tick(10_000);
        below_bar(&render(&u, 10_000))
    }

    const SWIPE_LEFT: Gesture = Gesture::Swipe(Direction::Left);
    const SWIPE_RIGHT: Gesture = Gesture::Swipe(Direction::Right);

    #[test]
    fn a_page_swipe_slides_from_the_old_page_to_the_new_one() {
        let (page0, page1) = (settled(&[]), settled(&[SWIPE_LEFT]));
        assert_ne!(page0, page1);
        let mut u = ui();
        assert_eq!(u.on_gesture(SWIPE_LEFT, 1000), Output::Redraw);
        assert!(u.is_animating());
        assert_eq!(
            below_bar(&render(&u, 1000)),
            page0,
            "the first frame is still the old page"
        );
        let mid = below_bar(&render(&u, 1000 + SLIDE_MS / 4));
        assert!(mid != page0 && mid != page1, "a frame in between is a mix");
        assert_eq!(
            below_bar(&render(&u, 1000 + SLIDE_MS)),
            page1,
            "the last frame is the new page"
        );
    }

    #[test]
    fn a_slide_moves_the_pages_sideways_in_the_right_direction() {
        let old = render(&ui(), 10_000);
        let mut next = ui();
        next.on_gesture(SWIPE_LEFT, 0);
        next.tick(10_000);
        let new = render(&next, 10_000);

        let mut u = ui();
        u.on_gesture(SWIPE_LEFT, 0);
        let now = SLIDE_MS / 4;
        let mid = render(&u, now);
        let shift = slide(0, 410, Easing::CubicOut.apply(Fx::from_ratio(1, 4)));
        assert!(
            (100..300).contains(&shift),
            "the test needs a real mid-slide frame: {shift}"
        );
        for y in 36..502 {
            for x in 0..410 {
                let want = if x < 410 - shift {
                    // The old page has moved left by `shift`: what is at x was at x + shift.
                    at(&old, x + shift, y)
                } else {
                    // The new page comes from the right: what is at x is at x - (410 - shift).
                    at(&new, x - (410 - shift), y)
                };
                assert_eq!(
                    at(&mid, x, y),
                    want,
                    "pixel ({x}, {y}) with a shift of {shift}"
                );
            }
        }
    }

    #[test]
    fn tick_runs_the_transition_to_its_end_and_then_stops() {
        let mut u = ui();
        assert!(!u.tick(0), "nothing runs at first");
        u.on_gesture(SWIPE_LEFT, 100);
        assert!(u.tick(100 + SLIDE_MS / 2), "mid slide: draw a frame");
        assert!(u.is_animating());
        assert!(u.tick(100 + SLIDE_MS), "the final frame is drawn once");
        assert!(!u.is_animating());
        assert!(!u.tick(100 + SLIDE_MS + 10), "then nothing more to draw");
    }

    #[test]
    fn gestures_that_change_nothing_start_no_animation() {
        let mut u = ui();
        assert_eq!(
            u.on_gesture(SWIPE_RIGHT, 0),
            Output::None,
            "no page before the first"
        );
        assert_eq!(
            u.on_gesture(tap(10, 10), 0),
            Output::None,
            "the bar does nothing in the launcher"
        );
        assert_eq!(
            u.on_gesture(Gesture::LongPress(TouchPoint { x: 5, y: 80 }), 0),
            Output::None
        );
        assert!(!u.is_animating());
    }

    #[test]
    fn opening_and_closing_an_app_slide_it_in_and_out() {
        let (launcher, app) = (settled(&[]), settled(&[tap(300, 56)]));
        let mut u = ui();
        u.on_gesture(tap(300, 56), 0);
        assert_eq!(
            below_bar(&render(&u, 0)),
            launcher,
            "opens from the right edge"
        );
        let mid = below_bar(&render(&u, SLIDE_MS / 4));
        assert!(mid != launcher && mid != app);
        assert_eq!(below_bar(&render(&u, SLIDE_MS)), app);
        u.tick(SLIDE_MS);
        u.on_gesture(tap(10, 10), 1000);
        assert_eq!(
            below_bar(&render(&u, 1000)),
            app,
            "closing starts from the app page"
        );
        assert_eq!(below_bar(&render(&u, 1000 + SLIDE_MS)), launcher);
        u.tick(1000 + SLIDE_MS);
        assert!(!u.is_animating());
    }

    #[test]
    fn a_swipe_during_a_slide_restarts_it_and_settles_on_the_right_page() {
        let mut u = ui();
        u.on_gesture(SWIPE_LEFT, 0);
        u.on_gesture(SWIPE_RIGHT, SLIDE_MS / 3);
        assert_eq!(u.shell().launcher().page(), 0);
        u.tick(SLIDE_MS / 3 + SLIDE_MS);
        assert!(!u.is_animating());
        assert_eq!(below_bar(&render(&u, 5000)), settled(&[]));
    }

    #[test]
    fn the_clock_wrapping_or_running_backwards_cannot_break_a_frame() {
        let mut u = ui();
        u.on_gesture(SWIPE_LEFT, u32::MAX - 50);
        for now in [u32::MAX - 50, u32::MAX, 0, 100, SLIDE_MS, 5, u32::MAX / 2] {
            render(&u, now);
        }
    }

    #[test]
    fn a_slide_on_any_screen_size_does_not_panic() {
        for size in [(1, 1), (30, 30), (466, 466), (2000, 2000)] {
            let mut u = Ui::new(BOARD, &APPS, size);
            let mut s = screen(size.0, size.1);
            u.on_gesture(SWIPE_LEFT, 0);
            u.draw(&mut s, SLIDE_MS / 2).unwrap();
            u.on_gesture(tap(size.0 as i16 / 2, size.1 as i16 / 2), 5);
            u.draw(&mut s, SLIDE_MS / 2).unwrap();
        }
    }

    #[test]
    fn the_slide_looks_the_same_on_the_anti_aliased_backend() {
        let mut u = ui();
        u.on_gesture(SWIPE_LEFT, 0);
        let mut a = screen(410, 502);
        u.draw(&mut a, SLIDE_MS / 4).unwrap();
        let mut b = fusuma_render_vello_cpu::new(410, 502).unwrap();
        u.draw(&mut b, SLIDE_MS / 4).unwrap();
        let near = |p: Color, q: Color| {
            p.r.abs_diff(q.r) <= 10 && p.g.abs_diff(q.g) <= 10 && p.b.abs_diff(q.b) <= 10
        };
        let differing = (0..502)
            .flat_map(|y| (0..410).map(move |x| (x, y)))
            .filter(|&(x, y)| !near(a.pixel(x, y).unwrap(), b.pixel(x, y).unwrap()))
            .count();
        assert!(differing < 410 * 502 / 25, "{differing} pixels differ");
    }
}
