//! A framebuffer over a caller-provided `&mut [u16]` (RGB565), so a board can place it in PSRAM
//! and a test can place it on the stack. It is a draw target for the shell and tracks which rows
//! changed, so only those need to go to the display.

use embedded_graphics::{
    Pixel,
    geometry::{OriginDimensions, Size},
    pixelcolor::{IntoStorage, Rgb565},
    prelude::DrawTarget,
};

/// An RGB565 framebuffer with row-level change tracking.
#[derive(Debug)]
pub struct FrameBuf<'a> {
    width: u16,
    height: u16,
    pixels: &'a mut [u16],
    /// First and one-past-last changed row, if any.
    dirty: Option<(u16, u16)>,
}

impl<'a> FrameBuf<'a> {
    /// Wraps `pixels`, which must hold exactly `width * height` values.
    ///
    /// Returns `None` when the length does not match.
    #[must_use]
    pub fn new(width: u16, height: u16, pixels: &'a mut [u16]) -> Option<Self> {
        (pixels.len() == usize::from(width) * usize::from(height)).then_some(Self {
            width,
            height,
            pixels,
            dirty: None,
        })
    }

    /// Width in pixels.
    #[must_use]
    pub const fn width(&self) -> u16 {
        self.width
    }

    /// Height in pixels.
    #[must_use]
    pub const fn height(&self) -> u16 {
        self.height
    }

    /// All pixels, row by row.
    #[must_use]
    pub fn pixels(&self) -> &[u16] {
        self.pixels
    }

    /// Marks rows `first..end` as changed.
    fn touch_rows(&mut self, first: u16, end: u16) {
        self.dirty = Some(match self.dirty {
            Some((a, b)) => (a.min(first), b.max(end)),
            None => (first, end),
        });
    }

    /// Takes the changed rows as `(first_row, rows)` and clears the mark.
    pub fn take_dirty(&mut self) -> Option<(u16, &[u16])> {
        let (first, end) = self.dirty.take()?;
        let w = usize::from(self.width);
        Some((
            first,
            &self.pixels[usize::from(first) * w..usize::from(end) * w],
        ))
    }
}

impl OriginDimensions for FrameBuf<'_> {
    fn size(&self) -> Size {
        Size::new(u32::from(self.width), u32::from(self.height))
    }
}

impl DrawTarget for FrameBuf<'_> {
    type Color = Rgb565;
    type Error = core::convert::Infallible;

    fn draw_iter<I: IntoIterator<Item = Pixel<Rgb565>>>(
        &mut self,
        pixels: I,
    ) -> Result<(), Self::Error> {
        let (w, h) = (i32::from(self.width), i32::from(self.height));
        for Pixel(p, color) in pixels {
            if p.x >= 0 && p.y >= 0 && p.x < w && p.y < h {
                self.pixels[(p.y * w + p.x) as usize] = color.into_storage();
                let row = p.y as u16;
                self.touch_rows(row, row + 1);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::prelude::*;

    #[test]
    fn rejects_a_buffer_of_the_wrong_length() {
        let mut small = [0u16; 5];
        assert!(FrameBuf::new(2, 3, &mut small).is_none());
        let mut ok = [0u16; 6];
        assert!(FrameBuf::new(2, 3, &mut ok).is_some());
    }

    #[test]
    fn draws_inside_and_ignores_outside_pixels() {
        let mut buf = [0u16; 12];
        let mut fb = FrameBuf::new(4, 3, &mut buf).unwrap();
        let px = |x, y| Pixel(Point::new(x, y), Rgb565::WHITE);
        fb.draw_iter([px(1, 1), px(-1, 0), px(4, 0), px(0, 3), px(3, 2)])
            .unwrap();
        assert_eq!(fb.pixels()[5], 0xFFFF);
        assert_eq!(fb.pixels()[11], 0xFFFF);
        assert_eq!(fb.pixels().iter().filter(|&&p| p != 0).count(), 2);
    }

    #[test]
    fn tracks_only_the_changed_rows_and_clears_them() {
        let mut buf = [0u16; 20];
        let mut fb = FrameBuf::new(4, 5, &mut buf).unwrap();
        assert!(fb.take_dirty().is_none());
        fb.draw_iter([
            Pixel(Point::new(0, 3), Rgb565::RED),
            Pixel(Point::new(2, 1), Rgb565::RED),
        ])
        .unwrap();
        let (first, rows) = fb.take_dirty().unwrap();
        assert_eq!((first, rows.len()), (1, 3 * 4));
        assert!(fb.take_dirty().is_none());
    }

    #[test]
    fn clear_marks_everything() {
        let mut buf = [0u16; 6];
        let mut fb = FrameBuf::new(2, 3, &mut buf).unwrap();
        fb.clear(Rgb565::BLUE).unwrap();
        let (first, rows) = fb.take_dirty().unwrap();
        assert_eq!((first, rows.len()), (0, 6));
        assert!(
            fb.pixels()
                .iter()
                .all(|&p| p == Rgb565::BLUE.into_storage())
        );
    }

    #[test]
    fn zero_sized_buffer_is_fine() {
        let mut buf: [u16; 0] = [];
        let mut fb = FrameBuf::new(0, 0, &mut buf).unwrap();
        fb.clear(Rgb565::BLACK).unwrap();
        assert!(fb.take_dirty().is_none());
    }
}
