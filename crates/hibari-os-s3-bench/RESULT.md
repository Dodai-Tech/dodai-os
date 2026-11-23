# Result: vello_cpu on the ESP32-S3

Measured on the Waveshare ESP32-S3-Touch-AMOLED-2.06 (ESP32-S3, 240 MHz, 8 MB PSRAM), release build
(`opt-level = 3`, LTO), `vello_cpu` 0.4 with `libm` and no `std`, `fusuma-render-embedded` 0.2. Same scene on
both: a status bar, rounded cards with a disc and an outline each (no text). Average of 10 frames after 2 warm-ups.

| Surface | Integer renderer | `vello_cpu` build | render | to RGB565 | total | memory |
|---|---|---|---|---|---|---|
| 410 x 502 (full screen) | 61.4 ms | 91.7 ms | 119.5 ms | 49.6 ms | **260.8 ms** | 1.37 MB |
| 410 x 120 (a strip) | 14.4 ms | 29.2 ms | 26.0 ms | 12.5 ms | **67.7 ms** | 0.33 MB |
| 200 x 200 | 14.5 ms | 29.9 ms | 22.1 ms | 10.2 ms | **62.2 ms** | 0.27 MB |

* `vello_cpu` is about **4 times slower** than the integer renderer in total, and about 2 times slower for
  the render step alone. A full-screen frame is ~4 frames per second.
* It needs **1.4 MB** at full screen (a 4-byte-per-pixel pixmap plus working buffers), which only fits with
  PSRAM. The integer renderer draws into the 2-byte-per-pixel frame buffer and needs nothing more.
* The pictures agree: every probe pixel matched within what 16-bit colour allows.
* Not measured: text (the Vello path has no glyph call yet), the P4 (400 MHz, vector extension), and any
  SIMD (Vello's SIMD paths are not available for Xtensa, so this ran the scalar fallback).

Conclusion: keep `fusuma-render-embedded` on the ESP32 series. See `docs/ROADMAP.md`.
