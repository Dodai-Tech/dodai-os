//! The Fusuma components showcase on the Waveshare ESP32-S3-Touch-AMOLED-2.06.
//!
//! The app is `fusuma-showcase`, the same code the desktop window and the ESP32-P4 run. This file is the watch as
//! a `fusuma_platform_embedded::Board` (the CO5300 AMOLED over QSPI and the FT3168 touch panel, from
//! `hibari-drivers`) and a loop: everything else is shared.
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
use fusuma::platform::{Machine, Render};
use fusuma::render::{Point, Rect};
use fusuma_platform_embedded::{Board, Chip, Driver, DriverError, LINKED, Pixels, esp_hardware};
use fusuma_showcase::Showcase;
use hibari_board_s3_amoled_206::{DISPLAY, LCD_QSPI_MHZ, qspi};
use hibari_drivers::{co5300::Co5300, ft3168::Ft3168};
use hibari_hal::input::Phase;
use hibari_hal::{Clock, Display, Touch};

esp_bootloader_esp_idf::esp_app_desc!();

/// The PSRAM this board carries, in kilobytes (8 MB octal).
const PSRAM_KB: u32 = 8192;

/// Milliseconds since boot from the hardware timer.
struct BootClock;

impl Clock for BootClock {
    fn now_ms(&self) -> u32 {
        (Instant::now().duration_since_epoch().as_millis() & u64::from(u32::MAX)) as u32
    }
}

fn now_ms() -> u64 {
    Instant::now().duration_since_epoch().as_millis()
}

/// The watch: the AMOLED and its touch panel.
struct Watch<D, T> {
    display: D,
    touch: T,
    /// Where the finger is, kept between samples (the panel only reports changes).
    held: Option<Point>,
    /// Pixels of the region being sent: the panel takes one rectangle at a time.
    buffer: Vec<u16>,
}

impl<D: Display, T: Touch> Board for Watch<D, T> {
    type Error = ();

    fn size(&self) -> (u32, u32) {
        let (w, h) = self.display.size();
        (u32::from(w), u32::from(h))
    }

    fn touch(&mut self) -> Result<Option<Point>, ()> {
        // A bus error is a lifted finger, as the driver treats it.
        if let Ok(Some(e)) = self.touch.poll() {
            self.held = match e.phase {
                Phase::Up => None,
                Phase::Down | Phase::Move => Some(Point::new(i32::from(e.point.x), i32::from(e.point.y))),
            };
        }
        Ok(self.held)
    }

    fn present(&mut self, region: Rect, pixels: &mut dyn Pixels) -> Result<(), ()> {
        let (w, h) = self.size();
        // The CO5300 takes windows that start and end on even pixels.
        let x0 = (region.x.max(0) as u32) & !1;
        let y0 = (region.y.max(0) as u32) & !1;
        let x1 = ((region.right().max(0) as u32 + 1) & !1).min(w);
        let y1 = ((region.bottom().max(0) as u32 + 1) & !1).min(h);
        if x1 <= x0 || y1 <= y0 {
            return Ok(());
        }
        let (rw, rh) = ((x1 - x0) as usize, (y1 - y0) as usize);
        self.buffer.clear();
        self.buffer.resize(rw * rh, 0);
        for (i, row) in self.buffer.chunks_exact_mut(rw).enumerate() {
            pixels.row(y0 + i as u32, x0, row);
        }
        let area = hibari_hal::Rect { x: x0 as u16, y: y0 as u16, w: rw as u16, h: rh as u16 };
        self.display.flush(area, &self.buffer).map_err(|_| ())
    }
}

#[esp_hal::main]
fn main() -> ! {
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::_160MHz));
    esp_alloc::heap_allocator!(size: 72 * 1024);
    esp_alloc::psram_allocator!(peripherals.PSRAM, esp_hal::psram);
    esp_println::logger::init_logger_from_env();
    log::info!("Fusuma showcase on the ESP32-S3 watch");

    let delay = Delay::new();

    // Display: SPI2 in quad mode with DMA.
    let (rx_buf, rx_desc, tx_buf, tx_desc) = dma_buffers!(qspi::DMA_CHUNK);
    let (Ok(dma_rx), Ok(dma_tx)) = (DmaRxBuf::new(rx_desc, rx_buf), DmaTxBuf::new(tx_desc, tx_buf)) else {
        panic!("DMA buffers");
    };
    let spi_config = SpiConfig::default().with_frequency(Rate::from_mhz(LCD_QSPI_MHZ)).with_mode(SpiMode::_0);
    let Ok(spi) = Spi::new(peripherals.SPI2, spi_config) else { panic!("SPI2") };
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

    // Touch: I2C0 and the controller's reset line.
    let Ok(i2c) = I2c::new(peripherals.I2C0, I2cConfig::default().with_frequency(Rate::from_khz(400))) else {
        panic!("I2C0")
    };
    let i2c = i2c.with_sda(peripherals.GPIO15).with_scl(peripherals.GPIO14);
    let mut touch_reset = Output::new(peripherals.GPIO9, Level::High, OutputConfig::default());
    touch_reset.set_low();
    delay.delay_millis(10);
    touch_reset.set_high();
    delay.delay_millis(50);
    let mut touch = Ft3168::new(i2c, BootClock);
    if touch.init().is_err() {
        log::warn!("touch init failed; continuing without it");
    }

    let watch = Watch { display, touch, held: None, buffer: Vec::new() };
    let (w, h) = watch.size();

    // The renderer: the shared rule over this chip's memory, among what is linked.
    let machine = Machine {
        screen: Some((w, h)),
        hardware: Some(esp_hardware(Chip::Esp32S3, PSRAM_KB)),
        render: Render::default(),
    };
    let Some(plan) = fusuma::platform::plan(&machine, None, LINKED) else { panic!("no renderer is linked") };
    log::info!("{w}x{h} screen; renderer: {plan}");
    let Some(mut driver) = Driver::new(Showcase::new(w, h), watch, plan.uses) else {
        panic!("cannot make a {} surface", plan.uses.name())
    };
    log::info!("drawing with {}; heap free {} KB", plan.uses.name(), esp_alloc::HEAP.free() / 1024);

    let (mut frames, mut busy_ms, mut next_report) = (0u32, 0u64, now_ms() + 10_000);
    loop {
        let begun = now_ms();
        match driver.step(begun) {
            Ok(step) => {
                if step.shown.is_some() {
                    frames += 1;
                    busy_ms += now_ms() - begun;
                    if frames == 1 {
                        log::info!("first frame {} ms; heap free {} KB", busy_ms, esp_alloc::HEAP.free() / 1024);
                    }
                }
            }
            Err(DriverError::Draw(e)) => log::error!("draw failed: {}", e.0),
            Err(DriverError::Board(())) => log::error!("panel write failed"),
        }
        if now_ms() >= next_report {
            next_report = now_ms() + 10_000;
            log::info!("alive: {} on {w}x{h}: {frames} frames, {} ms each on average; heap free {} KB", plan.uses.name(), busy_ms.checked_div(u64::from(frames)).unwrap_or(0), esp_alloc::HEAP.free() / 1024);
        }
        // Touch is polled, so wait a little unless an animation is running.
        if !driver.wants_frame() {
            delay.delay_millis(8);
        }
    }
}
