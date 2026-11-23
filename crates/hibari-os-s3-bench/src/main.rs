//! Draws the launcher's shapes through Fusuma's integer renderer and through `vello_cpu`, and logs
//! how long each takes on the ESP32-S3.
#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

use esp_backtrace as _;
use esp_hal::{clock::CpuClock, delay::Delay, time::Instant};
use fusuma_render::{Color, Point, Rect, Renderer};
use fusuma_render_embedded::{Embedded, Frame};
use vello_cpu::{
    Pixmap, RenderContext, Resources,
    color::AlphaColor,
    kurbo::{self, Circle, RoundedRect, Shape, Stroke},
};

esp_bootloader_esp_idf::esp_app_desc!();

const BAR: i32 = 36;
const COLS: i32 = 2;
const MARGIN: i32 = 8;
const FRAMES: u32 = 10;

fn micros() -> u64 {
    Instant::now().duration_since_epoch().as_micros()
}

/// The launcher's shapes: a status bar, a grid of rounded cards, each with a disc and an outline.
fn grid(w: i32, h: i32) -> impl Iterator<Item = (i32, i32, i32, i32)> {
    let rows = ((h - BAR) / 140).clamp(1, 4);
    let cols = COLS;
    (0..cols * rows).map(move |i| {
        let (col, row) = (i % cols, i / cols);
        let (x0, y0) = (col * w / cols, BAR + row * (h - BAR) / rows);
        let (cw, ch) = ((col + 1) * w / cols - x0, BAR + (row + 1) * (h - BAR) / rows - y0);
        (x0 + MARGIN, y0 + MARGIN, cw - 2 * MARGIN, ch - 2 * MARGIN)
    })
}

fn scene_integer<R: Renderer>(r: &mut R, w: i32, h: i32) {
    let _ = r.clear(Color::rgb(8, 8, 16));
    let _ = r.fill_rect(Rect::new(0, 0, w as u32, BAR as u32), 0, Color::rgb(33, 38, 41));
    for (x, y, cw, ch) in grid(w, h) {
        let _ = r.fill_rect(Rect::new(x, y, cw as u32, ch as u32), 14, Color::rgb(33, 38, 41));
        let _ = r.fill_circle(Point::new(x + cw / 2, y + ch / 2 - 12), 22, Color::rgb(24, 162, 165));
        let _ = r.stroke_rect(Rect::new(x + 10, y + ch - 34, cw as u32 - 20, 24), 8, 2, Color::rgb(250, 200, 0));
    }
}

fn scene_vello(ctx: &mut RenderContext, w: i32, h: i32) {
    ctx.reset();
    let paint = |c: (u8, u8, u8)| AlphaColor::from_rgba8(c.0, c.1, c.2, 255);
    ctx.set_paint(paint((8, 8, 16)));
    ctx.fill_rect(&kurbo::Rect::new(0.0, 0.0, f64::from(w), f64::from(h)));
    ctx.set_paint(paint((33, 38, 41)));
    ctx.fill_rect(&kurbo::Rect::new(0.0, 0.0, f64::from(w), f64::from(BAR)));
    for (x, y, cw, ch) in grid(w, h) {
        let (x, y, cw, ch) = (f64::from(x), f64::from(y), f64::from(cw), f64::from(ch));
        ctx.set_paint(paint((33, 38, 41)));
        ctx.fill_path(&RoundedRect::new(x, y, x + cw, y + ch, 14.0).to_path(0.1));
        ctx.set_paint(paint((24, 162, 165)));
        ctx.fill_path(&Circle::new((x + cw / 2.0, y + ch / 2.0 - 12.0), 22.0).to_path(0.1));
        ctx.set_paint(paint((250, 200, 0)));
        ctx.set_stroke(Stroke::new(2.0));
        ctx.stroke_path(&RoundedRect::new(x + 11.0, y + ch - 33.0, x + cw - 11.0, y + ch - 11.0, 7.0).to_path(0.1));
    }
}

/// Premultiplied RGBA to RGB565, as a display needs it.
fn to_565(rgba: &[u8], out: &mut [u16]) {
    for (o, p) in out.iter_mut().zip(rgba.chunks_exact(4)) {
        *o = ((u16::from(p[0]) >> 3) << 11) | ((u16::from(p[1]) >> 2) << 5) | (u16::from(p[2]) >> 3);
    }
}

fn average(mut f: impl FnMut()) -> u64 {
    f();
    f();
    let start = micros();
    for _ in 0..FRAMES {
        f();
    }
    (micros() - start) / u64::from(FRAMES)
}

fn run(w: u16, h: u16) {
    let (wi, hi) = (i32::from(w), i32::from(h));
    log::info!("--- {w}x{h} ---");

    let mut screen = Embedded::new(Frame::new(w, h, vec![0u16; usize::from(w) * usize::from(h)]).expect("frame"));
    let t_int = average(|| scene_integer(&mut screen, wi, hi));
    log::info!("integer renderer:  {t_int:>8} us/frame");

    let used_before = esp_alloc::HEAP.used();
    let mut ctx = RenderContext::new(w, h);
    let mut resources = Resources::new();
    let mut target = Pixmap::new(w, h);
    let mut rgb565: Vec<u16> = vec![0; usize::from(w) * usize::from(h)];

    let t_build = average(|| scene_vello(&mut ctx, wi, hi));
    let t_render = average(|| {
        ctx.flush();
        target.data_as_u8_slice_mut().fill(0);
        ctx.render(&mut target, &mut resources);
    });
    let t_convert = average(|| to_565(target.data_as_u8_slice(), &mut rgb565));
    log::info!("vello_cpu build:   {t_build:>8} us/frame");
    log::info!("vello_cpu render:  {t_render:>8} us/frame");
    log::info!("vello_cpu to 565:  {t_convert:>8} us/frame");
    log::info!("vello_cpu total:   {:>8} us/frame", t_build + t_render + t_convert);
    log::info!("memory: vello used {} bytes, heap free {} bytes", esp_alloc::HEAP.used().saturating_sub(used_before), esp_alloc::HEAP.free());

    // The two must show the same picture, within what 16-bit colour allows: compare a few probes.
    let mut agree = 0;
    let probes = [(wi / 4, BAR / 2), (wi / 4, BAR + 30), (wi / 2, hi - 4), (wi / 4 + 3, BAR + 60)];
    for (x, y) in probes {
        let (x, y) = (x.clamp(0, wi - 1) as usize, y.clamp(0, hi - 1) as usize);
        let a = screen.target().pixels()[y * usize::from(w) + x];
        let b = rgb565[y * usize::from(w) + x];
        let d = |p: u16, q: u16| (i32::from(p >> 11) - i32::from(q >> 11)).abs() <= 1 && (i32::from((p >> 5) & 63) - i32::from((q >> 5) & 63)).abs() <= 2;
        if d(a, b) {
            agree += 1;
        }
    }
    log::info!("probes that agree: {agree} of {}", probes.len());
}

#[esp_hal::main]
fn main() -> ! {
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::_240MHz));
    esp_alloc::heap_allocator!(size: 72 * 1024);
    esp_alloc::psram_allocator!(peripherals.PSRAM, esp_hal::psram);
    esp_println::logger::init_logger_from_env();
    log::info!("hibari-os-s3-bench: vello_cpu against the integer renderer, 240 MHz");
    let delay = Delay::new();
    loop {
        run(410, 502);
        run(410, 120);
        run(200, 200);
        log::info!("=== done; again in 5 s ===");
        delay.delay_millis(5000);
    }
}
