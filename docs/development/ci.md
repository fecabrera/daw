# GitHub Actions builds

[Development guides](README.md)

The [Test and build workflow](../../.github/workflows/build-and-test.yml) runs on pushes, pull requests, and manual dispatches. It calls the [platform workflow](../../.github/workflows/platform.yml) separately for Windows, macOS, and Linux. Each platform has separate `test` and `build` jobs. Tests run formatting, strict workspace/all-target Clippy, workspace tests, and the headless CLI smoke checks with the pinned Rust toolchain and locked dependencies. Build starts after that platform's tests pass, builds and packages release binaries on a fresh worker, and uploads artifacts retained for 14 days. A failure on one platform does not block or cancel another platform's build. Both jobs use the same [native build setup action](../../.github/actions/setup-build/action.yml).

| Platform | Runner | Download |
| --- | --- | --- |
| Windows x64 | `windows-2025` | `DAW-windows-x64`: portable `daw-desktop.exe`, project license, and dependency notices |
| macOS Apple Silicon | `macos-15` | `DAW-macos-arm64.dmg` |
| Linux x64 | `ubuntu-24.04` | `DAW-linux-x64.zip` |

Download artifacts from the workflow run's summary. Windows downloads as a ZIP containing the executable and licenses; it is not an installer. The disk image and Linux ZIP upload directly as files. These builds do not sign binaries, notarize the macOS app, or publish a GitHub release. The Linux package targets Ubuntu 24.04 and compatible systems; it is not a static binary for every distribution. Native UI and audio-device checks remain separate from CI.

Workers also upload command logs as `logs-test-<OS>-<architecture>` and `logs-build-<OS>-<architecture>`, including failed jobs, with 14-day retention. Job summaries show step results, a download link, and the last command output on failure. These artifacts contain formatting, Clippy, test, CLI, or packaging output; checkout and external setup-action details remain in the raw runner logs. If the job page shows no steps, retrieve those logs with `gh run view <run-id> --log` from the repository ([GitHub log documentation](https://docs.github.com/en/actions/how-tos/monitor-workflows/use-workflow-run-logs)). A skipped build job has no worker or build logs.
