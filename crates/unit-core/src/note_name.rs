//! MIDI note naming shared by the piano roll and sampler metadata editor.
pub fn note_name(pitch: u8) -> String {
    format!(
        "{}{}",
        [
            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"
        ][usize::from(pitch % 12)],
        i16::from(pitch) / 12 - 1
    )
}
pub fn natural_note(text: &str, sharp: bool) -> Option<u8> {
    let text = text.trim();
    let mut chars = text.chars();
    let letter = chars.next()?.to_ascii_uppercase();
    let step = match letter {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    if sharp && matches!(letter, 'E' | 'B') {
        return None;
    }
    let octave: i16 = chars.as_str().parse().ok()?;
    let value = (i32::from(octave) + 1) * 12 + step + i32::from(sharp);
    (0..=127).contains(&value).then_some(value as u8)
}
