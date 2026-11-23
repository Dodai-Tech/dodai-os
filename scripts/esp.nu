#!/usr/bin/env nu
# ──────────────────────────────────────────────────────────────────────────────
# hibari-os — ESP firmware helpers (the justfile calls these)
# ──────────────────────────────────────────────────────────────────────────────
# Firmware crates are not workspace members (they need the ESP toolchain), so their
# generated files, flashing and backups live here, tested, instead of in sh recipes.
#
#   nu scripts/esp.nu gen-sdkconfig
#   nu scripts/esp.nu build <crate> [--release]     nu scripts/esp.nu build-all
#   nu scripts/esp.nu flash <crate> [--port P]      nu scripts/esp.nu monitor [--port P]
#   nu scripts/esp.nu backup [--port P] [--dir backups]
# ──────────────────────────────────────────────────────────────────────────────

const ROOT = path self ..

# One record per firmware crate: crate, chip, target, flash_mb (written by the scaffold).
export const FIRMWARE = [[crate, chip, target, flash_mb]; ["hibari-os-esp32s3", "esp32s3", "xtensa-esp32s3-espidf", 32]]

# ── Pure helpers ──────────────────────────────────────────────────────────────

# A firmware record by crate name; error listing the known ones.
export def firmware [crate: string]: nothing -> record {
    let r = $FIRMWARE | where crate == $crate
    if ($r | is-empty) { error make { msg: $"unknown firmware crate '($crate)'; one of: ($FIRMWARE.crate | str join ', ')" } }
    $r | first
}

# sdkconfig.defaults from its template: @PARTITIONS@ becomes an absolute path.
export def sdkconfig_text [template: string, partitions: string]: nothing -> string {
    $template | str replace --all "@PARTITIONS@" $partitions
}

# The newest IDF-built bootloader under `<crate>/target/<target>/<profile>/build`, or null.
export def newest_bootloader [build_dir: path]: nothing -> any {
    let found = glob ($build_dir | path join "esp-idf-sys" "*" "out" "build" "bootloader" "bootloader.bin")
    if ($found | is-empty) { return null }
    $found | each {|f| { path: $f, modified: (ls $f | get modified.0) } } | sort-by modified | last | get path
}

# espflash arguments for a flash (`port` empty = let espflash pick).
export def flash_args [fw: record, bootloader: path, image: path, port: string]: nothing -> list<string> {
    let base = [
        "flash" "--chip" $fw.chip "--flash-size" $"($fw.flash_mb)mb" "--partition-table" "partitions.csv"
        "--bootloader" $bootloader "--monitor"
    ]
    let port_args = if ($port | is-empty) { [] } else { ["--port" $port] }
    [...$base ...$port_args $image]
}

# Little-endian unsigned integer from `n` bytes at `at`.
export def le [bytes: binary, at: int, n: int]: nothing -> int {
    $bytes | bytes at $at..<($at + $n) | into int --endian little
}

# Partition table entries from the raw table bytes (0x8000 in flash, 32 B per entry).
export def parse_ptable [bytes: binary]: nothing -> table {
    0..(($bytes | bytes length) // 32 - 1)
    | each {|i|
        let e = $bytes | bytes at ($i * 32)..<($i * 32 + 32)
        if ($e | bytes at 0..<2) != 0x[aa 50] { null } else {
            {
                name: ($e | bytes at 12..<28 | bytes split 0x[00] | first | decode utf-8)
                type: (le $e 2 1)
                subtype: (le $e 3 1)
                offset: (le $e 4 4)
                size: (le $e 8 4)
            }
        }
    }
    | take while {|r| $r != null }
}

# Bytes an app image really occupies, read through `read: {|offset, len| binary}`;
# null when there is no image (erased partition).
export def image_size [read: closure, offset: int]: nothing -> any {
    let head = do $read $offset 24
    if ($head | bytes at 0..<1) != 0x[e9] { return null }
    mut pos = 24
    for _ in 0..<(le $head 1 1) {
        let seg = do $read ($offset + $pos) 8
        $pos = $pos + 8 + (le $seg 4 4)
    }
    # + checksum byte padded to 16, + 32-byte SHA-256 digest
    (($pos + 1 + 15) // 16) * 16 + 32
}

def hex [n: int]: nothing -> string { $n | format number | get lowerhex }

# ── Commands ──────────────────────────────────────────────────────────────────

def "main gen-sdkconfig" [] {
    for fw in $FIRMWARE {
        let dir = $ROOT | path join crates $fw.crate
        let csv = $dir | path join partitions.csv | path expand
        sdkconfig_text (open --raw ($dir | path join sdkconfig.defaults.in)) $csv | save -f ($dir | path join sdkconfig.defaults)
        print $"Generated ($dir | path join sdkconfig.defaults)"
    }
}

def "main build" [crate: string, --release] {
    let fw = firmware $crate
    cd ($ROOT | path join crates $fw.crate)
    if $release { ^cargo build --release } else { ^cargo build }
}

def "main build-all" [] {
    for fw in $FIRMWARE { main build $fw.crate --release }
}

def "main flash" [crate: string, --port: string = ""] {
    let fw = firmware $crate
    let dir = $ROOT | path join crates $fw.crate
    let out = $dir | path join target $fw.target release
    let bl = newest_bootloader ($out | path join build)
    if $bl == null { error make { msg: $"bootloader.bin not found under ($out)/build; run `just fw-release ($crate)`" } }
    cd $dir
    ^espflash ...(flash_args $fw $bl ($out | path join $fw.crate) $port)
}

def "main monitor" [--port: string = ""] {
    let p = if ($port | is-empty) { [] } else { ["--port" $port] }
    ^espflash monitor ...$p
}

# Parts of the flash worth keeping, read over USB (~10 KB/s, so no full dump): the partition
# table, everything before the first app (bootloader, NVS, otadata) and each app's real size.
def "main backup" [--port: string = "", --dir: path = "backups"] {
    mkdir $dir
    let p = if ($port | is-empty) { [] } else { ["--port" $port] }
    let tmp = mktemp
    let read = {|off: int, len: int|
        ^espflash read-flash ...$p (hex $off) (hex $len) $tmp | complete | ignore
        open --raw $tmp | into binary
    }
    do $read 0x8000 0x1000 | save -f ($dir | path join ptable.bin)
    let table = parse_ptable (open --raw ($dir | path join ptable.bin))
    if ($table | is-empty) { error make { msg: "no partition table at 0x8000 (is this an ESP board in a normal boot?)" } }
    let apps = $table | where type == 0 | sort-by offset
    let first_app = $apps | get -o 0.offset | default 0x10000
    ^espflash read-flash ...$p 0 (hex $first_app) ($dir | path join $"boot-and-data-0x0.bin")
    for app in $apps {
        let size = image_size $read $app.offset
        if $size == null { print $"($app.name): empty, skipped"; continue }
        let file = $dir | path join $"($app.name)-(hex $app.offset).bin"
        ^espflash read-flash ...$p (hex $app.offset) (hex $size) $file
    }
    rm -f $tmp
    print $"Backed up to ($dir). Restore with: espflash write-bin 0x0 <boot-and-data-0x0.bin>; espflash write-bin <offset> <app.bin>"
}

def main [] { help main }
