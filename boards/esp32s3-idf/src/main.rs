//! hibari-board-esp32s3-idf: a sample ESP-IDF (std) firmware for the ESP32-S3 with WiFi. The logic lives in the host-testable `hibari-*` crates.

mod wifi;

use esp_idf_svc::{eventloop::EspSystemEventLoop, hal::delay::FreeRtos, hal::peripherals::Peripherals, nvs::EspDefaultNvsPartition};

const WIFI_SSID: &str = env!("WIFI_SSID");
const WIFI_PASS: &str = env!("WIFI_PASS");

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    log::info!("hibari-board-esp32s3-idf v{} starting", env!("CARGO_PKG_VERSION"));

    if let Err(e) = run() {
        log::error!("fatal: {e:#}");
        // Keep the task alive so the log stays readable.
        loop {
            FreeRtos::delay_ms(1000);
        }
    }
}

fn run() -> anyhow::Result<()> {
    anyhow::ensure!(!WIFI_SSID.is_empty(), "set [wifi] in config.toml (copy config.toml.example) and rebuild");
    let p = Peripherals::take()?;
    let sys_loop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    // Create the driver once and retry connecting: a failed constructor would consume the modem.
    let mut wifi = wifi::start(p.modem, sys_loop, nvs)?;
    loop {
        match wifi::join(&mut wifi, WIFI_SSID, WIFI_PASS) {
            Ok(()) => break,
            Err(e) => {
                log::warn!("WiFi failed: {e}; retrying");
                FreeRtos::delay_ms(5000);
            }
        }
    }
    let ip = wifi.wifi().sta_netif().get_ip_info()?;
    log::info!("online: {ip:?}");

    // Pure logic from the core crate (unit-tested and fuzzed on the host).
    log::info!("{:?}", hibari_device::CapabilitySet::of(&[hibari_device::Capability::Wifi]).len());
    loop {
        FreeRtos::delay_ms(10_000);
    }
}
