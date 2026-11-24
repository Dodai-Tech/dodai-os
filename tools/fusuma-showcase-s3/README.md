# fusuma-showcase-s3

The Fusuma components showcase on the Waveshare ESP32-S3-Touch-AMOLED-2.06 watch (410x502 AMOLED, FT3168 touch), on
`esp-hal` with no ESP-IDF. The app is `fusuma-showcase` (the same code the desktop window and the ESP32-P4 run; on this
small screen it switches to its compact layout). This crate is the watch as a `fusuma_platform_embedded::Board` plus a
loop. It reuses the display and touch drivers of `hibari-drivers` and the bus code of `hibari-board-s3-amoled-206`.

```sh
just native-install board=s3-fusuma-showcase                       # build, install in OTA slot 0 and boot it
just native-install board=s3-fusuma-showcase features=skia         # also link tiny-skia
just native-monitor                                                # timings and heap
just native-restore                                                # boot the factory app again
```

Cargo features: `vello-cpu` and `fonts` (default), `skia`. Which renderer draws is the shared rule's choice among
what is linked, from the chip's memory.
