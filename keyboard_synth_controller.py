#!/usr/bin/env python3
"""GPIO matrix scanner that drives FluidSynth through its TCP shell."""

from __future__ import annotations

import argparse
import signal
import socket
import sys
import time
from dataclasses import dataclass

import RPi.GPIO as GPIO


BUSES = [
    ("BUS1", 5),
    ("BUS2", 6),
    ("BUS3", 13),
    ("BUS4", 19),
    ("BUS5", 26),
]

NOTES = [
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
]


@dataclass(frozen=True, order=True)
class Key:
    bus: int
    note_col: int


@dataclass
class DebounceSlot:
    raw: bool = False
    stable: bool = False
    changed_at: float = 0.0


class FluidSynthClient:
    def __init__(self, host: str, port: int, channel: int, program: int) -> None:
        self.host = host
        self.port = port
        self.channel = channel
        self.program = program
        self.sock: socket.socket | None = None

    def connect(self) -> None:
        deadline = time.monotonic() + 30.0
        last_error: OSError | None = None
        while time.monotonic() < deadline:
            try:
                self.sock = socket.create_connection((self.host, self.port), timeout=3)
                self.command(f"prog {self.channel} {self.program}")
                return
            except OSError as exc:
                last_error = exc
                time.sleep(1.0)
        raise RuntimeError(f"could not connect to FluidSynth at {self.host}:{self.port}: {last_error}")

    def command(self, command: str) -> None:
        if self.sock is None:
            raise RuntimeError("FluidSynth socket is not connected")
        self.sock.sendall((command + "\n").encode())

    def note_on(self, note: int, velocity: int) -> None:
        self.command(f"noteon {self.channel} {note} {velocity}")

    def note_off(self, note: int) -> None:
        self.command(f"noteoff {self.channel} {note}")

    def all_notes_off(self) -> None:
        try:
            self.command(f"cc {self.channel} 123 0")
        except OSError:
            pass

    def close(self) -> None:
        if self.sock is not None:
            self.sock.close()
            self.sock = None


def midi_note_for_key(key: Key) -> int | None:
    if key.bus == 0:
        if 5 <= key.note_col <= 11:
            return 29 + (key.note_col - 5)
        return None
    if key.bus in (1, 2, 3):
        return 36 + ((key.bus - 1) * 12) + key.note_col
    if key.bus == 4 and key.note_col == 0:
        return 72
    return None


def setup_gpio() -> None:
    GPIO.setmode(GPIO.BCM)
    GPIO.setwarnings(False)
    for _, pin in BUSES:
        GPIO.setup(pin, GPIO.OUT, initial=GPIO.LOW)
    for _, pin in NOTES:
        GPIO.setup(pin, GPIO.IN, pull_up_down=GPIO.PUD_DOWN)


def scan_once(settle_seconds: float) -> set[Key]:
    pressed: set[Key] = set()
    for bus_index, (_, bus_pin) in enumerate(BUSES):
        GPIO.output(bus_pin, GPIO.HIGH)
        time.sleep(settle_seconds)
        for note_index, (_, note_pin) in enumerate(NOTES):
            if GPIO.input(note_pin) and midi_note_for_key(Key(bus_index, note_index)) is not None:
                pressed.add(Key(bus_index, note_index))
        GPIO.output(bus_pin, GPIO.LOW)
    return pressed


def update_debounce(
    slots: dict[Key, DebounceSlot],
    raw_pressed: set[Key],
    now: float,
    press_debounce_seconds: float,
    release_debounce_seconds: float,
) -> tuple[list[Key], list[Key]]:
    pressed: list[Key] = []
    released: list[Key] = []

    for bus_index in range(len(BUSES)):
        for note_index in range(len(NOTES)):
            key = Key(bus_index, note_index)
            if midi_note_for_key(key) is None:
                continue
            slot = slots.setdefault(key, DebounceSlot(changed_at=now))
            raw = key in raw_pressed
            if raw != slot.raw:
                slot.raw = raw
                slot.changed_at = now
            debounce_seconds = press_debounce_seconds if slot.raw else release_debounce_seconds
            if slot.stable != slot.raw and now - slot.changed_at >= debounce_seconds:
                slot.stable = slot.raw
                if slot.stable:
                    pressed.append(key)
                else:
                    released.append(key)

    return pressed, released


def run(args: argparse.Namespace) -> int:
    synth = FluidSynthClient(args.fluidsynth_host, args.fluidsynth_port, args.channel, args.program)
    synth.connect()
    setup_gpio()

    slots: dict[Key, DebounceSlot] = {}
    sounding: set[int] = set()
    stopping = False

    def stop(_signum: int, _frame: object) -> None:
        nonlocal stopping
        stopping = True

    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGINT, stop)

    try:
        print("keyboard-synth controller started", flush=True)
        while not stopping:
            now = time.monotonic()
            raw_pressed = scan_once(args.settle_ms / 1000.0)
            pressed, released = update_debounce(
                slots,
                raw_pressed,
                now,
                args.press_debounce_ms / 1000.0,
                args.release_debounce_ms / 1000.0,
            )

            for key in pressed:
                note = midi_note_for_key(key)
                if note is None or note in sounding:
                    continue
                synth.note_on(note, args.velocity)
                sounding.add(note)
                if args.monitor:
                    print(f"noteon bus={BUSES[key.bus][0]} note={NOTES[key.note_col][0]} midi={note}", flush=True)

            for key in released:
                note = midi_note_for_key(key)
                if note is None or note not in sounding:
                    continue
                synth.note_off(note)
                sounding.remove(note)
                if args.monitor:
                    print(f"noteoff bus={BUSES[key.bus][0]} note={NOTES[key.note_col][0]} midi={note}", flush=True)

            time.sleep(args.scan_ms / 1000.0)
    finally:
        for note in sorted(sounding):
            synth.note_off(note)
        synth.all_notes_off()
        synth.close()
        for _, pin in BUSES:
            try:
                GPIO.output(pin, GPIO.LOW)
            except RuntimeError:
                pass
        GPIO.cleanup()

    return 0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Scan the Synth DIY organ keyboard matrix.")
    parser.add_argument("--fluidsynth-host", default="127.0.0.1")
    parser.add_argument("--fluidsynth-port", type=int, default=9800)
    parser.add_argument("--channel", type=int, default=0)
    parser.add_argument("--program", type=int, default=16, help="GM program number, zero-based. 16 is drawbar organ.")
    parser.add_argument("--velocity", type=int, default=110)
    parser.add_argument("--press-debounce-ms", type=float, default=15.0)
    parser.add_argument("--release-debounce-ms", type=float, default=120.0)
    parser.add_argument("--settle-ms", type=float, default=1.0)
    parser.add_argument("--scan-ms", type=float, default=2.0)
    parser.add_argument("--monitor", action="store_true")
    return parser.parse_args()


if __name__ == "__main__":
    sys.exit(run(parse_args()))
