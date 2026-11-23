#!/usr/bin/env nu
# The pure parts of scripts/esp.nu.

use std/assert
use runner.nu *
use ../esp.nu [FIRMWARE firmware sdkconfig_text newest_bootloader flash_args parse_ptable image_size le]

# One 32-byte partition entry.
def pt_entry [name: string, type: int, offset: int, size: int]: nothing -> binary {
    let n = ($name | encode utf-8) | bytes add --end (0..<(16 - ($name | str length)) | each { 0x[00] } | bytes collect)
    let u32 = {|v| $v | into binary --compact | bytes add --end 0x[00 00 00 00] | bytes at 0..<4 }
    [0x[aa 50] ($type | into binary --compact | bytes at 0..<1) 0x[00] (do $u32 $offset) (do $u32 $size) $n 0x[00 00 00 00]] | bytes collect
}

def "test esp: a known firmware crate is found, an unknown one is an error" [] {
    let first = $FIRMWARE | first
    assert equal (firmware $first.crate) $first
    assert (try { firmware "nope"; false } catch { true })
}

def "test esp: sdkconfig text gets an absolute partition path" [] {
    assert equal (sdkconfig_text "A=\"@PARTITIONS@\"\nB=\"@PARTITIONS@\"\n" "/abs/p.csv") "A=\"/abs/p.csv\"\nB=\"/abs/p.csv\"\n"
}

def "test esp: every firmware crate ships its template files" [] {
    const root = path self ../..
    for fw in $FIRMWARE {
        for f in [Cargo.toml build.rs partitions.csv sdkconfig.defaults.in rust-toolchain.toml .cargo/config.toml src/main.rs] {
            assert ($root | path join $fw.dir $f | path exists) $"($fw.dir)/($f) is missing"
        }
    }
}

def "test esp: no bootloader gives null, the newest one wins" [] {
    assert equal (newest_bootloader "/definitely/not/here") null
    let root = mktemp --directory
    for h in [aaa bbb] {
        let d = $root | path join esp-idf-sys $h out build bootloader
        mkdir $d
        "x" | save ($d | path join bootloader.bin)
        sleep 20ms
    }
    assert equal (newest_bootloader $root | path split | reverse | get 4) bbb
    rm -rf $root
}

def "test esp: flash args carry chip, flash size, bootloader and optional port" [] {
    let fw = { crate: c, chip: esp32c6, target: t, flash_mb: 4 }
    let with = flash_args $fw /b.bin /app /dev/ttyACM0
    assert ($with | any {|a| $a == "esp32c6" })
    assert ($with | any {|a| $a == "4mb" })
    assert ($with | any {|a| $a == "--port" })
    assert equal ($with | last) "/app"
    assert not (flash_args $fw /b.bin /app "" | any {|a| $a == "--port" })
}

def "test esp: the partition table is parsed up to the first empty entry" [] {
    let table = [(pt_entry nvs 1 0x9000 0x6000) (pt_entry factory 0 0x10000 0x100000) (0..<32 | each { 0x[ff] } | bytes collect)] | bytes collect
    let rows = parse_ptable $table
    assert equal ($rows.name) [nvs factory]
    assert equal $rows.1.offset 0x10000
    assert equal $rows.1.size 0x100000
}

def "test esp: an image size follows its segment lengths" [] {
    # header (24) + 2 segments of 4 and 8 data bytes, read through a fake flash
    let seg = {|len| [0x[00 00 00 00] ($len | into binary --compact | bytes add --end 0x[00 00 00 00] | bytes at 0..<4) (0..<$len | each { 0x[aa] } | bytes collect)] | bytes collect }
    let flash = [0x[e9 02] (0..<22 | each { 0x[00] } | bytes collect) (do $seg 4) (do $seg 8) (0..<64 | each { 0x[00] } | bytes collect)] | bytes collect
    let read = {|off, len| $flash | bytes at $off..<($off + $len) }
    assert equal (image_size $read 0) (((24 + 12 + 16 + 1 + 15) // 16) * 16 + 32)
}

def "test esp: erased flash has no image" [] {
    let read = {|off, len| 0..<$len | each { 0x[ff] } | bytes collect }
    assert equal (image_size $read 0x10000) null
}

def main [] { run-tests }
