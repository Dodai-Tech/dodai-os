//! The public API as a user of the crate sees it.

use dodai_os_core::parse_kv;

#[test]
fn parses_a_config_text() {
    let text = "# settings\nname = demo\n\nlevel=3\n";
    let pairs: Vec<_> = text.lines().filter_map(parse_kv).collect();
    assert_eq!(pairs, [("name", "demo"), ("level", "3")]);
}
