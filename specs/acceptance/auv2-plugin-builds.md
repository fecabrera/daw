# AUv2 acceptance gates

The [architecture contract](../architecture/auv2-plugin-builds.md) owns behavior.
This matrix distinguishes automated coverage from native release acceptance.

| Gate | Required evidence |
| --- | --- |
| Bindings | Exact approved generation inventory; repeat clean generation hashes; independent C/Rust layouts, offsets, signed union field and callback calls. No generated/SDK source in archives. |
| Registry | All manifest products have unique AU codes, bundle IDs/classes and declared targets; unchanged existing identities/state payloads. |
| Native API | C selector dispatch, macOS process-local AudioComponent registration, property ownership/errors, lifecycle, pull audio, cache/reset and rejected negotiation. |
| DSP | Compare each typed processor at six rates, odd/variable/zero blocks, latency/tails, bypass, transport and worker lifetime. |
| Automation | Signed ramps/sample offsets, choices, malformed batch atomicity, capacity bounds, pre-render scheduling and contention; zero-frame updates. |
| Real time | Warm render/reset/dense schedules have no allocation or retirement; expected errors stay bounded. Contained render fault is silent, latched and recovers only after reinitialization. |
| Editor | Shared body/gesture/state tests plus native Cocoa attach/reopen, scale/focus/keyboard/accessibility, views after instance close and balanced gestures. |
| Packaging | Both profiles and platform-relative APIs, all 20 AU plists/targets, notices, executable modes, signatures, provenance, subset checksums and publication rollback. |
| Regression | Rustfmt, strict workspace Clippy, debug and release workspace tests; all Python tooling tests; existing CLAP/VST3 ABI gates. |
| Apple validator | Install in a disposable test environment; run `python3 tools/check_auv2.py --units all` for each profile. Retain strict auval logs and exact component hashes. Process-local registration tests do not establish this gate. |
| Hosts | Logic Pro scan/insert, multiple instances, automation modes, dirty state/presets/save/reopen, generic/custom editor, timing, bypass/delay compensation, offline/realtime bounce and tails for all 20. GarageBand and REAPER AU provide independent host coverage. |
| Platforms | Native minimum macOS 12 and supported arm64 host checks. Windows/Linux and Intel macOS workspace checks verify inert AU crates without AU SDK generation or framework linking. Intel/universal AU exports are outside this delivery. |

Only executed checks belong in validation history. Automated success does not
establish bundle discovery, host behavior, listening quality or minimum-OS support.
No publication, notarization or installation into a user's existing component
folder is required to perform the in-process tests.
