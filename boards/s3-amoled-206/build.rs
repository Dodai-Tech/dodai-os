//! Turns `board.toml` into Rust constants (`profile.rs` in `OUT_DIR`), so the device is written down
//! once: the firmware, the host preview and the build scripts all read the same file.

use std::{env, fs, path::PathBuf, process::ExitCode};

use hibari_device::DeviceProfile;

fn main() -> ExitCode {
    println!("cargo:rerun-if-changed=board.toml");
    println!("cargo:rerun-if-changed=build.rs");
    let text = match fs::read_to_string("board.toml") {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: cannot read board.toml: {e}");
            return ExitCode::FAILURE;
        }
    };
    // The profile borrows from the text for the rest of this short-lived process.
    let text: &'static str = Box::leak(text.into_boxed_str());
    let profile = match DeviceProfile::from_toml(text) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: board.toml: {e}");
            return ExitCode::FAILURE;
        }
    };
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap_or_default()).join("profile.rs");
    if let Err(e) = fs::write(&out, profile.to_rust("PROFILE")) {
        eprintln!("error: cannot write {}: {e}", out.display());
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
