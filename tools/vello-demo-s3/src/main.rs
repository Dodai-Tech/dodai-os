//! `vello_cpu` on the ESP32-S3: an anti-aliased launcher on the AMOLED, with touch.
//!
//! The whole screen is drawn once. A touch presses the card under the finger and only that card is
//! drawn again, in a small render context of its own, so a press costs far less than a full frame.
#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec::Vec;

use esp_backtrace as _;
use esp_hal::{
    clock::CpuClock,
    delay::Delay,
    dma::{DmaRxBuf, DmaTxBuf},
    dma_buffers,
    gpio::{Level, Output, OutputConfig},
    i2c::master::{Config as I2cConfig, I2c},
    spi::{
        Mode as SpiMode,
        master::{Config as SpiConfig, Spi},
    },
    time::{Instant, Rate},
};
use hibari_board_s3_amoled_206::{DISPLAY, LCD_QSPI_MHZ, PROFILE, qspi};
use hibari_drivers::{co5300::Co5300, ft3168::Ft3168};
use hibari_hal::input::Phase;
use hibari_hal::{Clock, Display, Rect, Touch};
use vello_cpu::{
    Pixmap, RenderContext, Resources,
    color::AlphaColor,
    kurbo::{Affine, BezPath, Circle, Rect as KRect, RoundedRect, Shape, Stroke},
};

esp_bootloader_esp_idf::esp_app_desc!();

const BAR: i32 = 36;
const COLS: i32 = 2;
const ROWS: i32 = 3;
const MARGIN: i32 = 8;

/// A colour for each card's disc.
const HUES: [(u8, u8, u8); 6] = [
    (24, 162, 165),
    (240, 120, 70),
    (150, 110, 230),
    (90, 190, 100),
    (235, 190, 50),
    (225, 90, 140),
];

struct BootClock;

impl Clock for BootClock {
    fn now_ms(&self) -> u32 {
        (Instant::now().duration_since_epoch().as_millis() & u64::from(u32::MAX)) as u32
    }
}

fn micros() -> u64 {
    Instant::now().duration_since_epoch().as_micros()
}

fn paint((r, g, b): (u8, u8, u8)) -> AlphaColor<vello_cpu::color::Srgb> {
    AlphaColor::from_rgba8(r, g, b, 255)
}

/// Where card `i` is, in screen pixels: `(x, y, width, height)`.
fn card(w: i32, h: i32, i: i32) -> (i32, i32, i32, i32) {
    let (col, row) = (i % COLS, i / COLS);
    let (x0, y0) = (col * w / COLS, BAR + row * (h - BAR) / ROWS);
    let (cw, ch) = (
        (col + 1) * w / COLS - x0,
        BAR + (row + 1) * (h - BAR) / ROWS - y0,
    );
    (x0 + MARGIN, y0 + MARGIN, cw - 2 * MARGIN, ch - 2 * MARGIN)
}

/// The card under a point, if any.
fn card_at(w: i32, h: i32, x: i32, y: i32) -> Option<i32> {
    (0..COLS * ROWS).find(|&i| {
        let (cx, cy, cw, ch) = card(w, h, i);
        x >= cx && y >= cy && x < cx + cw && y < cy + ch
    })
}

/// Draws one card (the surface behind it is cleared by the caller).
fn draw_card(ctx: &mut RenderContext, w: i32, h: i32, i: i32, pressed: bool) {
    let (x, y, cw, ch) = card(w, h, i);
    let (mut x, mut y, mut cw, mut ch) = (f64::from(x), f64::from(y), f64::from(cw), f64::from(ch));
    if pressed {
        // A pressed card shrinks a little towards its centre.
        let d = 6.0;
        (x, y, cw, ch) = (x + d, y + d, cw - 2.0 * d, ch - 2.0 * d);
    }
    let surface = if pressed { (56, 64, 72) } else { (33, 38, 41) };
    ctx.set_paint(paint(surface));
    ctx.fill_path(&RoundedRect::new(x, y, x + cw, y + ch, 16.0).to_path(0.1));
    let hue = HUES[i as usize % HUES.len()];
    ctx.set_paint(paint(hue));
    ctx.fill_path(&Circle::new((x + cw / 2.0, y + ch / 2.0 - 14.0), 26.0).to_path(0.1));
    // A soft ring round the disc and an outline bar at the bottom: curves that show the anti-aliasing.
    ctx.set_stroke(Stroke::new(2.0));
    ctx.set_paint(paint((235, 238, 245)));
    ctx.stroke_path(&Circle::new((x + cw / 2.0, y + ch / 2.0 - 14.0), 33.0).to_path(0.1));
    ctx.set_paint(paint(hue));
    ctx.stroke_path(
        &RoundedRect::new(x + 14.0, y + ch - 36.0, x + cw - 14.0, y + ch - 14.0, 9.0).to_path(0.1),
    );
}

