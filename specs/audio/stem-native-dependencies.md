# Stem-separation native dependency audit

Audit date: 2026-10-22. Native inputs: project revision `58a5d98`, `ort-sys` revision `f2a5f73c9dd9ff3682303481cc792bf4669e3e94`, and ONNX Runtime source `402979ac1bef89dcd55add41eaa7277c407599eb` (1.31.0).

## Result

The selected native closure, published advisories, distribution terms and package imports were checked. This is not a clean dependency result: **ONNX 1.22.0 has an applicable published shape-inference advisory**. Its remediation remains open. Native clean-machine deployment checks also remain separate from static package inspection.

This audit closes the previous unspecified native-inventory gap with a source-derived component list, checksums and archive/member evidence. It does not certify unknown vulnerabilities or establish a reproducible-build attestation. Scope is the native closure added by stem separation and its effect on existing distribution. Existing media libraries are recorded below; their implementation was not re-audited.

The [machine-readable evidence](../validation/stem-native-dependencies-2026-10-22.json) contains full source commits, hashes, component platforms, advisory dispositions and actual imports. Private media, model weights, native binaries, caches and machine-specific paths are excluded from tracked evidence.

## Artifact selection and provenance

Read target/feature selection from the [pinned distribution table](https://github.com/pykeio/ort/blob/f2a5f73c9dd9ff3682303481cc792bf4669e3e94/ort-sys/build/download/dist.tsv), not the package version alone. The resolver selects the first best compatible distribution when an exact feature set is absent. Thus Windows CPU use still selects the DirectML-enabled archive.

| Target | Selected archive features | SHA-256 |
| --- | --- | --- |
| `aarch64-apple-darwin` | CoreML | `4033efca0e6c1606bd2ec3870d4cb19d0dba0ad0160dbfb616e66cae969ed68c` |
| `aarch64-pc-windows-msvc` | DirectML | `f71671d263ce8ce7ed0d03c70a75cc1ac0e912d2df40b94844c22e6fcb7ad87f` |
| `aarch64-unknown-linux-gnu` | CPU | `380c1b63eedf5671e7b39e4aa0f6c377ecc351c3a7b01ae708a707e73fb54709` |
| `x86_64-pc-windows-msvc` | DirectML | `f99aaef23f12986abb05924c944d8433521263ad756b462df762a5a982c98405` |
| `x86_64-unknown-linux-gnu` | CPU | `02aac0d3b4927026292088986d7a377c657c88761b1c068be217a67b35447678` |

All five archives were downloaded and verified against those hashes. Every static library embeds `git-branch=rel-1.31.0, git-commit-id=402979a`. Windows archives contain `onnxruntime.lib` and `DirectML.dll`; macOS/Linux archives contain `libonnxruntime.a`. Intel macOS does not link this backend.

The [native builder](https://github.com/pykeio/ort-artifacts/tree/c6d58d10c1470691e154ae401e52307dc61f88a9) bundles the static CMake target closure. Its recipe is evidence of construction, not an attestation that every archived build used that exact builder commit. The embedded runtime source revision matches the [1.31.0 dependency pins](https://github.com/microsoft/onnxruntime/blob/402979ac1bef89dcd55add41eaa7277c407599eb/cmake/deps.txt). Archive members corroborate ONNX/protobuf, Abseil, RE2, FlatBuffers, CPUInfo, CoreMLTools and Arm KleidiAI. Header dependencies are included conservatively rather than inferred absent from missing object names.

CUDA, TensorRT, WebGPU/Dawn, OpenVINO, oneDNN, ROCm/MIGraphX, XNNPACK, mimalloc and 1DS telemetry transports are not selected. The corresponding upstream notice sections do not establish that those components ship. The selected members contain no 1DS/curl/mbedTLS objects, and the builder does not enable telemetry.

## Source-derived native inventory

`All` means the five supported archive targets above. CoreML additions apply only to macOS arm64; KleidiAI applies to arm64 archives. API headers are included even when optimized away. The 20 top-level source projects and protobuf’s embedded UTF-8 validator are listed separately. Full immutable source links and exact commit IDs are in the JSON and packaged `Native-Stem-SOURCE-NOTICE.txt`.

| Component | Version or source pin | Platforms | License |
| --- | --- | --- | --- |
| [onnxruntime](https://github.com/microsoft/onnxruntime/tree/402979ac1bef89dcd55add41eaa7277c407599eb) | `1.31.0` | All | MIT |
| [abseil_cpp](https://github.com/abseil/abseil-cpp/tree/987c57f325f7fa8472fa84e1f885f7534d391b0d) | `20250814.0` | All | Apache-2.0 |
| [onnx](https://github.com/onnx/onnx/tree/2bb50465112feca9003e1ed654d77f01ff1415ca) | `1.22.0` | All | Apache-2.0 |
| [protobuf](https://github.com/protocolbuffers/protobuf/tree/6e1998413a5bca7c058b85999667893f167434bc) | `33.6` | All | BSD-3-Clause |
| [flatbuffers](https://github.com/google/flatbuffers/tree/0100f6a5779831fa7a651e4b67ef389a8752bd9b) | `23.5.26` | All | Apache-2.0 |
| [utf8_range](https://github.com/protocolbuffers/protobuf/tree/6e1998413a5bca7c058b85999667893f167434bc/third_party/utf8_range) | Protobuf v33.6 vendored source | All | MIT |
| [re2](https://github.com/google/re2/tree/6dcd83d60f7944926bfd308cc13979fc53dd69ca) | `2024-07-02` | All | BSD-3-Clause |
| [eigen](https://github.com/eigen-mirror/eigen/tree/1d8b82b0740839c0de7f1242a3585e3390ff5f33) | `3.4 branch 1d8b82b` | All | MPL-2.0 |
| [json](https://github.com/nlohmann/json/tree/9cca280a4d0ccf0c08f47a99aa71d1b0e52f8d03) | `3.11.3` | All | MIT |
| [date](https://github.com/HowardHinnant/date/tree/6e921e1b1d21e84a5c82416ba7ecd98e33a436d0) | `3.0.1` | All | MIT |
| [microsoft_gsl](https://github.com/microsoft/GSL/tree/249146590e322123cd0b378b1f364d6069717687) | `4.2.1` | All | MIT |
| [mp11](https://github.com/boostorg/mp11/tree/0a0b5fb001ce0233ae3a6f99d849c0649e5a7361) | `1.82.0` | All | BSL-1.0 |
| [safeint](https://github.com/dcleblanc/SafeInt/tree/4cafc9196c4da9c817992b20f5253ef967685bf8) | `3.0.28` | All | MIT |
| [dlpack](https://github.com/dmlc/dlpack/tree/5c210da409e7f1e51ddf445134a4376fdbd70d7d) | `5c210da` | All | Apache-2.0 |
| [pytorch_cpuinfo](https://github.com/pytorch/cpuinfo/tree/66ee79c038d70dad9f08705b2c9b3e58f6d8f512) | `66ee79c` | All | BSD-2-Clause |
| [kleidiai](https://github.com/ARM-software/kleidiai/tree/74b1a12d3620c89dae4766de640e064952000f4d) | `1.30.0` | macos-arm64, windows-arm64, linux-arm64 | Apache-2.0 OR BSD-3-Clause |
| [coremltools](https://github.com/apple/coremltools/tree/428d4b2658dfc44194f27f4f36870751be402ff7) | `9.0` | macos-arm64 | BSD-3-Clause |
| [fp16](https://github.com/Maratyszcza/FP16/tree/0a92994d729ff76a58f692d3028ca1b64b145d91) | `0a92994` | macos-arm64 | MIT |
| [psimd](https://github.com/Maratyszcza/psimd/tree/072586a71b55b7f8c584153d223e95687148a900) | `072586a` | macos-arm64 | MIT |
| [microsoft_wil](https://github.com/microsoft/wil/tree/b6ec76a2d9a609897f25a7fa0a0bdf4238e94e35) | `1.0.250325.1` | windows-x64, windows-arm64 | MIT |
| [directx_headers](https://github.com/microsoft/DirectX-Headers/tree/de28d93dfa9ebf3e473127c1c657e1920a5345ee) | `1.613.1` | windows-x64, windows-arm64 | MIT |

| Platform runtime | Inventory and disposition |
| --- | --- |
| DirectML | Both archive DLLs are byte-identical to Microsoft.AI.DirectML 1.15.4 NuGet payloads. PE fixed version is 1.15.4.0. Runtime DLL has Microsoft proprietary terms; source/import headers have separate MIT terms. DAW currently ships neither NuGet DLL. |
| Apple compiler-rt | Local vendor build: Apple clang 21.0.0, clang-2100.3.34.2. Full universal archive hash is recorded in JSON. Arm64 member `os_version_check.c.o` supplies `___isPlatformVersionAtLeast`, present in the inspected release executable. No sanitizer/fuzzer runtime is linked for this dependency. CI selects its own installed Apple toolchain, so this receipt does not pin every future CI runtime. |
| macOS system frameworks | CoreML, Foundation, Accelerate, libc++ and normal application/audio/graphics frameworks are OS-provided, not redistributed in the app. |
| Windows system/runtime DLLs | DirectML, D3D12, DXGI, Windows API DLLs and Visual C++ v14 runtime are external requirements. Complete PE import names are recorded in JSON. |
| Linux system libraries | The inspected x64 package requires the ELF loader, glibc, libm, libgcc_s, libstdc++ and ALSA. Highest observed symbol requirements include GLIBC_2.39 and GLIBCXX_3.4.31. Runtime-loaded graphics/portal libraries remain required as documented in README; `DT_NEEDED` is not a complete inventory of `dlopen` features. |

Existing native media remains governed by its existing notices: Rubber Band 4.0.0 through rubberband-sys, its KissFFT/Speex components, and the MP3/LAME encoder build. Clang/libclang used by bindgen is a build tool, distinct from the small compiler-rt object linked into macOS. OpenSSL in the Cargo build graph is not a new shipped `libssl`/`libcrypto` dependency of the inspected Linux executable. No Rubber Band implementation source was opened.

## Advisory checks

On the audit date, queried OSV by all 20 exact source commits and read the primary published advisories for every component repository. The OSV commit queries returned no matches, but the upstream ONNX advisory below is applicable. Commit indexing and RustSec alone therefore do not cover this closure. Twenty published ONNX/protobuf records were individually disposed of in JSON.

| Candidate | Result |
| --- | --- |
| [GHSA-xrch-8vh7-h656](https://github.com/onnx/onnx/security/advisories/GHSA-xrch-8vh7-h656) | **Affected:** ONNX 1.22.0 is below fixed 1.23.0. The reported effect is incorrect inferred dimensions from integer overflow/narrowing; upstream does not demonstrate memory corruption or code execution. |
| Other 12 ONNX advisories | Fixed by the pinned 1.22.0 or earlier; Python hub/external-data APIs additionally are not used here. |
| Seven protobuf advisories | Old C++ issues are fixed before source v33.6. Java/Python/PHP-only reports do not apply to the linked C++ binary/lite runtime. |
| Other 18 source repositories | No published repository advisory and no OSV commit match returned at check time. This is a dated screening result, not proof of safety. |
| DirectML | NVD keyword query returned no records. DirectML supplied by Windows must follow Microsoft OS servicing; the unused NuGet DLL version is separately identified rather than treated as shipped. |
| Apple compiler-rt | Checked [Xcode 27 security content](https://support.apple.com/en-ca/149040) and NVD. The compiler-rt keyword hit CVE-2023-53182 concerns Linux ACPICA, not the linked Apple availability builtin. IDE, binary-inspection-tool and sanitizer findings are not silently attributed to this builtin. |

The ONNX issue was published October 20. Native symbol inspection confirms the old `MathOpTwoIntegers(string const&, int, int)` signature in the selected macOS archive. Runtime `core/graph/graph.cc` calls the ONNX schema inference/data-propagation functions. A pip ecosystem label does not make a shared C++ implementation irrelevant. This audit did not run an oversized-model exploit or demonstrate an incorrect stem result.

**Required remediation:** use a compatible native runtime build that includes [ONNX fix #8031](https://github.com/onnx/onnx/pull/8031), or backport the fix into a checksum-pinned source build. Add bounded shape-inference regression coverage and repeat exact-history, CPU/CoreML/fallback and package checks. Do not silently replace the native dependency during an audit. The earlier runtime Loop/Scan rejection test covers a different issue and cannot clear this finding.

## Licensing and distribution

The inspected source licenses are permissive except Eigen MPL-2.0, the proprietary optional DirectML payload and pre-existing copyleft media dependencies. No model-weight distribution rights are assumed.

- Keep ONNX MIT and the exact upstream third-party notice file. The repository copy matches the native source revision byte-for-byte: SHA-256 `143764b952fdb1a7c69ce653bfba74a7744d6a8a573bfb73e235fba356c83de3`.
- Keep component copyright/notice texts. Added notices cover FP16, PSIMD, DirectX headers, DirectML source headers, protobuf’s embedded utf8_range validator and the KleidiAI alternative BSD text. Existing umbrella notices remain intact.
- Eigen requires a notice identifying where its covered source is available, including modifications if made. The packaged source notice now gives immutable native component source locations. See [Mozilla MPL distribution guidance](https://www.mozilla.org/en-US/MPL/2.0/FAQ/).
- Apple installed acknowledgments identify LLVM under NCSA. Retain that supplier acknowledgment plus upstream current compiler-rt Apache-2.0/LLVM-exception and legacy MIT/NCSA texts conservatively. The Apple vendor build is identified by its own version/hash; it is not falsely labeled byte-identical to an upstream LLVM tag. Frameworks and SDKs are not copied into the app.
- DirectML NuGet runtime terms allow redistribution within qualifying Windows/Xbox applications, with restrictions distinct from MIT source headers. The current package relies on the system DLL, so no proprietary DLL is added by this audit. If packaging changes to app-local DirectML, retain the exact Microsoft runtime terms and assess that deliverable before release.
- Original DAW code stays BSD-2-Clause. Rubber Band is GPL-2.0-or-later and linked into the combined executable. Apache-2.0 components require the GPLv3-compatible distribution option; [Apache documents GPLv3 compatibility and GPLv2 incompatibility](https://www.apache.org/licenses/GPL-compatibility.html). Provide the exact combined application/dependency source, patches and build materials required by the selected GPL distribution route. Added the complete GPLv3 text beside the existing GPLv2 media text; both are copied by the existing packagers. Notices alone do not satisfy corresponding-source obligations. No release-source offering was created or certified by this audit.

## Package verification

| Deliverable | Verified evidence | Requirements / remaining checks |
| --- | --- | --- |
| macOS arm64 | Inspected local release executable with `otool -L` and `nm`; only system dylibs/frameworks, and the compiler-rt availability symbol. Packager copies all separation notices. | Rebuilt the debug package after notice additions and compared all copied separation/media notices. Do not bundle Apple frameworks. The ORT objects were built for newer macOS than the application’s nominal 11.0 deployment setting; older-macOS behavior is not established by successful linking on the audit host. |
| Intel macOS | Cargo target gate omits the native separation graph. | No ONNX/DirectML runtime needed for separation; existing app dependencies still apply. Native Intel hardware was not used. |
| Windows x64 | Downloaded completed CI artifact for `9fcbae5`; native inputs are unchanged at audit revision. PE imports include DirectML/DMLCreateDevice1 and MSVCP140/VCRUNTIME140 variants. The package contains no ONNX or DirectML DLL. | OS must export DMLCreateDevice1 and supply D3D12; install matching-or-newer [Visual C++ v14 Redistributable](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist). [Microsoft API requirements](https://learn.microsoft.com/en-us/windows/win32/api/directml/nf-directml-dmlcreatedevice1) are stronger evidence than “DirectML exists since Windows 10 1903.” Verify on the oldest supported clean Windows image. |
| Linux x64 | Downloaded completed CI package; inspected `DT_NEEDED` and version requirements. No external ONNX, CUDA, OpenSSL or other ML shared library is required by that executable. | Retain Ubuntu 24.04/compatible baseline, ALSA/C/C++ runtime and documented desktop dependencies. Do not copy host system libraries into the ZIP without auditing those new redistributions. |
| Windows/Linux arm64 | Downloaded and checksum-verified the selected native archives; object inventory confirms CPU/KleidiAI and Windows DirectML selection. | No arm64 application package or clean-machine launch was tested. Use target-native CI before claiming release deployment support. |
| CLI | Uses the shared separation backend; native selection is identical for that target. | Current desktop packagers do not distribute CLI. A separately distributed CLI needs its own copy of applicable project/media/separation notices and the same native prerequisites. |

The inspected CI artifacts came from [run 39808533403](https://github.com/fecabrera/daw/actions/runs/39808533403), commit `9fcbae5`. Their binary hashes and imports are recorded in JSON. No foreign executable was run. The audit does not treat a successful developer-machine test as a clean-machine deployment test.

## Recheck procedure

1. Read the locked `ort-sys` target feature graph and distribution table; account for feature fallback and environment overrides such as `ORT_LIB_PATH`. This receipt covers default pinned downloads, not arbitrary user overrides.
2. Download each selected archive and compare SHA-256 before reading it. List members with `llvm-ar t`; inspect defined/undefined symbols and embedded runtime source revision without executing supplied code.
3. Resolve that source revision’s `cmake/deps.txt` and target closure, including headers, vendored subcomponents and platform-only additions. Save exact commits, licenses and source URLs. Compare archive members and original vendor DLL hashes.
4. Query current OSV and primary component advisories; review affected API/language and fixed ranges. Use MSRC/Apple/OS vendor servicing records for external platform runtimes. Do not infer safety from missing Cargo findings or empty commit queries.
5. Inspect built executables with `llvm-readobj --coff-imports` on PE, `llvm-readobj --needed-libs --version-info` on ELF, and `otool -L`/`nm` on Mach-O. Compare the actual package, not only the build cache.
6. Verify notices, corresponding source delivery and runtime prerequisites for each distributed executable. Native OS/architecture smoke tests remain required for deployment claims.
