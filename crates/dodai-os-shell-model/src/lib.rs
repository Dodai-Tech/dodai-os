//! Shell logic with no UI toolkit: which apps the launcher shows, paging, hit testing and
//! navigation. A view (Slint, a Wayland panel, a terminal) draws it and feeds it gestures, so the
//! same logic serves Dodai OS and Dodai DE.
#![cfg_attr(not(test), no_std)]

use dodai_os_core::{AppInfo, CapabilitySet, Direction, Gesture, Point, runnable};

/// The paged app grid.
#[derive(Clone, Copy, Debug)]
pub struct Launcher<'a> {
    board: CapabilitySet,
    apps: &'a [AppInfo],
    cols: u8,
    rows: u8,
    page: usize,
}

impl<'a> Launcher<'a> {
    /// A launcher for `board`, listing the `apps` it can run in a `cols` x `rows` grid.
    #[must_use]
    pub fn new(board: CapabilitySet, apps: &'a [AppInfo], cols: u8, rows: u8) -> Self {
        Self {
            board,
            apps,
            cols: cols.max(1),
            rows: rows.max(1),
            page: 0,
        }
    }

    fn per_page(&self) -> usize {
        usize::from(self.cols) * usize::from(self.rows)
    }

    /// Number of apps the board can run.
    #[must_use]
    pub fn app_count(&self) -> usize {
        runnable(self.board, self.apps).count()
    }

    /// Number of pages (at least one).
    #[must_use]
    pub fn page_count(&self) -> usize {
        self.app_count().div_ceil(self.per_page()).max(1)
    }

    /// The current page, from 0.
    #[must_use]
    pub const fn page(&self) -> usize {
        self.page
    }

    /// The apps on the current page, row by row.
    pub fn page_apps(&self) -> impl Iterator<Item = &'a AppInfo> {
        self.apps_on(self.page)
    }

    /// The apps on `page`, row by row (empty past the last page); used to draw two pages while one
    /// slides out and the other in.
    pub fn apps_on(&self, page: usize) -> impl Iterator<Item = &'a AppInfo> {
        let per = self.per_page();
        runnable(self.board, self.apps)
            .skip(page.saturating_mul(per))
            .take(per)
    }

    /// Moves a page for a swipe (left or up: next, right or down: previous); true when the page changed.
    pub fn swipe(&mut self, dir: Direction) -> bool {
        let next = match dir {
            Direction::Left | Direction::Up => self.page + 1,
            Direction::Right | Direction::Down => match self.page.checked_sub(1) {
                Some(p) => p,
                None => return false,
            },
        };
        if next >= self.page_count() {
            return false;
        }
        self.page = next;
        true
    }

    /// The app under `at` on a screen of `size` (`width`, `height`), if any.
    #[must_use]
    pub fn hit(&self, at: Point, size: (u16, u16)) -> Option<&'a AppInfo> {
        let (w, h) = (u32::from(size.0), u32::from(size.1));
        let (x, y) = (u32::try_from(at.x).ok()?, u32::try_from(at.y).ok()?);
        if x >= w || y >= h {
            return None;
        }
        let col = (x * u32::from(self.cols) / w) as usize;
        let row = (y * u32::from(self.rows) / h) as usize;
        let idx = self.page * self.per_page() + row * usize::from(self.cols) + col;
        runnable(self.board, self.apps).nth(idx)
    }
}

/// What is on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen<'a> {
    /// The launcher grid.
    Launcher,
    /// An app in the foreground.
    App(&'a AppInfo),
}

/// What the view must do after a gesture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Output<'a> {
    /// Nothing changed.
    None,
    /// Draw again.
    Redraw,
    /// Start this app.
    Launch(&'a AppInfo),
}

/// The shell: the launcher plus navigation between it and one foreground app.
#[derive(Clone, Copy, Debug)]
pub struct Shell<'a> {
    launcher: Launcher<'a>,
    screen: Screen<'a>,
    size: (u16, u16),
}

impl<'a> Shell<'a> {
    /// A shell on a screen of `size`, showing the launcher.
    #[must_use]
    pub const fn new(launcher: Launcher<'a>, size: (u16, u16)) -> Self {
        Self {
            launcher,
            screen: Screen::Launcher,
            size,
        }
    }

