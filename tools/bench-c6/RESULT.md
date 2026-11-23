# Result: vello_cpu on an ESP32-C6

Measured on an ESP32-C6 (revision v0.2, 8 MB flash, no PSRAM, no floating-point unit), 160 MHz
(`CpuClock::max()`), release build (`opt-level = 3`, LTO), `fusuma-render-vello-cpu` built `no_std` with
`libm`, `esp-hal` 1.0.0, a heap of 320 KB (the chip has about 512 KB of RAM in all). The same launcher scene
(a status bar and a grid of rounded cards with a disc and an outline each, no text) through Fusuma's integer
renderer, through `tiny-skia` (built `no_std` with its software-float feature) and through `vello_cpu`; average of 5 frames. No screen was attached: the time is drawing and
rendering only.

| Surface | Integer renderer | `tiny-skia` | `vello_cpu` | Frame / pixmap: integer, `tiny-skia`, Vello | Heap left: `tiny-skia`, Vello |
|---|---|---|---|---|---|
| 64 x 64 | 1.6 ms | 11 ms | 48 ms | 8 KB, 16 KB, 32 KB | 311 KB, 296 KB |
| 128 x 128 | 3.7 ms | 13 ms | 59 ms | 32 KB, 64 KB, 89 KB | 262 KB, 239 KB |
| 160 x 160 | 4.9 ms | 14 ms | 64 ms | 50 KB, 100 KB, 129 KB | 225 KB, 199 KB |
| 200 x 200 | 11 ms | 27 ms | 126 ms | 78 KB, 156 KB, 192 KB | 168 KB, 136 KB |
| 172 x 320 | 11 ms | 38 ms | 176 ms | 108 KB, 215 KB, 265 KB | 108 KB, 63 KB |
| 240 x 240 | 15 ms | 50 ms | 224 ms | 113 KB, 225 KB, 273 KB | 97 KB, 54 KB |
| 240 x 320 | 21 ms | 73 ms | **out of memory** | 150 KB, 300 KB, needs 369 KB | 20 KB, none |

(At 32 x 32 no card fits, so that row says nothing and is left out.)

* **All three run on the C6.** `vello_cpu` is the slowest and heaviest; `tiny-skia` is the middle one; the
  integer renderer is the fastest and lightest but has no anti-aliasing or alpha.
* **`tiny-skia` is about 3 to 4 times faster than `vello_cpu`** here (38 against 176 ms for 172 x 320) and uses
  exactly its pixmap, 4 bytes per pixel, where Vello adds working buffers (about 4.8 bytes per pixel in all).
  It draws a full 240 x 320 surface (73 ms, 20 KB left) where Vello runs out of memory.
* **The integer renderer is still 3 to 4 times faster than `tiny-skia`** and uses half the memory (2 bytes per
  pixel), so on the C6 it is the choice when RAM must be left for the radio.
* **A card costs about 12 ms in `tiny-skia`** against 45 ms in Vello and a few ms in the integer renderer, so
  region redraws are comfortable with `tiny-skia` and the integer renderer.
* **RAM:** at 172 x 320 `tiny-skia` leaves 108 KB of the 320 KB heap, Vello 63 KB, the integer renderer 210 KB. I
  did not measure how much the radio needs, so what is really left for WiFi or Bluetooth is unknown.

Not measured: text, the radio's memory, a heap larger than 320 KB, a real display's flush time, or any other
C6 board. Reproduce: `nu scripts/native.nu build --board c6-bench`, then flash and read the log (see
`README.md`).

A trap found on the way: `esp-hal` 1.2 uses a different app-descriptor section than `esp-bootloader-esp-idf`
0.4 writes, so an image built from a fresh lockfile has no descriptor and `espflash` refuses it. This crate
pins `esp-hal` to 1.0.0, as the S3 firmware's lockfile does.
