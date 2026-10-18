# Macro

A Macro provides a named Value knob that can control several parameters. Add Macro from Modulators in the Instrument components catalog to a layer or Main. Use its Plus strip or add Macro again to create more independent controls.

## Set up a Macro

1. Set Value from 0–100%; the default is 0%.
2. Open a receiving unit's modulation controls.
3. Select the Macro as Source and choose a destination and signed Amount.
4. Repeat on other destinations to control them together.

Double-click a Macro name to rename it. Enter accepts a nonempty name. Escape or focus loss cancels. Names appear in source lists and Value destinations. Duplicate names are allowed.

Header power and individual strip power are independent. Off supplies a neutral contribution. At Amplitude with full positive Amount, Value 0 silences the signal and Value 100% reaches unity. On additive destinations, the receiving Amount defines how far Value moves the parameter.

## Modulate a Macro

Open the Macro header's secondary-color dot. Select a shared Envelope, LFO, or another Macro and choose the wanted strip's Value destination. Amount spans −100% to +100%. Incoming movement combines with the base value within 0–100%. Self-connections and feedback loops are unavailable. Per note Envelopes cannot feed Macro.

The knob shows the base value; the live dot shows the processed value. Sources and destinations stay within the same MIDI instrument, including Main. A Macro does not create or prolong notes.

## Remove or move

Right-click a strip's empty space, Value knob, or power control and choose Remove macro. Plus restores the first retained strip before adding another. Removing the whole component retains all its strips for restoration.

Drag the component header to move or copy its strips between layers and Main. An occupied destination merges the strips. Individual strip transfers are not available. Names, settings, and connections save with the project. See [unit editing](../editing/units.md).
