# External plugin hosting foundation — 2026-11-17

Historical checkpoint. See the later [runtime integration record](external-plugin-hosting-integration-2026-11-17.md) for current implementation and checks.

Status at this checkpoint: partial implementation. This is not complete external plugin hosting
acceptance. The [architecture status matrix](../architecture/external-plugin-hosting.md)
lists the remaining active-instance, renderer, model, control/editor, modulation,
PCM sidechain, state/history and packaging work.

## Implemented and checked

- Portable plugin identities, role metadata and plain/normalized parameter domains.
- Default-root resolution, native payload/header checks, symlink deduplication,
  bounded traversal and resource fingerprints.
- Disposable CLAP and VST3 entry/factory inspection and versioned responses.
- Persistent scan journals, pending versus attempted recovery, quarantine,
  fingerprint retries, duplicate precedence, atomic cache writes and writer locks.
- CLI catalog inspection, cached read-only mode, retry, rescan and cache rebuild.

No installed third-party plugin was loaded. Native checks used only the
repository's own Drive exports copied into temporary bundles. User projects and
recordings were not changed. The unrelated untracked tools/__pycache__ directory
was retained.

## Executed checks

Platform: macOS arm64, workspace-pinned Rust toolchain. Commands used `--offline`
because the required dependencies were available locally.

| Command | Result |
| --- | --- |
| `cargo test -p daw-plugin-host -p daw-unit-core -p daw-plugin-scan -p daw-cli --offline` | PASS: 117 tests, zero failures or ignored tests. |
| `cargo test -p daw-plugin-host -p daw-unit-core -p daw-plugin-scan -p daw-cli --release --offline` | PASS: 117 tests, zero failures or ignored tests. |
| `cargo test -p daw-cli --test plugins --offline` | PASS: three catalog integration tests. |
| `cargo fmt --all --check` | PASS on the frozen source. |
| `cargo clippy --workspace --all-targets --offline -- -D warnings` | PASS on the frozen source. |
| `cargo build -p daw-drive-clap -p daw-drive-vst3 -p daw-plugin-scan --offline` | PASS. |
| `cargo build -p daw-cli -p daw-plugin-scan --offline` | PASS. |
| `python3 tools/check_plugin_scanner.py` | PASS: both native factories, exact identities, differing macOS bundle/payload names, unload and damaged-module failure without metadata publication. |
| `python3 tools/check_cli.py` | PASS: existing import, render, collection, inspection, codec, overwrite and exit-code workflows. |
| `cargo test --workspace --offline` | FAIL at the native AUv2 registration assertion described below. |
| `cargo test --workspace --offline --no-fail-fast` | FAIL: 3,577 passed, 20 failed, 22 ignored. All 20 failures are the AUv2 native-registration case below. This broad run began before the last scanner refinements; final affected-package tests and clippy were rerun after those refinements. |
| `cargo test --workspace --release --offline --no-fail-fast` | FAIL: 3,581 passed, 20 failed, 22 ignored. All 20 failures are the same AUv2 registration case. Final affected release tests were rerun after the last parameter-domain refinement. |

Development logs are under ignored `work/plugin-hosting-validation/`. Early
development checks caught cache pending-state and canonical-path defects; both
were corrected and regression tests pass. Concurrent compilation during source
editing also produced stale-interface errors; final checks use frozen source.

The broad workspace run fails the unchanged
`native_audio_component_registration_and_host_calls` test in AUv2 export crates:
`plugins/drive-auv2/src/native/tests.rs` asserts that the registered component is
non-null. The component lookup returns null in this execution environment. The
cause has not been established; this record does not claim that sandboxing is the
cause, that the test is defective, or that full workspace validation passes.
No AUv2 implementation or test was changed to bypass the assertion.

## Remaining limits

Windows/Linux native discovery, macOS aliases, Windows shortcuts, incompatible
architecture inventory and packaged scanner lookup/signing remain unverified or
pending. The portable resolver tests do not establish native acceptance on those
systems. Active plugin processing, instruments, editor windows, modulation,
auxiliary audio inputs, state recall, Undo, live/export parity, callback deadlines,
audio devices and listening have not been implemented or checked by this change.

Exact generated-binding provenance and the full distributed dependency/notice
graph require release review. No external plugin redistribution is added.