/// The status bar: a bar with a small ring and a pill, again to show smooth curves.
fn draw_bar(ctx: &mut RenderContext, w: i32) {
    ctx.set_paint(paint((33, 38, 41)));
    ctx.fill_rect(&KRect::new(0.0, 0.0, f64::from(w), f64::from(BAR)));
    ctx.set_paint(paint((24, 162, 165)));
    ctx.fill_path(&Circle::new((20.0, f64::from(BAR) / 2.0), 7.0).to_path(0.1));
    ctx.set_paint(paint((235, 238, 245)));
    ctx.fill_path(&RoundedRect::new(36.0, 11.0, 110.0, 25.0, 7.0).to_path(0.1));
    let mut ring = BezPath::new();
    ring.extend(Circle::new((f64::from(w) - 24.0, f64::from(BAR) / 2.0), 9.0).path_elements(0.1));
    ctx.set_stroke(Stroke::new(3.0));
    ctx.set_paint(paint((235, 190, 50)));
    ctx.stroke_path(&ring);
}

/// The smallest region that covers `(x, y, w, h)` with even edges, inside a `sw` x `sh` screen. The
/// panel controller wants update windows that start and end on even pixels.
fn even_region(x: i32, y: i32, w: i32, h: i32, sw: i32, sh: i32) -> (i32, i32, u16, u16) {
    let (x0, y0) = ((x.max(0)) & !1, (y.max(0)) & !1);
    let (x1, y1) = (
        ((x + w + 1) & !1).min(sw & !1),
        ((y + h + 1) & !1).min(sh & !1),
    );
    (x0, y0, (x1 - x0).max(2) as u16, (y1 - y0).max(2) as u16)
}

/// Renders `scene` into a region of `rw` x `rh` pixels whose top-left is at screen `(ox, oy)`, and
/// sends it to the panel as RGB565.
fn show<D: Display>(
    display: &mut D,
    ox: i32,
    oy: i32,
    rw: u16,
    rh: u16,
    scene: impl FnOnce(&mut RenderContext),
) -> (u64, u64) {
    let t0 = micros();
    let mut ctx = RenderContext::new(rw, rh);
    let mut resources = Resources::new();
    let mut target = Pixmap::new(rw, rh);
    ctx.set_transform(Affine::translate((-f64::from(ox), -f64::from(oy))));
    ctx.set_paint(paint((8, 8, 16)));
    ctx.fill_rect(&KRect::new(
        f64::from(ox),
        f64::from(oy),
        f64::from(ox) + f64::from(rw),
        f64::from(oy) + f64::from(rh),
    ));
    scene(&mut ctx);
    ctx.flush();
    ctx.render(&mut target, &mut resources);
    let t1 = micros();
    let pixels: Vec<u16> = target
        .data_as_u8_slice()
        .chunks_exact(4)
        .map(|p| {
            ((u16::from(p[0]) >> 3) << 11) | ((u16::from(p[1]) >> 2) << 5) | (u16::from(p[2]) >> 3)
        })
        .collect();
    let _ = display.flush(
        Rect {
            x: ox as u16,
            y: oy as u16,
            w: rw,
            h: rh,
        },
        &pixels,
    );
    (t1 - t0, micros() - t1)
}

