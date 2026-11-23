#!/usr/bin/env nu
# ──────────────────────────────────────────────────────────────────────────────
# hibari-os: esp-hal (no_std) boards, built and flashed without ESP-IDF
# ──────────────────────────────────────────────────────────────────────────────
# Flashing is non-destructive: the image goes into an OTA slot and an `otadata` entry selects
# it. The bootloader, the partition table and the native firmware are not touched, and
# `native` erases `otadata` so the bootloader falls back to the factory app.
#
# Verified on the Waveshare ESP32-S3-Touch-AMOLED-2.06 (32 MB flash): the slot must lie in the first
# 16 MB. `ota_1` at 0x1000000 is skipped by the bootloader, which falls back to the other slot.
# An otadata entry with a wrong CRC makes the bootloader erase otadata and boot the factory app.
#
#   nu scripts/native.nu build   [--board s3-amoled-206]
#   nu scripts/native.nu install [--board B] [--slot ota_0] [--port P]
#   nu scripts/native.nu native  [--port P]
#   nu scripts/native.nu monitor [--port P]
# ──────────────────────────────────────────────────────────────────────────────

use esp.nu [parse_ptable]

const ROOT = path self ..

# What can be built for an ESP32 with esp-hal: name, directory, crate, and the directory whose `board.toml`
# describes the device it runs on (chip, target and flash size are read from its [firmware] table).
export const BOARDS = [[name, dir, crate, device]; ["s3-amoled-206", "boards/s3-amoled-206", "hibari-board-s3-amoled-206", "boards/s3-amoled-206"] ["s3-bench", "tools/bench-s3", "hibari-bench-s3", "boards/s3-amoled-206"] ["s3-vello-demo", "tools/vello-demo-s3", "hibari-demo-s3-vello", "boards/s3-amoled-206"]]

# ── Pure helpers ──────────────────────────────────────────────────────────────

# The `chip`, `target` and `flash_mb` of the [firmware] table of a `board.toml`.
export def firmware_of [toml: path]: nothing -> record {
    if not ($toml | path exists) { error make { msg: $"($toml) does not exist" } }
    let fw = open $toml | get -o firmware
    if $fw == null { error make { msg: $"($toml) has no [firmware] table" } }
    for key in [chip target flash_mb] {
        if ($fw | get -o $key) == null { error make { msg: $"($toml): [firmware] has no '($key)'" } }
    }
    { chip: $fw.chip, target: $fw.target, flash_mb: $fw.flash_mb }
}

# A board record by name, with the firmware settings of its device file; error listing the known ones.
export def board [name: string]: nothing -> record {
    let r = $BOARDS | where name == $name
    if ($r | is-empty) { error make { msg: $"unknown board '($name)'; one of: ($BOARDS.name | str join ', ')" } }
    let b = $r | first
    $b | merge (firmware_of ($ROOT | path join $b.device board.toml))
}

# CRC-32 (IEEE) of `data` with the ROM's calling convention: `init` is the "previous result", 0 for a
# plain CRC (the zlib value). The bootloader checks an otadata entry with init 0xFFFFFFFF, which is
# a different value than zlib's; a wrong one makes it erase otadata and boot the factory app.
export def crc32 [data: binary, init: int = 0]: nothing -> int {
    mut crc = $init bit-xor 0xFFFFFFFF
    for i in 0..<($data | bytes length) {
        $crc = $crc bit-xor ($data | bytes at $i..<($i + 1) | into int)
        for _ in 0..<8 {
            $crc = if ($crc bit-and 1) == 1 { ($crc bit-shr 1) bit-xor 0xEDB88320 } else { $crc bit-shr 1 }
        }
    }
    $crc bit-xor 0xFFFFFFFF
}

# `n` as `width` little-endian bytes.
export def le_bytes [n: int, width: int]: nothing -> binary {
    0..<$width | each {|i| (($n bit-shr ($i * 8)) bit-and 0xFF) | into binary --compact | bytes at 0..<1 } | bytes collect
}

# The otadata entry that makes the bootloader boot OTA slot number `seq - 1` (32 bytes):
# sequence, 20 unused label bytes, state (undefined), CRC of the sequence.
export def otadata_entry [seq: int]: nothing -> binary {
    if $seq < 1 { error make { msg: "otadata sequence starts at 1" } }
    let label = 0..<20 | each { 0x[ff] } | bytes collect
    [(le_bytes $seq 4) $label (le_bytes 0xFFFFFFFF 4) (le_bytes (crc32 (le_bytes $seq 4) 0xFFFFFFFF) 4)] | bytes collect
}

