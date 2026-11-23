# hibari-demo-s3-vello

A demo for the ESP32-S3 (Waveshare ESP32-S3-Touch-AMOLED-2.06): `vello_cpu`, built with `libm` and no `std`,
draws an anti-aliased launcher on the AMOLED. A touch presses the card under the finger; only that card is
redrawn. It is slower than the integer renderer (see `tools/bench-s3/RESULT.md`) and exists to see the
quality difference on the real screen. Not published; it reuses the display and touch code of
`hibari-board-s3-amoled-206`.

```sh
just native-install board=s3-vello-demo    # builds, installs in OTA slot 0 and boots it
just native-monitor                        # timings
just native-install                        # put the real firmware back
```