#[esp_hal::main]
fn main() -> ! {
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::_240MHz));
    esp_alloc::heap_allocator!(size: 72 * 1024);
    esp_alloc::psram_allocator!(peripherals.PSRAM, esp_hal::psram);
    esp_println::logger::init_logger_from_env();
    log::info!("vello_cpu demo on {}", PROFILE.name);

    let delay = Delay::new();
    let (rx_buf, rx_desc, tx_buf, tx_desc) = dma_buffers!(qspi::DMA_CHUNK);
    let (Ok(dma_rx), Ok(dma_tx)) = (
        DmaRxBuf::new(rx_desc, rx_buf),
        DmaTxBuf::new(tx_desc, tx_buf),
    ) else {
        panic!("DMA buffers");
    };
    let spi_config = SpiConfig::default()
        .with_frequency(Rate::from_mhz(LCD_QSPI_MHZ))
        .with_mode(SpiMode::_0);
    let Ok(spi) = Spi::new(peripherals.SPI2, spi_config) else {
        panic!("SPI2")
    };
    let spi = spi
        .with_sck(peripherals.GPIO11)
        .with_sio0(peripherals.GPIO4)
        .with_sio1(peripherals.GPIO5)
        .with_sio2(peripherals.GPIO6)
        .with_sio3(peripherals.GPIO7)
        .with_dma(peripherals.DMA_CH0)
        .with_buffers(dma_rx, dma_tx);
    let cs = Output::new(peripherals.GPIO12, Level::High, OutputConfig::default());
    let reset = Output::new(peripherals.GPIO8, Level::High, OutputConfig::default());
    let mut display = Co5300::new(qspi::Qspi::new(spi, cs), reset, delay, DISPLAY);
    if display.init().is_err() {
        panic!("display init");
    }

    let Ok(i2c) = I2c::new(
        peripherals.I2C0,
        I2cConfig::default().with_frequency(Rate::from_khz(400)),
    ) else {
        panic!("I2C0")
    };
    let i2c = i2c
        .with_sda(peripherals.GPIO15)
        .with_scl(peripherals.GPIO14);
    let mut touch_reset = Output::new(peripherals.GPIO9, Level::High, OutputConfig::default());
    touch_reset.set_low();
    delay.delay_millis(10);
    touch_reset.set_high();
    delay.delay_millis(50);
    let mut touch = Ft3168::new(i2c, BootClock);
    if touch.init().is_err() {
        log::warn!("touch init failed; continuing without it");
    }

    let (w, h) = Display::size(&display);
    let (wi, hi) = (i32::from(w), i32::from(h));
    let (render_us, flush_us) = show(&mut display, 0, 0, w, h, |ctx| {
        draw_bar(ctx, wi);
        for i in 0..COLS * ROWS {
            draw_card(ctx, wi, hi, i, false);
        }
    });
    log::info!(
        "first frame {w}x{h}: render {render_us} us, convert and send {flush_us} us; free heap {} bytes",
        esp_alloc::HEAP.free()
    );

    let mut pressed: Option<i32> = None;
    loop {
        if let Ok(Some(event)) = touch.poll() {
            let (x, y) = (i32::from(event.point.x), i32::from(event.point.y));
            let target = match event.phase {
                Phase::Up => None,
                _ => card_at(wi, hi, x, y),
            };
            if target != pressed {
                for (i, is_pressed) in [(pressed, false), (target, true)] {
                    let Some(i) = i else { continue };
                    let (cx, cy, cw, ch) = card(wi, hi, i);
                    let (rx, ry, rw, rh) = even_region(cx - 2, cy - 2, cw + 4, ch + 4, wi, hi);
                    let (render_us, flush_us) = show(&mut display, rx, ry, rw, rh, |ctx| {
                        draw_card(ctx, wi, hi, i, is_pressed)
                    });
                    log::info!(
                        "card {i} {}: render {render_us} us, send {flush_us} us",
                        if is_pressed { "pressed" } else { "released" }
                    );
                }
                pressed = target;
            }
        }
        delay.delay_millis(10);
    }
}
