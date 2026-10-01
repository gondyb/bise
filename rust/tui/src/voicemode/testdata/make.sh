#!/usr/bin/env bash
# Voice mode's recorded fixtures (docs/voice-mode-plan.md §7 voice-audio):
# 16 kHz mono 16-bit WAVs, each < 150 KB. `say -o` and `afconvert` write
# files and play nothing; the noises are made by noise.py (seeded, stdlib
# only). Run it here: ./make.sh (macOS).
set -euo pipefail
cd "$(dirname "$0")"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
speak() { # name text
  say -v Samantha -o "$tmp/$1.aiff" "$2"
  afconvert -f WAVE -d LEI16@16000 -c 1 "$tmp/$1.aiff" "$1.wav"
}
speak sentence "Can you run the tests on the login page and tell me what failed?"
speak mm "Mm."
speak ok "Okay."
python3 noise.py
ls -l ./*.wav
