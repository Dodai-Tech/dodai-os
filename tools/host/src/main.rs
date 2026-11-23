//! `hibari-host`: runs the Hibari OS shell on this computer.
//!
//! ```text
//! hibari-host --list
//! hibari-host --device watch-round --png out.png [--tap X,Y]... [--renderer auto|vello-cpu|vello-gpu] [--at MS]
//! hibari-host --device p4-7in --window        (needs --features window)
//! ```

use std::process::ExitCode;

use hibari_device::Backend;
use hibari_host::{Session, plan, png_vello_cpu, profile as find_profile, profiles};

#[derive(Debug, Default, PartialEq, Eq)]
struct Options {
    list: bool,
    device: Option<String>,
    /// The renderer asked for on the command line; `None` is the automatic choice.
    renderer: Option<Backend>,
    /// Milliseconds after the last tap at which the picture is taken (default: settled).
    at: Option<u32>,
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
            "--at" => {
                let v = value("--at")?;
                o.at = Some(
                    v.parse()
                        .map_err(|_| format!("bad --at '{v}', want milliseconds"))?,
                );
            }
            "--renderer" => {
                let name = value("--renderer")?;
                o.renderer = match name.as_str() {
                    "auto" => None,
                    other => Some(Backend::from_name(other).ok_or_else(|| {
                        format!("unknown renderer '{other}'; auto, vello-cpu or vello-gpu")
                    })?),
                }
            }
            "--tap" => o.taps.push(parse_tap(&value("--tap")?)?),
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    Ok(o)
}

fn run(o: &Options) -> Result<(), String> {
    if o.list {
        for p in profiles() {
            let (w, h) = p.screen_size();
            println!("{:<28} {w}x{h}", p.name);
        }
        return Ok(());
    }
    let name = o.device.as_deref().unwrap_or("desktop");
    let profile =
        find_profile(name).ok_or_else(|| format!("unknown device '{name}'; try --list"))?;
    let mut session = Session::new(profile).ok_or_else(|| format!("'{name}' has no screen"))?;
    for &(x, y) in &o.taps {
        session.tap(x, y);
        session.advance(60);
    }
    // Let the last slide run to the requested time; 1000 ms is long past any transition.
    session.advance(o.at.unwrap_or(1000));
    if let Some(path) = &o.png {
        let plan = plan(profile, o.renderer).ok_or_else(|| format!("'{name}' has no screen"))?;
        eprintln!("renderer: {plan}");
        let bytes = png_vello_cpu(&session)?;
        std::fs::write(path, bytes).map_err(|e| format!("{path}: {e}"))?;
        eprintln!("wrote {path}");
    }
    if o.window {
        #[cfg(feature = "window")]
        hibari_host::window::run(&mut session).map_err(|e| e.to_string())?;
        #[cfg(not(feature = "window"))]
        return Err("built without a window; rebuild with --features window".to_owned());
    } else if o.png.is_none() {
        let (w, h) = profile.screen_size();
        println!("{}: {w}x{h}; use --png FILE or --window", profile.name);
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
    fn the_renderer_is_automatic_unless_chosen() {
        assert_eq!(parse(args("--png a.png")).unwrap().renderer, None);
        assert_eq!(parse(args("--renderer auto")).unwrap().renderer, None);
        for (flag, want) in [
            ("vello-cpu", Backend::VelloCpu),
            ("vello-gpu", Backend::VelloGpu),
        ] {
            let o = parse(args(&format!("--renderer {flag}"))).unwrap();
            assert_eq!(o.renderer, Some(want), "{flag}");
        }
        assert!(parse(args("--renderer vello")).is_err());
        assert!(parse(args("--renderer skia")).is_err(), "removed");
        assert!(parse(args("--renderer")).is_err());
    }

    #[test]
    fn both_renderers_write_a_png() {
        let dir = std::env::temp_dir().join(format!("hibari-host-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for backend in ["vello-cpu", "auto", "vello-gpu"] {
            let path = dir.join(format!("{backend}.png"));
            let cmd = format!(
                "--device watch-round --renderer {backend} --tap 200,150 --png {}",
                path.display()
            );
            run(&parse(args(&cmd)).unwrap()).unwrap();
            assert!(std::fs::metadata(&path).unwrap().len() > 100, "{backend}");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn at_takes_milliseconds() {
        assert_eq!(parse(args("--at 120")).unwrap().at, Some(120));
        assert_eq!(parse(args("--png a.png")).unwrap().at, None);
        assert!(parse(args("--at soon")).is_err());
        assert!(parse(args("--at -5")).is_err());
        assert!(parse(args("--at")).is_err());
    }

    #[test]
    fn a_picture_taken_mid_slide_differs_from_the_settled_one() {
        let dir = std::env::temp_dir().join(format!("hibari-at-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let shot = |at: &str| {
            let path = dir.join(format!("at-{at}.png"));
            let cmd = format!(
                "--device waveshare-s3-amoled-2.06 --tap 300,56 --at {at} --png {}",
                path.display()
            );
            run(&parse(args(&cmd)).unwrap()).unwrap();
            std::fs::read(path).unwrap()
        };
        let (start, mid, end) = (shot("0"), shot("60"), shot("1000"));
        assert!(start != mid && mid != end && start != end);
        std::fs::remove_dir_all(&dir).unwrap();
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
