//! How large a surface `vello_cpu` can draw on an ESP32-C6, and how long it takes.
#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec;

use esp_backtrace as _;
use esp_hal::{clock::CpuClock, delay::Delay, time::Instant};
use fusuma_render::{Color, Point, Rect, Renderer};
use fusuma_render_embedded::{Embedded, Frame};
use fusuma_render_vello_cpu::Scene;

esp_bootloader_esp_idf::esp_app_desc!();

const BAR: i32 = 28;
const MARGIN: i32 = 6;
const FRAMES: u32 = 5;
/// The heap: the C6 has about 512 KB of RAM in total, shared with the code's data and the stack.
const HEAP_BYTES: usize = 320 * 1024;

fn micros() -> u64 {
    Instant::now().duration_since_epoch().as_micros()
}

/// The launcher's shapes: a status bar and a grid of rounded cards, each with a disc and an outline.
fn scene<R: Renderer>(r: &mut R, w: i32, h: i32) {
    let _ = r.clear(Color::rgb(8, 8, 16));
    let _ = r.fill_rect(Rect::new(0, 0, w as u32, BAR as u32), 0, Color::rgb(33, 38, 41));
    let cols = (w / 90).clamp(1, 3);
    let rows = ((h - BAR) / 90).clamp(1, 4);
    for i in 0..cols * rows {
        let (col, row) = (i % cols, i / cols);
        let (x0, y0) = (col * w / cols, BAR + row * (h - BAR) / rows);
        let (cw, ch) = ((col + 1) * w / cols - x0, BAR + (row + 1) * (h - BAR) / rows - y0);
        let (x, y, cw, ch) = (x0 + MARGIN, y0 + MARGIN, cw - 2 * MARGIN, ch - 2 * MARGIN);
        if cw < 4 || ch < 4 {
            continue;
        }
        let _ = r.fill_rect(Rect::new(x, y, cw as u32, ch as u32), 10, Color::rgb(33, 38, 41));
        let _ = r.fill_circle(Point::new(x + cw / 2, y + ch / 2 - 6), 14.min(cw / 2 - 2).max(1) as u32, Color::rgb(24, 162, 165));
        let _ = r.stroke_rect(Rect::new(x + 6, y + ch - 22, (cw - 12).max(1) as u32, 14), 5, 2, Color::rgb(250, 200, 0));
    }
}

fn average(mut f: impl FnMut()) -> u64 {
    f();
    let start = micros();
    for _ in 0..FRAMES {
        f();
    }
    (micros() - start) / u64::from(FRAMES)
}

fn try_size(w: u16, h: u16) {
    let (wi, hi) = (i32::from(w), i32::from(h));
    log::info!("--- {w}x{h}: heap free {} bytes; vello needs about {} ---", esp_alloc::HEAP.free(), usize::from(w) * usize::from(h) * 48 / 10);

    {
        let mut screen = Embedded::new(Frame::new(w, h, vec![0u16; usize::from(w) * usize::from(h)]).expect("frame"));
        let t = average(|| scene(&mut screen, wi, hi));
        log::info!("integer renderer: {t:>8} us/frame, {} bytes for the frame", usize::from(w) * usize::from(h) * 2);
    }

    // tiny-skia, one surface at a time; dropped before vello allocates its own.
    {
        let before = esp_alloc::HEAP.used();
        log::info!("tiny-skia: allocating...");
        match fusuma_render_skia::Skia::new(u32::from(w), u32::from(h)) {
            None => log::info!("tiny-skia: the surface cannot exist"),
            Some(mut r) => {
                let t = average(|| scene(&mut r, wi, hi));
                log::info!(
                    "tiny-skia: ok, {t:>8} us/frame, heap used {} bytes, heap free {} bytes",
                    esp_alloc::HEAP.used().saturating_sub(before),
                    esp_alloc::HEAP.free()
                );
            }
        }
    }

    let before = esp_alloc::HEAP.used();
    log::info!("vello_cpu: allocating...");
    let Some(mut r) = fusuma_render_vello_cpu::new(u32::from(w), u32::from(h)) else {
        log::info!("vello_cpu: the surface cannot exist");
        return;
    };
    let t_draw = average(|| {
        scene(&mut r, wi, hi);
        r.with_scene(|s| {
            let _ = s.pixels();
        });
    });
    log::info!(
        "vello_cpu: ok, {t_draw:>8} us/frame (draw and render), heap used {} bytes, heap free {} bytes",
        esp_alloc::HEAP.used().saturating_sub(before),
        esp_alloc::HEAP.free()
    );
}

#[esp_hal::main]
fn main() -> ! {
    let _peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));
    esp_alloc::heap_allocator!(size: HEAP_BYTES);
    esp_println::logger::init_logger_from_env();
    log::info!("hibari-bench-c6: vello_cpu on an ESP32-C6 (no PSRAM, no FPU), heap {HEAP_BYTES} bytes");
    let delay = Delay::new();
    delay.delay_millis(500);
    for (w, h) in [(32, 32), (64, 64), (96, 96), (128, 128), (160, 160), (172, 320), (200, 200), (240, 240), (240, 320)] {
        try_size(w, h);
    }
    log::info!("=== every size fitted ===");
    loop {
        delay.delay_millis(5000);
    }
}
