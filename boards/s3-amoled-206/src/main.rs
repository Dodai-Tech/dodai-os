//! Hibari OS on the Waveshare ESP32-S3-Touch-AMOLED-2.06: the shell on the AMOLED, driven by touch.
//!
//! `no_std`, built on esp-hal with no ESP-IDF. The loop polls touch, turns samples into gestures,
//! feeds the shell and redraws the rows that changed.
#![no_std]
#![no_main]

extern crate alloc;


use alloc::vec;
use hibari_board_s3_amoled_206::{DISPLAY, LCD_QSPI_MHZ, PROFILE, qspi};
use hibari_device::CapabilitySet;
use hibari_drivers::{co5300::Co5300, ft3168::Ft3168};
use hibari_hal::input::GestureRecognizer;
use hibari_hal::{Clock, Display, Rect, Touch};
use hibari_launcher::{AppInfo, FrameBuf, Ui};
use fusuma_render_embedded::Embedded;
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

esp_bootloader_esp_idf::esp_app_desc!();

/// Milliseconds since boot from the hardware timer.
struct BootClock;

impl Clock for BootClock {
    fn now_ms(&self) -> u32 {
        (Instant::now().duration_since_epoch().as_millis() & u64::from(u32::MAX)) as u32
    }
}

const fn app(name: &'static str, requires: &[hibari_device::Capability]) -> AppInfo {
    AppInfo { id: name, name, requires: CapabilitySet::of(requires) }
}

/// Placeholder apps until the real ones are written.
static APPS: [AppInfo; 12] = {
    use hibari_device::Capability::{Battery, Ble, Display, Imu, Storage, Touch, Wifi};
    [
        app("Weather", &[Display, Wifi]),
        app("Clock", &[Display]),
        app("Settings", &[Display, Touch]),
        app("Compass", &[Display, Imu]),
        app("Battery", &[Display, Battery]),
        app("Files", &[Display, Storage, Ble]),
        app("Timer", &[Display, Touch]),
        app("Notes", &[Display, Touch, Storage]),
        app("Music", &[Display, Touch]),
        app("Maps", &[Display, Wifi]),
        app("Camera", &[Display, Touch]),
        app("Health", &[Display, Imu]),
    ]
};

#[esp_hal::main]
fn main() -> ! {
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::_160MHz));
    esp_alloc::heap_allocator!(size: 72 * 1024);
    esp_alloc::psram_allocator!(peripherals.PSRAM, esp_hal::psram);
    esp_println::logger::init_logger_from_env();
    log::info!("Hibari OS on {}", PROFILE.name);

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
    log::info!("display ready");

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

    // Framebuffer in PSRAM, one RGB565 value per pixel.
    let (w, h) = Display::size(&display);
    let pixels = vec![0u16; usize::from(w) * usize::from(h)].leak();
    let Some(fb) = FrameBuf::new(w, h, pixels) else { panic!("framebuffer size") };
    // Fusuma's integer renderer draws into it and the framebuffer tracks the rows that changed.
    let mut canvas = Embedded::new(fb);

    let mut ui = Ui::new(PROFILE.capabilities, &APPS, (w, h));
    let mut gestures = GestureRecognizer::default();
    let _ = ui.draw(&mut canvas, BootClock.now_ms());
    flush(&mut display, canvas.target_mut());
    log::info!("shell drawn; waiting for touch");

    let mut next_beat = BootClock.now_ms();
    loop {
        let now = BootClock.now_ms();
        if now.wrapping_sub(next_beat) < u32::MAX / 2 {
            log::info!("alive {} s, screen {:?}", now / 1000, ui.shell().screen());
            next_beat = now.wrapping_add(1000);
        }
        let mut redraw = false;
        if let Ok(Some(event)) = touch.poll() {
            if let Some(gesture) = gestures.feed(event) {
                log::info!("gesture {gesture:?}");
                redraw |= ui.on_gesture(gesture, now) != hibari_launcher::Output::None;
            }
        }
        if let Some(gesture) = gestures.tick(now) {
            redraw |= ui.on_gesture(gesture, now) != hibari_launcher::Output::None;
        }
        // A slide needs a new frame every time round; the last one settles the picture.
        redraw |= ui.tick(now);
        if redraw {
            let _ = ui.draw(&mut canvas, now);
            flush(&mut display, canvas.target_mut());
        }
        // Idle polling can wait; while sliding, draw as fast as the panel takes frames.
        if !ui.is_animating() {
            delay.delay_millis(10);
        }
    }
}

/// Sends the changed rows to the panel.
fn flush<D: Display>(display: &mut D, fb: &mut FrameBuf<'_>) {
    let w = fb.width();
    if let Some((first, rows)) = fb.take_dirty() {
        let height = (rows.len() / usize::from(w)) as u16;
        let _ = display.flush(Rect { x: 0, y: first, w, h: height }, rows);
    }
}
