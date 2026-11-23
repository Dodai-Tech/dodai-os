//! A framebuffer over a caller-provided `&mut [u16]` (RGB565), so a board can place it in PSRAM
//! and a test can place it on the stack. It is a draw target for the shell and tracks which rows
//! changed, so only those need to go to the display.

use embedded_graphics::{
    Pixel,
    geometry::{OriginDimensions, Size},
    pixelcolor::{IntoStorage, Rgb565},
    prelude::{DrawTarget, Point},
    primitives::Rectangle,
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

    fn fill_solid(&mut self, area: &Rectangle, color: Rgb565) -> Result<(), Self::Error> {
        let area = area.intersection(&Rectangle::new(Point::zero(), self.size()));
        let Some(bottom_right) = area.bottom_right() else {
            return Ok(());
        };
        let w = usize::from(self.width);
        let (x0, x1) = (area.top_left.x as usize, bottom_right.x as usize + 1);
        let (y0, y1) = (area.top_left.y as usize, bottom_right.y as usize + 1);
        let value = color.into_storage();
        for row in y0..y1 {
            self.pixels[row * w + x0..row * w + x1].fill(value);
        }
        self.touch_rows(y0 as u16, y1 as u16);
        Ok(())
    }

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
    fn fill_solid_clips_to_the_buffer_and_marks_only_its_rows() {
        let mut buf = [0u16; 20];
        let mut fb = FrameBuf::new(4, 5, &mut buf).unwrap();
        let area = Rectangle::new(Point::new(-3, 2), Size::new(6, 10));
        fb.fill_solid(&area, Rgb565::GREEN).unwrap();
        let green = Rgb565::GREEN.into_storage();
        assert_eq!(fb.pixels().iter().filter(|&&p| p == green).count(), 3 * 3);
        assert_eq!(fb.pixels()[2 * 4], green);
        assert_eq!(fb.pixels()[2 * 4 + 3], 0, "outside the area on that row");
        assert_eq!(fb.pixels()[4], 0, "rows above are untouched");
        let (first, rows) = fb.take_dirty().unwrap();
        assert_eq!((first, rows.len()), (2, 3 * 4));
        let off = Rectangle::new(Point::new(10, 10), Size::new(2, 2));
        fb.fill_solid(&off, Rgb565::RED).unwrap();
        assert!(fb.take_dirty().is_none(), "a fill outside changes nothing");
    }

    #[test]
    fn zero_sized_buffer_is_fine() {
        let mut buf: [u16; 0] = [];
        let mut fb = FrameBuf::new(0, 0, &mut buf).unwrap();
        fb.clear(Rgb565::BLACK).unwrap();
        assert!(fb.take_dirty().is_none());
    }
}
