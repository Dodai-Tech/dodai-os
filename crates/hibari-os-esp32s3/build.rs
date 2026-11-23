use std::{env, fs, path::Path};

fn main() {
    embuild::espidf::sysenv::output();

    // WiFi credentials come from ../../config.toml (git-ignored) and are baked into the firmware.
    let root = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let cfg = root.join("config.toml");
    println!("cargo:rerun-if-changed={}", cfg.display());
    let text = fs::read_to_string(&cfg).unwrap_or_default();
    let value: toml::Value = text.parse().unwrap_or(toml::Value::Table(Default::default()));
    let get = |k: &str| value.get("wifi").and_then(|w| w.get(k)).and_then(|v| v.as_str()).unwrap_or("").to_string();
    println!("cargo:rustc-env=WIFI_SSID={}", get("ssid"));
    println!("cargo:rustc-env=WIFI_PASS={}", get("password"));
}
