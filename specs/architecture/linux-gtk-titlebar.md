# Linux GTK window host

## Scope

The normal `daw-desktop` executable runs the shared `DawUi` in a GTK
`ApplicationWindow` and `GLArea` through `egui_glow` on Linux. GTK 4.8 or newer
and desktop OpenGL 3.3 are required. The macOS and Windows entry points retain
eframe and their existing adapters. The headless CLI has no GTK dependency.
The `gtk_titlebar` example and empty `gtk-titlebar` feature remain compatibility
aliases for the former trial commands; neither is required for normal startup.

`GtkWindowControls` supplies actual widgets at both ends of an overlay and
follows GTK's decoration layout. Custom mode reserves their measured widths and
height in the shared header. Empty control groups reserve no width, including
their CSS padding, so the menu/title/view group uses the same left margin as
the Windows custom header. Custom mode keeps an empty titlebar widget for GTK's client frame and resize edges.
Native mode clears the custom titlebar with `set_titlebar(None)` so GTK or the
window manager supplies its regular titlebar, above the ordinary DAW menu/view
toolbar. No explicit `GtkHeaderBar` is installed. The UI exposes the Linux titlebar selector only
after this host registers its capability. Fresh Linux settings default to Custom; saved mode choices are retained.

GTK objects, GL contexts and painter lifetime stay on the main thread. The
painter is destroyed with a current GLArea context before unloading its GL
library. Worker repaint callbacks publish an atomic deadline; a main-thread
16 ms timer queues rendering when due. Existing audio and project workers remain
shared. GTK input mapping covers pointer buttons/motion, modifiers, keys,
committed IM text, scroll, clipboard text and local file hover/drop. Title/unused
header gestures use shared hit-testing for native dragging and double-click
maximize. Root close requests enter the existing unsaved-project guard before
GTK closes. Titlebar switching is deferred until after the current snapshot. The host hides
and unrealizes the window before changing its titlebar, then presents it again,
retaining the session and maximized state. Managed texture images are cached on
the CPU, including partial updates, and uploaded to each replacement GL painter.
Freed images leave the cache.

## Preferences and packaging

Preferences are written as a JSON document with `settings` and `window` fields
to `daw/settings.json` under GLib's user configuration directory. Loading first
uses that document. If absent, it reads `daw/app.ron` under the XDG data directory,
imports the `daw.settings` JSON value and compatible saved window geometry, and
leaves all original keys/files unchanged. The former GTK trial JSON is a fallback
when no established application preferences are available. The common settings
parser retains existing legacy audio-quality migration. Size, maximized state
and fullscreen state persist; position remains controlled by the compositor.
Writes use temporary-file replacement, at most once per second during ordinary
rendering, with a forced save on an accepted close. Smoke tests bypass preference
loading and saving. Tests use isolated temporary files for all migration cases.

Linux packaging builds the GTK binary without feature flags and includes
`LINUX-REQUIREMENTS.txt`. GTK C libraries remain system dependencies; no GTK C
binaries are bundled. Rust dependency notices use the existing desktop notice
collection. Linux CI installs GTK development headers and runs the real launcher
under Xvfb for Custom/Native surface-switch checks, plus the headless font-license
command. Neither the CLI nor other platforms requires Linux GTK headers.

## Validation

```sh
cargo run -p daw-desktop
cargo run -p daw-desktop -- --smoke-test
cargo clippy -p daw-desktop --all-targets --locked --offline -- -D warnings
cargo test -p daw-ui titlebar::tests --locked --offline
```

The smoke test renders the DAW, switches Custom → Native → Custom, checks GTK
custom-titlebar presence and control visibility, verifies surface recreation,
saves `work/gtk-titlebar/native.png` and `work/gtk-titlebar/custom.png`, and closes.
It does not load or save preferences or open a user project. Render failure
returns a failing process exit code. The screenshot is an ignored scratch output.

The host does not establish parity for accessibility of the egui content,
full IM preedit/composition, image clipboard, or additional viewports. GTK owns
accessibility of its window controls. Physical key mapping and held-key behavior
need native verification. Native button clicks, dragging, resize edges, fractional
scaling, unsaved-close interaction, file drops, dialogs and audio device playback
remain native checks on Wayland and X11.
See the [validation record](../validation/README.md) for executed checks.