    /// What is on screen.
    #[must_use]
    pub const fn screen(&self) -> Screen<'a> {
        self.screen
    }

    /// The launcher, to draw it.
    #[must_use]
    pub const fn launcher(&self) -> &Launcher<'a> {
        &self.launcher
    }

    /// Handles a gesture. In an app it returns `None`: the caller passes the gesture to the app.
    pub fn on_gesture(&mut self, gesture: Gesture) -> Output<'a> {
        if self.screen != Screen::Launcher {
            return Output::None;
        }
        match gesture {
            Gesture::Swipe(dir) if self.launcher.swipe(dir) => Output::Redraw,
            Gesture::Tap(p) => match self.launcher.hit(p, self.size) {
                Some(app) => {
                    self.screen = Screen::App(app);
                    Output::Launch(app)
                }
                None => Output::None,
            },
            _ => Output::None,
        }
    }

    /// Back to the launcher (the app quit or the user went back).
    pub fn back(&mut self) -> Output<'a> {
        if self.screen == Screen::Launcher {
            return Output::None;
        }
        self.screen = Screen::Launcher;
        Output::Redraw
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dodai_os_core::Capability::{Ble, Display, Touch};

    const fn app(id: &'static str, requires: CapabilitySet) -> AppInfo {
        AppInfo {
            id,
            name: id,
            requires,
        }
    }
    const SHOW: CapabilitySet = CapabilitySet::of(&[Display]);
    const APPS: [AppInfo; 6] = [
        app("a", SHOW),
        app("b", SHOW),
        app("ble", CapabilitySet::of(&[Display, Ble])),
        app("c", SHOW),
        app("d", SHOW),
        app("e", SHOW),
    ];
    const BOARD: CapabilitySet = CapabilitySet::of(&[Display, Touch]);

    fn shell() -> Shell<'static> {
        Shell::new(Launcher::new(BOARD, &APPS, 2, 1), (200, 100))
    }
    const fn tap(x: i16, y: i16) -> Gesture {
        Gesture::Tap(Point { x, y })
    }
    fn ids<'a>(it: impl Iterator<Item = &'a AppInfo>) -> Vec<&'a str> {
        it.map(|a| a.id).collect()
    }

    #[test]
    fn hides_apps_the_board_cannot_run_and_pages() {
        let l = Launcher::new(BOARD, &APPS, 2, 1);
        assert_eq!(l.app_count(), 5);
        assert_eq!(l.page_count(), 3);
        assert_eq!(ids(l.page_apps()), ["a", "b"]);
    }

    #[test]
    fn swipes_change_pages_within_bounds() {
        let mut l = Launcher::new(BOARD, &APPS, 2, 1);
        assert!(!l.swipe(Direction::Right));
        assert!(l.swipe(Direction::Left));
        assert_eq!(ids(l.page_apps()), ["c", "d"]);
        assert!(l.swipe(Direction::Left));
        assert!(!l.swipe(Direction::Left));
        assert!(!l.swipe(Direction::Up), "no page after the last");
        assert!(l.swipe(Direction::Right));
        assert_eq!(l.page(), 1);
        assert!(l.swipe(Direction::Down), "down goes back like right");
        assert!(!l.swipe(Direction::Down));
        assert!(l.swipe(Direction::Up), "up goes forward like left");
        assert_eq!(l.page(), 1);
    }

    #[test]
    fn tap_hits_the_cell_under_the_finger() {
        let l = Launcher::new(BOARD, &APPS, 2, 1);
        assert_eq!(
            l.hit(Point { x: 10, y: 50 }, (200, 100)).map(|a| a.id),
            Some("a")
        );
        assert_eq!(
            l.hit(Point { x: 150, y: 50 }, (200, 100)).map(|a| a.id),
            Some("b")
        );
        assert_eq!(l.hit(Point { x: 200, y: 50 }, (200, 100)), None);
        assert_eq!(l.hit(Point { x: -1, y: 0 }, (200, 100)), None);
        assert_eq!(l.hit(Point { x: 0, y: 0 }, (0, 0)), None);
    }

    #[test]
    fn empty_cells_hit_nothing() {
        let mut l = Launcher::new(BOARD, &APPS, 2, 1);
        l.swipe(Direction::Left);
        l.swipe(Direction::Left);
        assert_eq!(ids(l.page_apps()), ["e"]);
        assert_eq!(l.hit(Point { x: 150, y: 10 }, (200, 100)), None);
    }

    #[test]
    fn shell_launches_and_goes_back() {
        let mut s = shell();
        let out = s.on_gesture(tap(150, 10));
        assert!(matches!(out, Output::Launch(a) if a.id == "b"));
        assert!(matches!(s.screen(), Screen::App(a) if a.id == "b"));
        assert_eq!(s.on_gesture(tap(10, 10)), Output::None);
        assert_eq!(s.back(), Output::Redraw);
        assert_eq!(s.back(), Output::None);
    }

    #[test]
    fn shell_pages_on_swipe() {
        let mut s = shell();
        assert_eq!(
            s.on_gesture(Gesture::Swipe(Direction::Left)),
            Output::Redraw
        );
        assert_eq!(s.launcher().page(), 1);
        assert_eq!(
            s.on_gesture(Gesture::LongPress(Point::default())),
            Output::None
        );
    }

    #[test]
    fn apps_on_any_page_including_out_of_range() {
        let l = Launcher::new(BOARD, &APPS, 2, 1);
        assert_eq!(ids(l.apps_on(1)), ["c", "d"]);
        assert_eq!(ids(l.apps_on(2)), ["e"]);
        assert_eq!(ids(l.apps_on(3)), Vec::<&str>::new());
        assert_eq!(ids(l.apps_on(usize::MAX)), Vec::<&str>::new());
    }

    #[test]
    fn empty_grid_sizes_are_clamped() {
        let l = Launcher::new(BOARD, &APPS, 0, 0);
        assert_eq!(l.page_count(), 5);
    }
}
