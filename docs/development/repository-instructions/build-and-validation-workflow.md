# Build and validation workflow

Follow the platform prerequisites, build steps, troubleshooting, and packaging instructions in the [development guides](../../../docs/development/README.md). Run the commands below from the repository root with the pinned toolchain.

## Run and headless commands

```sh
cargo run -p daw-desktop
cargo run -p daw-desktop -- examples/demo
cargo build -p daw-cli
cargo run -p daw-cli -- --project examples/demo project.validate
cargo run -p daw-cli -- --project examples/demo project.export.mix --output work/mix.wav
cargo run -p daw-cli -- --project examples/demo project.export.mix --output work/mix.wav --overwrite
cargo run -p daw-cli -- --project examples/demo project.export.mix --output work/mix.wav --codec float32
cargo run -p daw-cli -- --project examples/demo project.export.mix --output work/mix.mp3 --format mp3 --bitrate 320
```

Create `work/` before using it as an output directory. If `work/cargo-home` contains the local dependency cache, prefix Cargo commands with `CARGO_HOME="$PWD/work/cargo-home"`. Add `--offline` only when the required dependencies are cached. Do not require this local cache on other machines.

The CLI infers WAV, MP3 or AAC from the output extension unless you specify `--format`. WAV accepts `--codec pcm16|pcm24|float32`. MP3 and AAC accept `--bitrate 128|192|256|320`. Reject incompatible options and mismatched format extensions. The CLI must run without graphics or an audio device.

## Validation

For code changes, run affected checks while developing, then run the workspace checks before completion. Both debug and release tests are required:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --workspace --release
```

- The default Cargo test profile uses optimization level 1 with debug assertions and overflow checks explicitly enabled to keep DSP matrices within CI limits. Keep debug assertions enabled for the default `cargo test --workspace` run. Release tests do not replace it: lifecycle checks such as egui texture-delta cleanup can fail only with debug assertions enabled. Record debug and release commands and results separately in the validation record.
- Add meaningful tests for changed behavior and failure paths. Keep unit tests near the implementation; use `crates/project/tests/workflow.rs` for storage/render/export workflows and existing egui input tests for interactions. Avoid tests that only repeat implementation details.
- For CLI or shared import/render/export changes, build `daw-cli`, then run `python3 tools/check_cli.py`. It needs the tracked demo and uses temporary output folders.
- `python3 tools/create_demo.py` regenerates tracked demo files. Do not run it over user edits or include regenerated fixtures unless required by the task.
- On macOS, rebuild desktop deliverables with `sh tools/package_macos.sh debug` or `release` when desktop/runtime changes need an updated app. The ad-hoc signed bundle is under `target/<profile>/DAW.app`.
- CI calls `.github/workflows/platform.yml` independently for Windows, macOS, and Linux, with fail-fast disabled. Each invocation has separate `test` and `build` jobs; build depends only on that invocation's tests and includes release builds, packaging, and artifact uploads. Do not gate all builds on the aggregate test matrix. Both jobs use `.github/actions/setup-build` for native dependencies, the pinned Rust toolchain, Python, and Cargo caching. Tests run locked workspace checks and CLI smoke checks. Keep the portable Windows executable with its project license and dependency notices, and retain the same notices in macOS DMG and Linux ZIP artifacts. Use the native scripts in `tools` rather than duplicating packaging commands in the workflow.
- Keep CI command output visible in the console and in log artifacts. Preserve command failures when using `tee`, upload logs on failure, and write job summaries through `tools/summarize_ci.py`. Windows setup selects the MSVC linker by absolute path to avoid Git Bash's `link.exe`. UI background-job tests use a bounded 30-second wall-clock deadline for slower hosted workers.
- `.github/workflows/plugins.yml` builds effect products as CLAP/VST3 and macOS AUv2 previews on independent native platform workers. Reuse the shared setup/log/summary helpers and `tools/package_plugins.py`; retain native bundle layouts, executable ZIP permissions and notices. Keep plugin uploads gated on that worker's tests. Development CI does not replace SDK-validator, native editor/host or release acceptance checks.
- For documentation-only changes, check content, links, commands against their definitions, and `git diff --check`; do not rebuild or run unrelated audio tests.
- Use temporary folders for tests. Do not overwrite user projects, recordings, or exports. Review `git diff` and `git status` before completion.
- Once relevant checks pass, repeat them only after new changes or unresolved failures. Record unavailable device/platform checks explicitly; automated egui tests do not establish native gesture behavior.
