use std::{
    collections::BTreeSet,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

use alsa::seq::{Addr, EvCtrl, EvNote, Event, EventType, PortCap, PortSubscribe, PortType, Seq};
use alsa::Direction;
use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use keyboard_synth::{
    all_mapped_notes, midi_note_for_key, MatrixKey, MidiAction, ScannerState, DEFAULT_VELOCITY,
};
use rppal::gpio::{Gpio, InputPin, OutputPin};
use signal_hook::consts::signal::{SIGINT, SIGTERM};
use signal_hook::flag;

const BUSES: [(&str, u8); 5] = [
    ("BUS1", 5),
    ("BUS2", 6),
    ("BUS3", 13),
    ("BUS4", 19),
    ("BUS5", 26),
];

const NOTES: [(&str, u8); 12] = [
    ("C", 12),
    ("C#", 16),
    ("D", 20),
    ("D#", 21),
    ("E", 17),
    ("F", 27),
    ("F#", 22),
    ("G", 23),
    ("G#", 24),
    ("A", 25),
    ("A#", 18),
    ("B", 4),
];

#[derive(Parser)]
#[command(version, about = "Synth DIY organ keyboard controller")]
struct Cli {
    #[arg(long, default_value = "FLUID Synth")]
    target: String,

    #[arg(long, default_value_t = 0)]
    channel: u8,

    #[arg(long, default_value_t = 16)]
    program: u32,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Run {
        #[arg(long, default_value_t = DEFAULT_VELOCITY)]
        velocity: u8,

        #[arg(long, default_value_t = 8)]
        press_debounce_ms: u64,

        #[arg(long, default_value_t = 80)]
        release_debounce_ms: u64,

        #[arg(long, default_value_t = 200)]
        settle_us: u64,

        #[arg(long, default_value_t = 1000)]
        scan_sleep_us: u64,

        #[arg(long)]
        monitor: bool,
    },
    Note {
        #[arg(long, default_value_t = 60)]
        note: u8,

        #[arg(long, default_value_t = DEFAULT_VELOCITY)]
        velocity: u8,

        #[arg(long, default_value_t = 1000)]
        ms: u64,
    },
    Demo,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut midi = MidiOut::connect(&cli.target)?;
    midi.program_change(cli.channel, cli.program)?;

    match cli.command {
        Command::Run {
            velocity,
            press_debounce_ms,
            release_debounce_ms,
            settle_us,
            scan_sleep_us,
            monitor,
        } => run_matrix(
            &mut midi,
            cli.channel,
            velocity,
            Duration::from_millis(press_debounce_ms),
            Duration::from_millis(release_debounce_ms),
            Duration::from_micros(settle_us),
            Duration::from_micros(scan_sleep_us),
            monitor,
        ),
        Command::Note { note, velocity, ms } => {
            midi.note_on(cli.channel, note, velocity)?;
            thread::sleep(Duration::from_millis(ms));
            midi.note_off(cli.channel, note)?;
            thread::sleep(Duration::from_millis(80));
            Ok(())
        }
        Command::Demo => play_demo(&mut midi, cli.channel),
    }
}

struct MidiOut {
    seq: Seq,
    source_port: i32,
}

impl MidiOut {
    fn connect(target_name: &str) -> Result<Self> {
        let seq =
            Seq::open(None, Some(Direction::Playback), false).context("open ALSA sequencer")?;
        seq.set_client_name(c"keyboard-synth")
            .context("set ALSA client name")?;

        let source_port = seq
            .create_simple_port(
                c"out",
                PortCap::READ | PortCap::SUBS_READ,
                PortType::MIDI_GENERIC | PortType::APPLICATION,
            )
            .context("create ALSA sequencer port")?;

        let target = find_client_port(&seq, target_name)
            .with_context(|| format!("find ALSA MIDI target containing {target_name:?}"))?;
        connect_ports(&seq, source_port, target).context("connect to FluidSynth MIDI input")?;

        Ok(Self { seq, source_port })
    }

    fn program_change(&mut self, channel: u8, program: u32) -> Result<()> {
        let control = EvCtrl {
            channel,
            param: 0,
            value: program as i32,
        };
        self.emit(EventType::Pgmchange, &control)
    }

    fn note_on(&mut self, channel: u8, note: u8, velocity: u8) -> Result<()> {
        self.emit_note(EventType::Noteon, channel, note, velocity)
    }

    fn note_off(&mut self, channel: u8, note: u8) -> Result<()> {
        self.emit_note(EventType::Noteoff, channel, note, 0)
    }

    fn all_notes_off(&mut self, channel: u8) {
        for note in all_mapped_notes() {
            let _ = self.note_off(channel, note);
        }
    }

    fn emit_note(
        &mut self,
        event_type: EventType,
        channel: u8,
        note: u8,
        velocity: u8,
    ) -> Result<()> {
        let note = EvNote {
            channel,
            note,
            velocity,
            off_velocity: 0,
            duration: 0,
        };
        self.emit(event_type, &note)
    }

    fn emit<T: alsa::seq::EventData>(&mut self, event_type: EventType, data: &T) -> Result<()> {
        let mut event = Event::new(event_type, data);
        event.set_source(self.source_port);
        event.set_subs();
        event.set_direct();
        self.seq
            .event_output_direct(&mut event)
            .context("send MIDI event")?;
        Ok(())
    }
}

struct MatrixGpio {
    buses: Vec<OutputPin>,
    notes: Vec<InputPin>,
}

impl MatrixGpio {
    fn new() -> Result<Self> {
        let gpio = Gpio::new().context("open GPIO")?;
        let mut buses = Vec::new();
        for (_, pin) in BUSES {
            buses.push(
                gpio.get(pin)
                    .with_context(|| format!("open BUS GPIO {pin}"))?
                    .into_output_low(),
            );
        }

        let mut notes = Vec::new();
        for (_, pin) in NOTES {
            notes.push(
                gpio.get(pin)
                    .with_context(|| format!("open note GPIO {pin}"))?
                    .into_input_pulldown(),
            );
        }

        Ok(Self { buses, notes })
    }

    fn scan(&mut self, settle: Duration) -> Vec<MatrixKey> {
        let mut pressed = Vec::new();
        for (bus_index, bus) in self.buses.iter_mut().enumerate() {
            bus.set_high();
            if !settle.is_zero() {
                thread::sleep(settle);
            }
            for (note_col, note) in self.notes.iter().enumerate() {
                let key = MatrixKey {
                    bus: bus_index,
                    note_col,
                };
                if note.is_high() && midi_note_for_key(key).is_some() {
                    pressed.push(key);
                }
            }
            bus.set_low();
        }
        pressed
    }
}

impl Drop for MatrixGpio {
    fn drop(&mut self) {
        for bus in &mut self.buses {
            bus.set_low();
        }
    }
}

fn run_matrix(
    midi: &mut MidiOut,
    channel: u8,
    velocity: u8,
    press_debounce: Duration,
    release_debounce: Duration,
    settle: Duration,
    scan_sleep: Duration,
    monitor: bool,
) -> Result<()> {
    let mut gpio = MatrixGpio::new()?;
    let mut state = ScannerState::new(press_debounce, release_debounce, velocity);
    let started = Instant::now();
    let mut sounding = BTreeSet::new();
    let shutdown = Arc::new(AtomicBool::new(false));
    flag::register(SIGTERM, Arc::clone(&shutdown)).context("register SIGTERM handler")?;
    flag::register(SIGINT, Arc::clone(&shutdown)).context("register SIGINT handler")?;

    println!("keyboard-synth Rust controller started");
    while !shutdown.load(Ordering::Relaxed) {
        let raw = gpio.scan(settle);
        let actions = state.update(started.elapsed(), &raw);
        for action in actions {
            match action {
                MidiAction::NoteOn { note, velocity } => {
                    midi.note_on(channel, note, velocity)?;
                    sounding.insert(note);
                    if monitor {
                        print_action("noteon", note);
                    }
                }
                MidiAction::NoteOff { note } => {
                    midi.note_off(channel, note)?;
                    sounding.remove(&note);
                    if monitor {
                        print_action("noteoff", note);
                    }
                }
            }
        }

        if !scan_sleep.is_zero() {
            thread::sleep(scan_sleep);
        }
    }

    for note in sounding {
        midi.note_off(channel, note)?;
    }
    midi.all_notes_off(channel);
    Ok(())
}

fn print_action(action: &str, note: u8) {
    println!("{action} midi={note}");
}

fn find_client_port(seq: &Seq, needle: &str) -> Result<Addr> {
    let needle = needle.to_lowercase();

    for client in alsa::seq::ClientIter::new(seq) {
        let client_name = client.get_name().unwrap_or("").to_lowercase();
        if !client_name.contains(&needle) {
            continue;
        }

        let client_id = client.get_client();
        for port in alsa::seq::PortIter::new(seq, client_id) {
            let caps = port.get_capability();
            if caps.contains(PortCap::WRITE) || caps.contains(PortCap::SUBS_WRITE) {
                return Ok(Addr {
                    client: client_id,
                    port: port.get_port(),
                });
            }
        }
    }

    bail!("no ALSA sequencer target found for {needle:?}; run `aconnect -l` on the Pi")
}

fn connect_ports(seq: &Seq, source_port: i32, target: Addr) -> Result<()> {
    let sender = Addr {
        client: seq.client_id().context("get ALSA client id")?,
        port: source_port,
    };

    let sub = PortSubscribe::empty().context("create ALSA port subscription")?;
    sub.set_sender(sender);
    sub.set_dest(target);
    seq.subscribe_port(&sub)
        .with_context(|| format!("subscribe {sender:?} -> {target:?}"))?;

    Ok(())
}

fn play_demo(midi: &mut MidiOut, channel: u8) -> Result<()> {
    let melody = [
        60, 63, 67, 70, 72, 70, 67, 63, 65, 68, 72, 75, 77, 75, 72, 68,
    ];

    for note in melody {
        midi.note_on(channel, note, 95)?;
        thread::sleep(Duration::from_millis(190));
        midi.note_off(channel, note)?;
        thread::sleep(Duration::from_millis(45));
    }

    for note in [60, 63, 67, 72] {
        midi.note_on(channel, note, 85)?;
    }
    thread::sleep(Duration::from_millis(900));
    for note in [60, 63, 67, 72] {
        midi.note_off(channel, note)?;
    }
    thread::sleep(Duration::from_millis(120));

    Ok(())
}
