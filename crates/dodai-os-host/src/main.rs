//! `dodai-os-host [--device NAME]`: shows the device profile the host would pretend to be.

use dodai_os_host::{DeviceProfile, PROFILES};

fn main() {
    let mut args = std::env::args().skip(1);
    let name = match (args.next().as_deref(), args.next()) {
        (Some("--device"), Some(name)) => name,
        (Some("--list"), _) => {
            for p in PROFILES {
                println!("{}  {}x{}", p.name, p.width, p.height);
            }
            return;
        }
        (None, _) => "desktop".to_owned(),
        _ => {
            eprintln!("usage: dodai-os-host [--device NAME | --list]");
            std::process::exit(2);
        }
    };
    let Some(p) = DeviceProfile::by_name(&name) else {
        eprintln!("unknown device '{name}'; try --list");
        std::process::exit(2);
    };
    println!("{}: {}x{} {:?}", p.name, p.width, p.height, p.shape);
    for c in p.capabilities.iter() {
        println!("  {c:?}");
    }
}
