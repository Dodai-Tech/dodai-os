//! `dodai-os-host`: runs the Dodai OS shell on this computer.
//!
//! ```text
//! dodai-os-host --list
//! dodai-os-host --device watch-round --png out.png [--tap X,Y]...
//! dodai-os-host --device p4-7in --window        (needs --features window)
//! ```

use std::process::ExitCode;

use dodai_os_host::{DeviceProfile, PROFILES, Session, png_bytes};

#[derive(Debug, Default, PartialEq, Eq)]
struct Options {
    list: bool,
    device: Option<String>,
    png: Option<String>,
    window: bool,
    taps: Vec<(i16, i16)>,
}

fn parse_tap(text: &str) -> Result<(i16, i16), String> {
    let (x, y) = text
        .split_once(',')
        .ok_or_else(|| format!("bad --tap '{text}', want X,Y"))?;
    let num = |s: &str| {
        s.trim()
            .parse::<i16>()
            .map_err(|_| format!("bad --tap '{text}', want X,Y"))
    };
    Ok((num(x)?, num(y)?))
}

fn parse(args: impl IntoIterator<Item = String>) -> Result<Options, String> {
    let mut o = Options::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
        match arg.as_str() {
            "--list" => o.list = true,
            "--window" => o.window = true,
            "--device" => o.device = Some(value("--device")?),
            "--png" => o.png = Some(value("--png")?),
            "--tap" => o.taps.push(parse_tap(&value("--tap")?)?),
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    Ok(o)
}

fn run(o: &Options) -> Result<(), String> {
    if o.list {
        for p in PROFILES {
            println!("{:<16} {}x{}", p.name, p.width, p.height);
        }
        return Ok(());
    }
    let name = o.device.as_deref().unwrap_or("desktop");
    let profile = DeviceProfile::by_name(name)
        .ok_or_else(|| format!("unknown device '{name}'; try --list"))?;
    let mut session = Session::new(profile).ok_or_else(|| format!("'{name}' has no screen"))?;
    for &(x, y) in &o.taps {
        session.tap(x, y);
    }
    if let Some(path) = &o.png {
        let bytes = png_bytes(session.display()).map_err(|e| e.to_string())?;
        std::fs::write(path, bytes).map_err(|e| format!("{path}: {e}"))?;
        eprintln!("wrote {path}");
    }
    if o.window {
        #[cfg(feature = "window")]
        dodai_os_host::window::run(&mut session).map_err(|e| e.to_string())?;
        #[cfg(not(feature = "window"))]
        return Err("built without a window; rebuild with --features window".to_owned());
    } else if o.png.is_none() {
        println!(
            "{}: {}x{}; use --png FILE or --window",
            profile.name, profile.width, profile.height
        );
    }
    Ok(())
}

fn main() -> ExitCode {
    match parse(std::env::args().skip(1)).and_then(|o| run(&o)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn parses_flags_and_taps() {
        let o = parse(args(
            "--device watch-round --png a.png --tap 10,20 --tap -1,5 --window",
        ))
        .unwrap();
        assert_eq!(o.device.as_deref(), Some("watch-round"));
        assert_eq!(o.png.as_deref(), Some("a.png"));
        assert_eq!(o.taps, [(10, 20), (-1, 5)]);
        assert!(o.window && !o.list);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse(args("--bogus")).is_err());
        assert!(parse(args("--device")).is_err());
        assert!(parse(args("--tap 10")).is_err());
        assert!(parse(args("--tap a,b")).is_err());
        assert!(parse(args("--tap 99999,1")).is_err());
    }

    #[test]
    fn unknown_and_headless_devices_are_errors() {
        let bad = |s: &str| run(&parse(args(s)).unwrap()).unwrap_err();
        assert!(bad("--device nope").contains("unknown device"));
        assert!(bad("--device headless").contains("no screen"));
    }
}
