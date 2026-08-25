# wispr-fox v3.3.0

> Stable release · 25 August 2026

This release promotes the v3.3 nightly line to stable. It is the meetings
release — diarized recordings finally read like conversations instead of a wall
of text — and it is also the release that stopped the app crying wolf. Three
separate warnings were firing on recordings that were perfectly fine, and
between them they had turned the floater into something you learned to ignore.
They are gone.

## Meetings, properly

- Turn on speaker labels when you upload a recording and it becomes a meeting:
  the transcript renders as **speaker turns** with real spacing, not one
  undifferentiated block.
- **Name your speakers once.** Replace "Speaker 1" and "Speaker 2" with real
  names and they update everywhere at once — the card, reading mode, and
  anything you copy out — with no re-transcription and no extra cost.
- **Meeting Notes is its own version**, alongside Raw, Cleaned, and Drafted.
  The default prompt aims at a leadership-ready summary: the discussion,
  the decisions, the risks, the next actions. You can generate notes later on a
  recording you already have, and the prompt and its model are yours to change.
- **Reading mode** opens any version full-screen in a calm, distraction-free
  view for when you actually need to read a long one.
- **One Rerun dialog** replaces the three separate re-transcribe / cleanup /
  draft menu items. Pick any combination; anything that depends on a fresh
  transcript waits for it automatically.

## One key, two natural gestures

Push-to-talk and sticky mode used to be separate bindings you had to choose
between. Now every dictation key does both, and it reads your intent from how
long you hold it:

- **Tap** (under 700 ms) to latch — talk as long as you like, then press any
  dictation key or **Esc** to stop and send.
- **Hold** (700 ms or longer) for push-to-talk — release to stop and send.

Underneath, the whole dictation path was rebuilt around a single serialized
coordinator, which closed a real race: on a slow-to-wake microphone, a quick key
release could previously vanish before the audio device had finished starting
up. Timing now comes from the physical key edges, so it does not matter how slow
your mic, your disk, or your machine is.

## Three warnings that were lying to you

- **"Mic was very quiet"** — the app quietly boosts a too-quiet recording before
  sending it off, which is why your text kept coming back fine. But the warning
  was measured *before* that boost, so a mic that naturally sits near the line
  set off a red alert several times a day about a problem the app had already
  fixed for you. It is now measured on the audio speech-to-text actually
  receives. If the boost was not enough, you still get told — and now it says so.
- **"Mic dropped mid-recording"** — Windows sheds the occasional capture buffer,
  each around ten milliseconds, spread thinly across a long recording. Inaudible,
  invisible in the transcript, and nothing you could act on. The old check
  flagged any shortfall over one second, which across five minutes is a rounding
  error. A genuine drop — unplugged mic, another app seizing it, a driver reset —
  still warns you. Ordinary buffer churn stays quiet.
- **Messages that vanished mid-sentence** — with the avatar set to "while
  dictating", the floater hid itself on a fixed timer that ignored whatever was
  on screen, so a warning could disappear part-read. It now waits for the message
  to finish. Errors also get a much wider bubble instead of being squeezed
  through the narrow status bubble at three words per line.

## History and Insights fill the window again

- **Transcript text now fills its card.** It was capped at a fixed reading width
  that did not scale, so on a maximised window a card stopped its text around
  the 45% mark and left the rest of the card empty — and the same sentence
  wrapped at the same word whether the window was small or full-screen.
- **Insights is no longer a narrow centred column** with its scrollbar floating
  in the middle of the page and dead space either side of it. It uses the full
  pane, like History and Settings already did.
- **Insights now resizes off the pane, not the window.** Its layout breakpoints
  were measuring the whole window while the page only ever gets the window minus
  the sidebar — so a 1100px window squeezed four stat cards into 176px each
  instead of breaking them onto two rows.

## Also fixed

- The **first press of a dictation key no longer freezes the app.** A nightly
  regression could wedge the tray, the floater, and the main window on the very
  first keypress. If you ever ran v3.3.0-nightly.2, this is the fix.
- OpenAI's dedicated diarization path is supported alongside Deepgram and
  ElevenLabs. Asking for speaker labels on a model that cannot do them now
  switches to one that can and tells you why.
- Groq and Gemini defaults moved off model IDs that are being retired.
- Custom hotkey bindings go live during startup instead of waiting for you to
  open the hotkey editor once.

## Notes

Windows, macOS (Apple Silicon), and Linux builds all come from this one tagged
commit. The macOS build is unsigned — right-click → Open on first launch, or run
`xattr -dr com.apple.quarantine /Applications/wispr-fox.app` once.
