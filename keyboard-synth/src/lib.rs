use std::time::Duration;

pub const BUS_COUNT: usize = 5;
pub const NOTE_COUNT: usize = 12;
pub const DEFAULT_VELOCITY: u8 = 110;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MatrixKey {
    pub bus: usize,
    pub note_col: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MidiAction {
    NoteOn { note: u8, velocity: u8 },
    NoteOff { note: u8 },
}

#[derive(Clone, Copy, Debug)]
struct DebounceSlot {
    raw: bool,
    stable: bool,
    changed_at: Duration,
}

impl Default for DebounceSlot {
    fn default() -> Self {
        Self {
            raw: false,
            stable: false,
            changed_at: Duration::ZERO,
        }
    }
}

#[derive(Debug)]
pub struct ScannerState {
    slots: [[DebounceSlot; NOTE_COUNT]; BUS_COUNT],
    press_debounce: Duration,
    release_debounce: Duration,
    velocity: u8,
}

impl ScannerState {
    pub fn new(press_debounce: Duration, release_debounce: Duration, velocity: u8) -> Self {
        Self {
            slots: [[DebounceSlot::default(); NOTE_COUNT]; BUS_COUNT],
            press_debounce,
            release_debounce,
            velocity,
        }
    }

    pub fn update(&mut self, now: Duration, raw_pressed: &[MatrixKey]) -> Vec<MidiAction> {
        let mut actions = Vec::new();

        for bus in 0..BUS_COUNT {
            for note_col in 0..NOTE_COUNT {
                let key = MatrixKey { bus, note_col };
                let Some(note) = midi_note_for_key(key) else {
                    continue;
                };

                let raw = raw_pressed.contains(&key);
                let slot = &mut self.slots[bus][note_col];
                if raw != slot.raw {
                    slot.raw = raw;
                    slot.changed_at = now;
                }

                let debounce = if slot.raw {
                    self.press_debounce
                } else {
                    self.release_debounce
                };

                if slot.stable != slot.raw && now.saturating_sub(slot.changed_at) >= debounce {
                    slot.stable = slot.raw;
                    if slot.stable {
                        actions.push(MidiAction::NoteOn {
                            note,
                            velocity: self.velocity,
                        });
                    } else {
                        actions.push(MidiAction::NoteOff { note });
                    }
                }
            }
        }

        actions
    }
}

pub fn midi_note_for_key(key: MatrixKey) -> Option<u8> {
    match key.bus {
        0 if (5..=11).contains(&key.note_col) => Some(29 + (key.note_col as u8 - 5)),
        1..=3 if key.note_col < NOTE_COUNT => {
            Some(36 + ((key.bus as u8 - 1) * 12) + key.note_col as u8)
        }
        4 if key.note_col == 0 => Some(72),
        _ => None,
    }
}

pub fn all_mapped_notes() -> Vec<u8> {
    let mut notes = Vec::new();
    for bus in 0..BUS_COUNT {
        for note_col in 0..NOTE_COUNT {
            if let Some(note) = midi_note_for_key(MatrixKey { bus, note_col }) {
                notes.push(note);
            }
        }
    }
    notes
}
