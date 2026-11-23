# Result: vello_cpu on an ESP32-C6

Measured on an ESP32-C6 (revision v0.2, 8 MB flash, no PSRAM, no floating-point unit), 160 MHz
(`CpuClock::max()`), release build (`opt-level = 3`, LTO), `fusuma-render-vello-cpu` built `no_std` with
`libm`, `esp-hal` 1.0.0, a heap of 320 KB (the chip has about 512 KB of RAM in all). The same launcher scene
(a status bar and a grid of rounded cards with a disc and an outline each, no text) through Fusuma's integer
renderer and through `vello_cpu`; average of 5 frames. No screen was attached: the time is drawing and
rendering only.

| Surface | Cards | Integer renderer | `vello_cpu` | `vello_cpu` heap | Heap left |
|---|---|---|---|---|---|
| 64 x 64 | 1 | 1.6 ms | 48 ms | 32 KB | 296 KB |
| 96 x 96 | 1 | 2.6 ms | 53 ms | 54 KB | 274 KB |
| 128 x 128 | 1 | 3.7 ms | 59 ms | 89 KB | 239 KB |
| 160 x 160 | 1 | 4.9 ms | 65 ms | 129 KB | 199 KB |
| 200 x 200 | 2 | 11.1 ms | 127 ms | 192 KB | 136 KB |
| 172 x 320 | 3 | 11.1 ms | 176 ms | 265 KB | 63 KB |
| 240 x 240 | 4 | 15.4 ms | 224 ms | 273 KB | 54 KB |
| 240 x 320 | n/a | 21.1 ms | **out of memory** | needs about 369 KB | none |

(At 32 x 32 no card fits, so that row says nothing and is left out.)

* **It works.** `vello_cpu` draws on a chip with no PSRAM and no FPU, up to about 240 x 240 with a 320 KB heap.
* **It is 10 to 20 times slower than the integer renderer** here (176 against 11 ms for 172 x 320), where it
  was about 4 times slower on the ESP32-S3. The C6 has no FPU, so Vello's floating-point maths run in
  software. A card costs about 45 ms, so a region redraw is practical but a full screen is not.
* **It takes almost all the RAM.** About 4.8 bytes per pixel; at 172 x 320 or 240 x 240 only 54 to 63 KB of
  the heap is left, and that heap is most of the chip's RAM. WiFi and Bluetooth need much of that, and I did
  not measure how much, so a C6 that draws with Vello probably cannot also run the radio.
* **The integer renderer needs far less:** its frame is 2 bytes per pixel (110 KB for 172 x 320) and it
  draws in milliseconds.

Not measured: text, the radio's memory, a heap larger than 320 KB, a real display's flush time, or any other
C6 board. Reproduce: `nu scripts/native.nu build --board c6-bench`, then flash and read the log (see
`README.md`).

A trap found on the way: `esp-hal` 1.2 uses a different app-descriptor section than `esp-bootloader-esp-idf`
0.4 writes, so an image built from a fresh lockfile has no descriptor and `espflash` refuses it. This crate
pins `esp-hal` to 1.0.0, as the S3 firmware's lockfile does.
