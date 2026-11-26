# Test audio recording alignment

The simulated loopback uses the output rate converter, a modeled input clock
and known impulses. It runs without audio hardware.

```sh
cargo test --locked -p daw-output simulated_loopback
```

For the native test, connect a line-level output to a suitable line input or
use a dedicated virtual loopback device. Select a pair that routes the first
output to the first input. The test plays one low-level impulse and captures
the input in memory. It reports the frame and time offset. Set
`DAW_LOOPBACK_RATE` to each project rate you want to qualify.

```sh
DAW_LOOPBACK_INPUT="Your wired input" DAW_LOOPBACK_OUTPUT="Your wired output" \
DAW_LOOPBACK_RATE=48000 cargo test --locked -p daw-output native_wired_loopback -- --ignored --nocapture
```

Repeat starts on each supported platform. Record the endpoints, native rates,
buffer configuration, raw alignment error and whether output conversion
was active. Test drift over a long take separately. Qualification requires
a physical or dedicated digital loopback result on the device pair.
