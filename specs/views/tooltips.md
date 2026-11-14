# Tooltip presentation

This is the canonical contract for first-party tooltips in the desktop app and
shared CLAP/VST3 editors. It supersedes older requirements for instructional
hover text, modulation summaries and hover-only diagnostic reports. It does not
change interactions, audio, parameter identities, saved settings or Undo.

## Types and formats

Classify a surface by its behavior, including callers of shared helpers.

| Type | Required content | Example |
| --- | --- | --- |
| Editable parameter: knob, slider, fader, dropdown, numeric input, checkbox or saved choice | Parameter and current formatted value, or just the value when its identity is clear. Retain units and useful source/band identity. | `Gain: −3.0 dB`; `Trigger: Gate`; `Mixed` |
| Action: button, power, audition, add/remove, disclosure, navigation or reset command | Action name, plus the actual applicable shortcut in parentheses when available. | `Play (Space)`; `Show Timeline (⌘1)`; `Show modulation: Frequency` |
| Read-only measurement: meter, plot or monitor | Short identity and actual current/held values with units. | `L: −6.0 dBFS · Peak: −2.0 dBFS` |
| Named object or elided identity | Full current name or path. | Complete source path or track name |
| Status or unavailable surface | Short current status or reason. | `Analyzing transients`; `Requires a regular layer` |
| Composite editing region or graph handle | Short operation/region identity, or the current editable values. Retain meaningful units and band identity. | `Trim clips`; `Band 2 · Frequency: 800 Hz · Gain: +2.0 dB · Q: 1.20` |
| Detailed diagnostic report | Not a tooltip. Use an explicit Details action and the shared read-only report dialog. | Memory accounting or the complete transient-analysis error |

A meter remains a reading when clicking it resets a peak. Keep the reset action
in its accessibility information and guide. A modulation dot is a button that
opens controls: its tooltip names that action, not a route summary. A graph
handle that edits several parameters shows their values, not a gesture lesson.

Do not add gesture instructions, default values, bounds, lists of alternatives,
implementation explanations or paragraphs to parameter/action tooltips. Keep
operating instructions in the relevant task guide. Do not shorten notices,
validation errors, menu captions or visible advice merely because they contain
prose; they are separate surfaces. Full identities and values may be long: do
not truncate them or impose a character limit. Qualifiers such as `Estimated`
and `approx.` remain when needed for accuracy.

## Values, shortcuts and accessibility

Use the owning parameter formatter, after applying input where the widget edits
its displayed value. Preserve silence, signed values, units, mode labels, mixed
selections, unavailable identities, exact beat durations and preview values.
Never substitute the first selected value for a mixed selection. Dropdown hover
uses its current displayed selection, including when disabled. Do not parse
hover text to recover numeric bounds or state.

Use registered/contextual action shortcuts and the shared platform formatter.
Do not invent a shortcut for an action or copy a binding from another scope.
Play/Pause uses Space; Stop/Return to origin and Loop do not inherit it. Keep
native titles, key equivalents, dispatch and availability guards unchanged.
Disabled action hover still names the action and optional shortcut; needed
reasons belong in concise status, accessible descriptions or task guidance.

Hover text and accessible identity are separate. Retain widget IDs, labels,
roles, enabled/selected state, values and reset actions. Adding a shortcut or
contextual hover identity must not rename the accessible parameter or a host
parameter. Icon-only controls retain an accessible name.

## Shared implementation and lifecycle

`unit-ui::tooltips` owns small parameter/action text builders. DAW action owners
format shortcuts through `toolbars::action_tooltip` and
`format_menu_shortcut`. Native adapters use the same action metadata.
`menus::Dropdown` exposes its selected caption as a value tooltip. Shared bodies
remain the single rendering path for built-in and standalone effects.

Shared knob tooltips contain only the parameter name and the value produced by the
existing parameter formatter, separated by a colon. Omit gesture instructions and
appended modulation summaries from the knob tooltip. Use one shared tooltip layout for
hover and drag: keep the name and colon left-aligned and the formatted value right-
aligned in one row with at least an 8-point gap. Measure both without wrapping,
reserve at least 120 points of content width, and grow to fit longer names or values.
Return to the same minimum for short values instead of inheriting the knob’s narrow
width or a previous popup width. Show it on normal hover and force it open during an
enabled, focused knob drag, including when the pointer is outside the widget. Build
the text after applying input so it follows the current value. Stop forced display on
release, disabled editing or focus loss; retain normal hover behavior afterward.
Modulation-marker gestures do not force the knob tooltip; they use the separate
show/open action tooltip. Implement this once in `unit-ui::knobs` for
track/layer/Output/Editor controls, source/modulator/effect controls,
scaled/choice/amount knobs and standalone plugin bodies. Tooltip rendering must not
emit extra parameter edits or change capture, reset, scale or gesture ownership.

A contextual `tooltip_name` changes only hover identity. Keep modulation-marker numeric `range`, `live` and active data independent of text.

No tooltip may emit a project edit, dirty flag, Undo entry, host gesture, worker
job or audio preparation. Retain capture, Shift/modifier behavior, cancellation,
release-only commits, bypass masking, resets and meter bounds. Consume cached
settings and published readings on UI/control threads. Add no callback work,
locks, I/O, allocation or final-owner retirement.

## Details reports

The processing-rate/buffer status label has a concise estimated-memory hover.
Its adjacent Details action opens the existing full accounting table, including
budgets and caveats, in the shared dialog with bounded vertical scrolling.
Transient failure status stays short; its Details action opens the complete
retained worker error. Never replace stored diagnostic data with its summary.

Report visibility is transient UI state. Only explicit activation opens it.
Existing modal/job/focus guards apply; editing is blocked while the report is
open, playback continues, and Close or Escape dismisses it. Opening/closing a
report must not modify the project, history or worker result. Reports do not
start a new accounting traversal on hover or create a diagnostics subsystem.

## Coverage and acceptance

Audit producers and their callers, including shared helpers, parameter wrappers,
custom graphs/meters, catalogs, selectors, statuses, disabled hover, native
setters and implicit elision. Classify each as changed, already compliant or
outside tooltip ownership. OS/host-owned tooltips are outside first-party text
ownership. A lexical search is not a count of runtime controls.

Keep semantic hover tests, shared knob lifecycle tests, numeric marker bounds,
accessible-identity checks, Details reachability/close/guard checks and mapped
hover parity across every effect editor. Idle `PointerGone` mesh parity does not
cover tooltips. Preserve existing gesture/host-queue tests. Run the repository
workspace checks and record native app, host and accessibility limits separately.
See the [acceptance inventory](../acceptance/tooltips.md) and
[validation record](../validation/README.md) for coverage and executed checks.
