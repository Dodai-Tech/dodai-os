#!/usr/bin/env nu
# The pure parts of scripts/native.nu.

use std/assert
use runner.nu *
use ../native.nu [BOARDS board crc32 le_bytes otadata_entry slot_seq partition check_fits esp_env]

def "test native: known boards are found, unknown ones are errors" [] {
    let first = $BOARDS | first
    assert equal (board $first.name) $first
    assert (try { board "nope"; false } catch { true })
}

def "test native: crc32 matches the standard check values" [] {
    assert equal (crc32 ("123456789" | into binary)) 0xCBF43926
    assert equal (crc32 0x[]) 0
    assert equal (crc32 0x[00 00 00 00]) 0x2144DF1C
}

def "test native: crc32 with the ROM's initial value matches ESP-IDF's otadata crc" [] {
    # binascii.crc32(struct.pack('<I', seq), 0xFFFFFFFF), the formula of IDF's otatool.py
    assert equal (crc32 0x[01 00 00 00] 0xFFFFFFFF) 0x4743989A
    assert equal (crc32 0x[02 00 00 00] 0xFFFFFFFF) 0x55F63774
}

def "test native: little-endian bytes" [] {
    assert equal (le_bytes 0x01020304 4) 0x[04 03 02 01]
    assert equal (le_bytes 0xFFFFFFFF 4) 0x[ff ff ff ff]
    assert equal (le_bytes 2 4) 0x[02 00 00 00]
}

def "test native: an otadata entry is 32 bytes with the sequence and its crc" [] {
    let e = otadata_entry 2
    assert equal ($e | bytes length) 32
    assert equal ($e | bytes at 0..<4) 0x[02 00 00 00]
    assert equal ($e | bytes at 4..<24) (0..<20 | each { 0x[ff] } | bytes collect)
    assert equal ($e | bytes at 24..<28) 0x[ff ff ff ff]
    assert equal ($e | bytes at 28..<32) 0x[74 37 f6 55]
    assert (try { otadata_entry 0; false } catch { true })
}

def "test native: ota_N is selected by sequence N+1" [] {
    assert equal (slot_seq ota_0) 1
    assert equal (slot_seq ota_1) 2
    assert (try { slot_seq factory; false } catch { true })
    assert (try { slot_seq "ota_"; false } catch { true })
}

def "test native: partitions are looked up and sizes checked" [] {
    let table = [{ name: otadata, offset: 0xf000, size: 0x2000 } { name: ota_1, offset: 0x1000000, size: 100 }]
    assert equal (partition $table ota_1).offset 0x1000000
    assert (try { partition $table nope; false } catch { true })
    check_fits ($table | last) 100
    assert (try { check_fits ($table | last) 101; false } catch { true })
}

def "test native: esp_env puts the toolchain first and finds clang" [] {
    let home = mktemp --directory
    let gcc = $home | path join .rustup toolchains esp xtensa-esp-elf esp-1 xtensa-esp-elf bin
    let clang = $home | path join .rustup toolchains esp xtensa-esp32-elf-clang esp-2 esp-clang lib
    mkdir $gcc $clang
    let e = esp_env $home
    assert equal ($e.PATH | first) $gcc
    assert equal $e.LIBCLANG_PATH $clang
    rm -rf $home
}

def "test native: a missing toolchain is a clear error" [] {
    let home = mktemp --directory
    let msg = try { esp_env $home; "" } catch {|e| $e.msg }
    assert ($msg | str contains "espup install")
    rm -rf $home
}

def main [] { run-tests }
