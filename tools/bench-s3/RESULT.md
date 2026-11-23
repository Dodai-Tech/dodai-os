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

## Through Fusuma's `Renderer` trait (the `no_std` Vello backend)

The same scene drawn through `fusuma-render-vello-cpu` built with `default-features = false` and `libm`,
which is what the toolkit and the launcher would use:

| Surface | raw `vello_cpu` | via the `Renderer` trait | memory of the backend |
|---|---|---|---|
| 410 x 502 | 260.4 ms | 267.0 ms | 957 KB |
| 410 x 120 | 67.6 ms | 69.0 ms | 228 KB |
| 200 x 200 | 62.0 ms | 63.6 ms | 192 KB |

The trait costs about 2 to 3 percent. The backend builds `no_std` for the S3 (Xtensa) and the C6 (RISC-V).

Conclusion for full frames: `vello_cpu` is slower. But a demo that repaints only the touched card (`hibari-demo-s3-vello`: 41 ms to render, 11 ms to send) felt richer and more responsive on the real screen, with smooth edges, so the preference is Vello on chips with PSRAM and partial redraws; the integer renderer stays for chips without PSRAM. See `docs/ROADMAP.md`.