# The otadata sequence that selects `ota_N`: the bootloader boots slot `(seq - 1) mod slots`.
export def slot_seq [slot: string]: nothing -> int {
    let m = $slot | parse --regex '^ota_(?<n>\d+)$'
    if ($m | is-empty) { error make { msg: $"'($slot)' is not an OTA slot like ota_1" } }
    ($m.0.n | into int) + 1
}

# A partition by name from a parsed table; error listing the names.
export def partition [table: table, name: string]: nothing -> record {
    let p = $table | where name == $name
    if ($p | is-empty) { error make { msg: $"partition '($name)' not found; table has: ($table.name | str join ', ')" } }
    $p | first
}

# Fails unless an image of `bytes` fits the partition.
export def check_fits [part: record, bytes: int] {
    if $bytes > $part.size {
        error make { msg: $"image is ($bytes) bytes but ($part.name) holds ($part.size)" }
    }
}

# The cross-compiler and clang that `espup install` set up, as an env record for `with-env`.
export def esp_env [home: path]: nothing -> record {
    let toolchain = $home | path join .rustup toolchains esp
    let gcc = glob ($toolchain | path join xtensa-esp-elf "*" xtensa-esp-elf bin) | sort
    let clang = glob ($toolchain | path join xtensa-esp32-elf-clang "*" esp-clang lib) | sort
    if ($gcc | is-empty) or ($clang | is-empty) {
        error make { msg: $"no Xtensa toolchain under ($toolchain); run `espup install`" }
    }
    {
        PATH: ($env.PATH | prepend ($gcc | last))
        LIBCLANG_PATH: ($clang | last)
    }
}

# ── Commands ──────────────────────────────────────────────────────────────────

def port_args [port: string]: nothing -> list<string> {
    if ($port | is-empty) { [] } else { ["--port" $port] }
}

def crate_dir [b: record]: nothing -> path { $ROOT | path join $b.dir }

def elf_path [b: record]: nothing -> path { crate_dir $b | path join target $b.target release $b.crate }

def "main build" [--board: string = "s3-amoled-206"] {
    let b = board $board
    cd (crate_dir $b)
    with-env (esp_env $nu.home-dir) { ^cargo build --release }
}

# Writes the image to an OTA slot and selects it. The bootloader and the partition table stay.
def "main install" [--board: string = "s3-amoled-206", --slot: string = "ota_0", --port: string = ""] {
    let b = board $board
    let seq = slot_seq $slot
    main build --board $board
    let image = mktemp --suffix .bin
    ^espflash save-image --chip $b.chip --flash-size $"($b.flash_mb)mb" (elf_path $b) $image
    let table_file = mktemp --suffix .bin
    ^espflash read-flash ...(port_args $port) 0x8000 0xc00 $table_file
    let table = parse_ptable (open --raw $table_file | into binary)
    let target = partition $table $slot
    let otadata = partition $table otadata
    check_fits $target (ls $image | get size.0 | into int)
    let entry = mktemp --suffix .bin
    otadata_entry $seq | save -f $entry
    ^espflash write-bin ...(port_args $port) $target.offset $image
    ^espflash write-bin ...(port_args $port) $otadata.offset $entry
    ^espflash reset ...(port_args $port)
    print -e $"Hibari OS installed in ($slot); run `nu scripts/native.nu native` to boot the native firmware again."
}

# Erases otadata so the bootloader runs the factory app (the board's native firmware).
def "main native" [--port: string = ""] {
    let table_file = mktemp --suffix .bin
    ^espflash read-flash ...(port_args $port) 0x8000 0xc00 $table_file
    let otadata = partition (parse_ptable (open --raw $table_file | into binary)) otadata
    ^espflash erase-region ...(port_args $port) $otadata.offset $otadata.size
    ^espflash reset ...(port_args $port)
    print -e "Native firmware selected."
}

def "main monitor" [--port: string = ""] {
    ^espflash monitor ...(port_args $port)
}

def main [] { print -e "usage: nu scripts/native.nu <build|install|native|monitor>" }
