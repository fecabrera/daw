# Live MIDI input

The desktop adapter owns `midir` device discovery and connections. Engine,
project and CLI remain independent of MIDI devices. Preferences store one device
identity, an optional channel filter and a recording offset bounded to ±200 ms.
Ambiguous identities are rejected. Reconnection requires an explicit selection.

The native callback validates supported three-byte channel messages and pushes
fixed packets into an 8192-entry ring. It performs no allocation, file I/O or
locking. Monotonic arrival timestamps and one sequence counter order input and
control changes. A broker processes physical key ownership, FIFO repeated pitches,
sustain and panic independently of painting. Physical releases after panic cannot
release a newer press. CC120/123 release sounding notes.

Attach the input broker only after the current complete project renderer is published. The startup renderer has no tracks; revision equality alone does not establish readiness, and early route commands cannot survive its replacement. Keep the unique MIDI producer available until that publication.

The output sender maps arrival time to the project-rate output clock through a
bounded atomic snapshot read. Future scheduling is capped at 20 ms. Capture uses
mapped project frames, the configured offset and monotonic clamping; monitoring
uses the immediate scheduled boundary. The receiver clamps capture to the take
origin. Hardware latency is not inferred. The audio callback consumes bounded
Copy commands and publishes fixed receipts through a 32768-entry ring. Overflow
sets an independent atomic panic epoch; release does not depend on queue space. Route changes survive panic epochs in queue order; stale note and capture commands do not. A focus change must release voices without clearing selected/armed input targets.

Resolve valid R-enabled MIDI tracks in project order. If none exist, use the
primary selected MIDI track. Selection of other track kinds yields no target.
Each MIDI track prepares 64 live voice slots on a worker. Live and scheduled
voices share the saved instrument processing root, layer/Main effects and buses.
Stopped monitoring advances a separate clock without moving the playhead. After a stopped or paused seek, clear reconstructed arrangement voices, effect tails and alignment buffers on the worker before live monitoring. Preserve the cache snapshots. Play after such a seek reconstructs exact playback history; an ordinary pause/resume retains its active history.
Transport-cache snapshots omit live reserves. The output worker must recreate these reserves after replacing its renderer with a restored snapshot, then reconcile memory admission before publication. Offline export does not allocate live reserves. Voice stealing is bounded and
old token releases cannot release a replacement. Panic forces a 5 ms release.

Sampler preparation unions requested live pitches with saved pitches on workers.
A target without every required exact enabled-layer pitch suppresses that attack;
no pitch transform runs in the callback and held attacks are not replayed late.
Compatible renderer updates preserve held voices and stable target identities.

Physical keyboard timing, audible latency, device loss and native Windows/Linux
behavior require platform checks. Synthetic checks do not establish these results.
