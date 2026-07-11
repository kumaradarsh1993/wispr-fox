# wispr-fox v2.1.0-nightly.9

Your laptop can sleep again — and onboarding got a proper do-over.

## Fixed: wispr-fox was quietly keeping your PC awake

If your laptop stopped going to sleep on its own, this was us. The little
"dop" sound that plays when you start and stop a recording kept its audio
channel open forever after the first use — and Windows treats *any* open
audio stream as "someone is listening to music, don't sleep." One dictation
after launch and your machine would stay awake until you quit the app.

The cue sound now opens its audio channel only when it actually plays and
releases it half a minute later. Nothing changes about how cues sound or
feel — your power settings just work again.

(If you want to double-check on your own machine: run `powercfg /requests`
in an elevated terminal. Before this build, wispr-fox showed up under
"An audio stream is currently in use." Now it doesn't.)

## Onboarding, rebuilt

The first-run experience is a clean three-step story again:

- **See it before you set it up.** The welcome screen acts out a real
  dictation: a hotkey cap gets pressed, one of the pixel buddies listens,
  transcribes, and the words type themselves into a little box. The buddy
  changes every loop, so you meet the avatar roster before you ever open
  Settings.
- **A straighter path to your key.** Pick Deepgram (recommended — $200
  signup credit that lasts years of daily use) or Groq (free forever),
  then tell us where you stand: **"I already have a key"** drops you at a
  paste box; **"Help me get one"** walks you through signup and the keys
  page link by link. The optional cleanup brain (Gemini, free) appears
  once your engine key is verified — and on the Groq path it's marked
  done automatically, because one Groq key does both jobs.
- **Fits your window.** No more fixed-width band with odd gutters, and no
  scrolling at the default window size — every step fits on one screen.

*Nightly build. The demo step is unchanged: press your dictation key on the
last screen and watch your words land in the box.*
