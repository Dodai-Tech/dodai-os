# hibari-bench-s3

A benchmark firmware for the ESP32-S3: it draws the same launcher scene through Fusuma's integer renderer
and through `vello_cpu` (built with `libm`, no `std`), times both, and logs the result over the serial
port. Not published. The result is in [`RESULT.md`](RESULT.md): `vello_cpu` is about 4 times slower than
the integer renderer on the S3 and needs 1.4 MB, so the ESP keeps the integer renderer.

```sh
just native-build board=s3-bench       # build only
just native-install board=s3-bench     # install in OTA slot 0 and boot it
just native-monitor                    # read the numbers
just native-install                    # put the real firmware back
```
