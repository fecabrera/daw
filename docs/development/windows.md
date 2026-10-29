# Windows

[Development guides](README.md)

These steps target Windows x64 with the MSVC Rust toolchain. Install the following software before building:

| Software | Required components and purpose |
| --- | --- |
| [Rustup](https://rust-lang.org/tools/install/) | Installs Rust and Cargo. Select the MSVC toolchain. The repository pins the build toolchain in [rust-toolchain.toml](../../rust-toolchain.toml). |
| [Visual Studio Build Tools](https://learn.microsoft.com/en-us/cpp/overview/acquire-msvc) | Select **Desktop development with C++**, including MSVC x64/x86 build tools and a Windows SDK. These provide `cl.exe`, `link.exe`, headers, and libraries. You can also add this workload to an existing Visual Studio installation. |
| [LLVM](https://github.com/llvm/llvm-project/releases/latest) | Download the **Windows x64 installer** from the official release page. It must include Clang and `libclang.dll`, which bindgen uses to generate the Rubber Band Rust bindings. Install LLVM separately from Rustup. |

[Git for Windows](https://git-scm.com/downloads/win) is needed if you clone the repository. [Python 3](https://www.python.org/downloads/windows/) is needed only for the demo generator and CLI smoke checks. Rubber Band and LAME sources are bundled, so you do not need separate installations of these audio libraries.

The desktop requires an OpenGL-capable graphics driver on Windows. No separate Windows App SDK installation is needed. Include the font, icon, UI dependency, and media notices under `crates/ui/assets` and `crates/media/assets/licenses` when distributing the application.

## Configure libclang

1. Install LLVM and check that `libclang.dll` exists, usually at `C:\Program Files\LLVM\bin\libclang.dll`.
2. Open Windows **Editor environment variables for your account**. Add a user variable named `LIBCLANG_PATH` with value `C:\Program Files\LLVM\bin`. If LLVM is installed elsewhere, use the folder that contains the DLL. Set the folder, not the DLL file.
3. Fully close and reopen your terminal and IDE so they receive the new environment variable.

See the [bindgen requirements](https://rust-lang.github.io/rust-bindgen/requirements.html#windows) for LLVM installation details. Setting `LIBCLANG_PATH` does not install the DLL.

## Build from a terminal

1. Open **x64 Native Tools Command Prompt for Visual Studio**. This configures the compiler, linker, and Windows SDK environment.
2. Change to the repository root.
3. Install the pinned Rust toolchain, then build and run:

   ```bat
   rustup show
   cargo build -p daw-desktop
   cargo run -p daw-desktop
   ```

The debug executable is `target\debug\daw-desktop.exe`. For an optimized build, run `cargo build --release -p daw-desktop`; the executable is `target\release\daw-desktop.exe`. Build the CLI with `cargo build -p daw-cli`.

If you prefer to set libclang only for the current shell, use `set "LIBCLANG_PATH=C:\Program Files\LLVM\bin"` in Command Prompt, or `$env:LIBCLANG_PATH = 'C:\Program Files\LLVM\bin'` in PowerShell, before running Cargo.

## Package a ZIP

1. Configure the build tools and libclang as above.
2. Run the Windows packager from the repository root:

   ```powershell
   powershell.exe -NoProfile -ExecutionPolicy RemoteSigned -File tools/package_windows.ps1 release
   ```

   Use `debug` for a debug build. The default is `debug`, matching the macOS packager. `RemoteSigned` applies only to this PowerShell process; it does not change the saved execution policy.
3. Take `target\release\DAW.zip` or `target\debug\DAW.zip`. If Cargo uses a different target directory or build target, the ZIP is placed beside the built executable. The script prints its full path.
4. Extract the ZIP and run `DAW\daw-desktop.exe`. Both profiles launch as Windows GUI applications without opening a terminal. Keep `DAW\LICENSE.txt` and the `DAW\licenses` folder with the executable.

The script builds the desktop with the lockfile, includes the same font, icon, UI, media, oscillator, and EQ notices as the macOS bundle, and replaces an existing ZIP only after packaging succeeds. It includes no CLI, projects, recordings, build cache, or debug symbols. The ZIP is unsigned. It does not include the Microsoft C++ runtime: destination PCs need the [Microsoft Visual C++ v14 Redistributable for x64](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist) and an OpenGL-capable graphics driver. The selected native inference archive also imports system DirectML (`DMLCreateDevice1`) and Direct3D 12 even though separation uses CPU. Those Windows components are not included in the ZIP; older Windows installations must provide the required APIs. See the [native dependency audit](../../specs/audio/stem-native-dependencies.md) for the verified imports and deployment limits. Rust, LLVM, Rubber Band, and LAME installations are not needed to run it.

## Troubleshoot build errors

| Error | Check and fix |
| --- | --- |
| `linker link.exe not found` | In the developer command prompt, run `where.exe link` and `where.exe cl`. If either is missing, add or repair the C++ workload through Visual Studio Installer. Adding only the linker folder to `PATH` does not configure SDK headers and libraries. |
| `rubberband-sys`: `Unable to find libclang` | Check that `libclang.dll` exists and that `LIBCLANG_PATH` points to its containing folder. Install Windows x64 LLVM if the DLL is missing. Restart the build application after changing Windows environment variables. |

The `CARGO_PROFILE_DEV_BUILD_OVERRIDE_DEBUG` suggestion adds backtrace detail; it does not fix missing build tools or libclang. Windows build, native UI, and audio-device verification remain separate from these setup instructions.
