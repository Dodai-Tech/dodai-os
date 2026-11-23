# hibari-bench-c6

How large a surface `vello_cpu` can draw on an ESP32-C6: 512 KB of RAM, no PSRAM, no floating-point unit.
It needs no screen. For each size, smallest first, it draws the same launcher scene through Fusuma's integer
renderer and then through `vello_cpu` and logs the time and the heap used; the largest size that logs `ok`
is the limit (an allocation that does not fit panics, and the log shows which size). The result is in
[`RESULT.md`](RESULT.md).

```sh
nu scripts/native.nu build --board c6-bench
espflash flash --chip esp32c6 --no-monitor tools/bench-c6/target/riscv32imac-unknown-none-elf/release/hibari-bench-c6
espflash monitor
```

This replaces the app on the board; `backups/c6` (git-ignored) holds what was there.
