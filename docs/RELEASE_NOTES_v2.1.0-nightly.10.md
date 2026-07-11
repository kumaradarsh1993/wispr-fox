# wispr-fox v2.1.0-nightly.10

Your mic gets a health-check — starting with the onboarding demo.

## The "it ate my first sentence" detector

On some Windows machines the microphone takes 3–8 seconds to actually wake
up after you press the dictation key. You start talking immediately; the
audio only starts flowing seconds later; your first words are simply gone.
Nothing in the app could see this before — the recording timer starts late
too, so everything *looked* fine.

The app now measures **mic wake-up time** on every recording: the gap
between your key going down and the first real audio arriving.

- **In the onboarding demo**, the try-it step now doubles as a mic
  health-check. After your first recording you'll see a verdict: a green
  "instant handover" chip when things are healthy, or a clear warning with
  the exact settings to fix when they're not — on Windows that's almost
  always the mic's **audio enhancements** and **"allow applications to take
  exclusive control"** toggles. Change them, press the key again, watch the
  number drop.
- **In normal use**, if your mic is pathologically slow to wake (over
  ~2.5s), the floater tells you once per session — with the same fix —
  instead of letting you lose the start of every dictation forever.
- **In the (i) panel**, every recording's timeline now includes its
  "mic wake-up" number, alongside the recorded / captured / provider
  triangulation from nightly.8. Between the two, a truncated transcript
  now tells you *where* the audio went missing: the head (slow wake-up —
  fix your mic settings), the middle/tail (mic dropped — often Bluetooth),
  or in transit (provider saw less than was captured).

*Nightly build. If a recording ever comes back clipped, open its (i) —
the numbers will point at the culprit.*
