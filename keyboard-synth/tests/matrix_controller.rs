use keyboard_synth::{
    all_mapped_notes, midi_note_for_key, MatrixKey, MidiAction, ScannerState,
    DEFAULT_VELOCITY,
};
use std::time::Duration;

#[test]
fn maps_organ_keyboard_matrix_to_44_midi_notes() {
    let notes = all_mapped_notes();

    assert_eq!(notes.len(), 44);
    assert_eq!(notes.first().copied(), Some(29));
    assert_eq!(notes.last().copied(), Some(72));
    assert_eq!(
        notes,
        [
            29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46,
            47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64,
            65, 66, 67, 68, 69, 70, 71, 72,
        ]
    );
}

#[test]
fn ignores_unwired_positions_outside_the_44_key_range() {
    assert_eq!(midi_note_for_key(MatrixKey { bus: 0, note_col: 0 }), None);
    assert_eq!(midi_note_for_key(MatrixKey { bus: 0, note_col: 4 }), None);
    assert_eq!(midi_note_for_key(MatrixKey { bus: 4, note_col: 1 }), None);
    assert_eq!(midi_note_for_key(MatrixKey { bus: 4, note_col: 11 }), None);
}

#[test]
fn debounce_emits_note_on_after_stable_press_and_note_off_after_stable_release() {
    let mut state = ScannerState::new(
        Duration::from_millis(8),
        Duration::from_millis(80),
        DEFAULT_VELOCITY,
    );
    let key = MatrixKey {
        bus: 2,
        note_col: 6,
    };

    assert_eq!(state.update(Duration::from_millis(0), &[key]), []);
    assert_eq!(state.update(Duration::from_millis(7), &[key]), []);
    assert_eq!(
        state.update(Duration::from_millis(8), &[key]),
        [MidiAction::NoteOn {
            note: 54,
            velocity: DEFAULT_VELOCITY
        }]
    );

    assert_eq!(state.update(Duration::from_millis(20), &[]), []);
    assert_eq!(state.update(Duration::from_millis(99), &[]), []);
    assert_eq!(
        state.update(Duration::from_millis(100), &[]),
        [MidiAction::NoteOff { note: 54 }]
    );
}

#[test]
fn brief_release_bounce_does_not_cut_sustained_note() {
    let mut state = ScannerState::new(
        Duration::from_millis(8),
        Duration::from_millis(80),
        DEFAULT_VELOCITY,
    );
    let key = MatrixKey {
        bus: 2,
        note_col: 6,
    };

    assert_eq!(state.update(Duration::from_millis(0), &[key]), []);
    assert_eq!(
        state.update(Duration::from_millis(8), &[key]),
        [MidiAction::NoteOn {
            note: 54,
            velocity: DEFAULT_VELOCITY
        }]
    );
    assert_eq!(state.update(Duration::from_millis(20), &[]), []);
    assert_eq!(state.update(Duration::from_millis(60), &[key]), []);
    assert_eq!(state.update(Duration::from_millis(120), &[key]), []);
}
