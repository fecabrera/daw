# Native editor dependency notices

Exact sources resolve through the workspace lockfile. Production editors use egui-baseview and baseview without the nice-plug runtime. The existing egui/OpenGL notices remain in `crates/ui/assets/licenses`; Outfit and fallback-font notices remain with their assets and the egui dependency. Final distribution inventory is a Phase 5 gate.

- baseview 0.3.7: MIT OR Apache-2.0; source: https://github.com/RustAudio/baseview; notices: baseview-LICENSE-APACHE.txt, baseview-LICENSE-MIT.txt
- egui-baseview 0.7.2: MIT OR Apache-2.0; source: https://codeberg.org/RustAudio/egui-baseview; notices: egui-baseview-LICENSE-APACHE.txt, egui-baseview-LICENSE-MIT.txt
- copypasta 0.10.2: MIT / Apache-2.0; source: https://github.com/alacritty/copypasta; notices: copypasta-LICENSE.apache2.txt, copypasta-LICENSE.mit.txt
- keyboard-types 0.8.3: MIT OR Apache-2.0; source: https://github.com/rust-windowing/keyboard-types; notices: keyboard-types-LICENSE-APACHE.txt, keyboard-types-LICENSE-MIT.txt
- open 5.4.5: MIT; source: https://github.com/Byron/open-rs; notices: open-LICENSE.md.txt
- is-docker 0.2.0: MIT; source: https://github.com/TheLarkInn/is-docker; notices: is-docker-LICENSE.txt
- is-wsl 0.4.0: MIT; source: https://github.com/TheLarkInn/is-wsl; notices: is-wsl-LICENSE.txt
- x11-clipboard 0.9.3: MIT; source: https://github.com/quininer/x11-clipboard; notices: x11-clipboard-LICENSE.txt

- objc2-open-gl 0.3.2: Zlib OR Apache-2.0 OR MIT; source: https://github.com/madsmtm/objc2; notices: objc2-open-gl-LICENSE.md.txt and objc2-open-gl-SOURCE-NOTICE.txt
