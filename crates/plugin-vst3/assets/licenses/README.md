# VST3 binding notices

The runtime uses `vst3` 0.3.0 and `com-scrape-types` 0.1.1, each declared MIT OR Apache-2.0. Original texts are retained here. These are the permissive coupler-rs bindings, not `vst3-sys`.

- `vst3`: texts from its published Cargo package; source [coupler-rs/vst3-rs](https://github.com/coupler-rs/vst3-rs).
- The generated `vst3` bindings at package revision `2d86c810be3e22367f92438800cd96a3e9cd3a37` use the Steinberg `pluginterfaces` fork at `31d6eeba6daaa3e2a8bfbe3e7a90ca0b7fbfbc1c`. Its MIT text is included separately. [Bundled notice](Steinberg-pluginterfaces-MIT.txt) · [Upstream license](https://raw.githubusercontent.com/coupler-rs/vst3_pluginterfaces/31d6eeba6daaa3e2a8bfbe3e7a90ca0b7fbfbc1c/LICENSE.txt).
- `com-scrape-types`: its package omits the workspace license files. Texts are retained from the repository root at published `.cargo_vcs_info.json` revision `33f1859fbdcfa247c1fdd17d565cb5285860e4ce`, path `com-scrape-types`.

Pinned package versions/checksums remain in the workspace lockfile. SDK sources/validator stay in ignored scratch checkouts, with their own original notices. DSP and other runtime notices are required separately; this directory is not a complete production distribution inventory.
