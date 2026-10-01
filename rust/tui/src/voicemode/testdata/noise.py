"""The fixtures that are not speech (make.sh): silence, a room's hiss and
hum, a keyboard typed on in that room, and the sentence said over the
room. Seeded, stdlib only, 16 kHz mono 16-bit; writes files, plays
nothing."""

import math
import random
import struct
import wave

RATE = 16000


def write(name, xs):
    with wave.open(name, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(b"".join(struct.pack("<h", max(-32767, min(32767, int(round(x))))) for x in xs))


def read(name):
    with wave.open(name, "rb") as w:
        assert w.getframerate() == RATE and w.getnchannels() == 1 and w.getsampwidth() == 2
        raw = w.readframes(w.getnframes())
    return [s for (s,) in struct.iter_unpack("<h", raw)]


def room(n, rng):
    """A laptop mic in a quiet room: hiss (~-55 dBFS) with a slow drift,
    a faint 50 Hz hum, a little low rumble."""
    out, low = [], 0.0
    for i in range(n):
        low = 0.995 * low + 0.005 * rng.gauss(0, 1)
        drift = 1.0 + 0.3 * math.sin(2 * math.pi * 0.4 * i / RATE)
        out.append(rng.gauss(0, 45) * drift + 1200 * low + 20 * math.sin(2 * math.pi * 50 * i / RATE))
    return out


def keyboard(n, rng):
    """The room, and keys: ~8 strokes a second, each a sharp broadband
    click (~5 ms) with a short ringing tail (~30 ms)."""
    out = room(n, rng)
    t = int(0.15 * RATE)
    while t < n:
        amp = rng.uniform(2500, 7000)
        ring = rng.uniform(1800, 3500)
        for k in range(int(0.035 * RATE)):
            if t + k >= n:
                break
            env = math.exp(-k / (0.004 * RATE))
            tail = 0.25 * math.exp(-k / (0.012 * RATE)) * math.sin(2 * math.pi * ring * k / RATE)
            out[t + k] += amp * (env * rng.gauss(0, 1) + tail)
        t += int(rng.uniform(0.07, 0.2) * RATE)
    return out


def main():
    rng = random.Random(7)
    write("silence.wav", [0] * int(1.0 * RATE))
    write("room.wav", room(int(3.0 * RATE), rng))
    write("keyboard.wav", keyboard(int(3.0 * RATE), rng))
    # the sentence in the room: 0.5 s of room, the sentence, 0.7 s of room
    speech = read("sentence.wav")
    pre, post = int(0.5 * RATE), int(0.7 * RATE)
    bed = room(pre + len(speech) + post, rng)
    write("sentence_room.wav", [b + (speech[i - pre] if pre <= i < pre + len(speech) else 0) for i, b in enumerate(bed)])


main()
