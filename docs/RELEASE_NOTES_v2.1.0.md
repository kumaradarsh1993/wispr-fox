# wispr-fox v2.1.0

The dictation gets quieter, the history gets smarter, and your laptop can
finally sleep. This is the stable release of everything the v2.1.0 nightly
cycle has been building since v2.0.0 — pulled together, tested, and promoted
to Latest.

## Your laptop can sleep again

The headline fix. A background audio helper used to hold your sound device
open for the app's entire lifetime — and Windows treats any app that's holding
the audio device as "audio is playing," so it refused to ever go to sleep.
wispr-fox now opens that device only for the split second it plays a start/stop
cue and releases it after a few seconds idle. Nothing about the cues changes;
your machine just sleeps normally again.

## A roster of characters — including pixel pets

- **Pixel pets.** Eight animated sprite-buddies now live in the avatar picker
  alongside the watercolor fox — they idle, listen, and celebrate as you
  dictate. Pick your favourite from the sidebar (six featured, plus a "More"
  tile for the rest).
- **Minimal skins.** Prefer no character at all? The **wave** skin is a small
  translucent pill with a live waveform that dances to your voice, and the
  **siri** skin is a glowing orb. Both stay out of the way.
- **Show/hide, your call.** A three-way avatar toggle: always visible, only
  while you're dictating, or fully hidden.

## History that names itself

- **Auto-titles.** Each recording now gets a short one-line name written for
  it, so your history reads like a table of contents instead of a wall of
  timestamps.
- **A built-in flight recorder.** Open the (i) panel on any recording to see
  exactly how long transcription and cleanup took, plus a step-by-step
  timeline. A slow result now explains itself.

## Your mic, diagnosed

Two classes of "why is my transcript wrong" now get caught and explained
instead of silently pasting a broken result:

- **Mic dropped mid-recording.** If your microphone dies partway through
  (a known Bluetooth-headset quirk), the app compares how much audio it
  actually captured against how long you spoke and tells you the transcript is
  cut short — rather than pretending a half-recording succeeded.
- **Mic slow to wake up.** On some Windows machines the mic takes several
  seconds to start after you press the key, quietly eating your first
  sentence. The app now measures this "wake-up time" on every recording, and
  the onboarding demo doubles as a mic health-check — with the exact Windows
  settings to fix it when it's slow (turn off the mic's audio enhancements and
  "exclusive control").

## A first-run that actually walks you through it

Onboarding was rebuilt from the ground up. It fills the window cleanly at any
size (no more frozen band with white margins), acts out a real dictation on the
welcome screen so you can see what the app does, and walks you through picking
a transcription engine and pasting a key with a clear "I already have a key"
vs. "help me get one" fork. Setup takes about five minutes and the app is, as
always, just the interface — you bring your own key and it stays on your
machine.

---

*Bring-your-own AI key. No subscription, no account, no telemetry. Windows and
macOS (Apple Silicon); the macOS build is unsigned — right-click → Open on
first launch.*
