//! A desktop window that shows a [`Session`] and turns the mouse into touch.

use hibari_os_core::{Phase, Point, TouchEvent};
use hibari_os_hal::Clock;
use minifb::{Key, MouseButton, MouseMode, Window, WindowOptions};

use crate::{HostClock, Session};

/// Opens a window for `session` and runs until it is closed or Escape is pressed.
///
/// # Errors
/// Returns the window system's error when the window cannot be created or updated.
pub fn run(session: &mut Session) -> Result<(), minifb::Error> {
    let p = session.profile();
    let (w, h) = (usize::from(p.width), usize::from(p.height));
    let title = format!("Hibari OS - {}", p.name);
    let mut window = Window::new(&title, w, h, WindowOptions::default())?;
    window.set_target_fps(60);
    let clock = HostClock::new();
    let mut down = false;
    let mut last = Point::default();
    let mut buffer = vec![0u32; w * h];

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let now = clock.now_ms();
        let pressed = window.get_mouse_down(MouseButton::Left);
        if let Some((x, y)) = window.get_mouse_pos(MouseMode::Clamp) {
            let point = Point {
                x: x as i16,
                y: y as i16,
            };
            let phase = match (down, pressed) {
                (false, true) => Some(Phase::Down),
                (true, true) if point != last => Some(Phase::Move),
                (true, false) => Some(Phase::Up),
                _ => None,
            };
            if let Some(phase) = phase {
                session.touch(TouchEvent {
                    phase,
                    point,
                    t_ms: now,
                });
            }
            last = point;
        }
        down = pressed;
        session.tick(now);

        for (out, &px) in buffer.iter_mut().zip(session.display().pixels()) {
            let (r, g, b) = (
                u32::from(px >> 11 & 0x1f),
                u32::from(px >> 5 & 0x3f),
                u32::from(px & 0x1f),
            );
            *out = (r << 3 | r >> 2) << 16 | (g << 2 | g >> 4) << 8 | (b << 3 | b >> 2);
        }
        window.update_with_buffer(&buffer, w, h)?;
    }
    Ok(())
}
