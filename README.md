# wispr-fox

**Press a key. Talk. Get text.** Anywhere on your computer.

A free, open-source dictation app for Windows and macOS. Powered by
Whisper. Bring your own API key. No subscription, no telemetry, no
account.

Yes, that's Clippy. He's back.

---

## ⬇ Download

### Windows — one click

**[Download the latest installer (.exe)](https://github.com/kumaradarsh1993/wispr-fox/releases/latest)** →
look for `wispr-fox_x.y.z_x64-setup.exe`.

Run it, click through, done. Installs to your user folder — no admin needed.
First launch walks you through a 2-minute onboarding (paste your API key,
test your mic).

### macOS

Right-click → Open the DMG. Mac install is a little more involved — full
walkthrough [below](#macos-first-time-install-walkthrough).

---

![wispr-fox screenshot placeholder — F8 popup with Clippy reacting](docs/screenshot.png)

---

## What it does

Two main hotkeys. Hold the key while you talk, release when you're done
(or use sticky mode to press-once-start / press-again-stop).

| Press | What happens |
|---|---|
| **F8** | You talk → raw transcript pastes into whatever app you're in. Toggle "Clean up raw" in Settings if you want light cleanup (spell + punctuation + paragraphing, no rewriting). |
| **Shift+F8** | One-shot override: force the cleanup on for *this* recording only — handy when you usually want raw but occasionally need cleaner output. |
| **F9** | You talk → a polished draft based on your brief. Best for emails, Slack, docs — describe what you want and the LLM writes it. |
| **Win+F8 / Win+F9** | Sticky versions of the above — press once to start, press again to stop. |

Works in any app with a text field: browser, Slack, Outlook, Teams, VS Code,
Word, your terminal, anywhere.

---

## Why use this

- **It's free.** WisprFlow is $15/month. This is $0/month.
- **Bring your own key.** You pay your AI provider directly. Typical
  cost for heavy daily use: **$3–8/month**. Light use: pennies.
- **Open source.** Read the code. Fork it. Self-host.
- **No data leaves your machine** except the audio you choose to
  send to your chosen provider for transcription. No analytics, no
  telemetry, no account signup.
- **Indian English + Hindi friendly.** Whisper auto-detects, handles
  code-switching mid-sentence without choking.
- **Fast.** ~0.5s round trip for a 10-second clip on a normal
  connection.
- **Clippy.** The actual Microsoft sprite. He blinks. He pays
  attention. He's optional.

---

## Install (detailed)

### Windows

1. Download the latest `wispr-fox_x.y.z_x64-setup.exe` from
   [Releases](https://github.com/kumaradarsh1993/wispr-fox/releases/latest).
2. Run it. Click through the installer.
3. App **starts silently** by default — only the system-tray icon
   and Clippy floater appear. **Double-click Clippy** or **left-click
   the tray icon** to open the main window for Settings/History.
4. First launch walks you through onboarding (~2 minutes).

### macOS (first-time install walkthrough)

The Mac build isn't code-signed yet (Apple Developer cert costs $99/yr
and we're not there). So macOS treats it as untrusted on first launch.
The dance below gets past it once; subsequent launches work normally.

**1. Download the right DMG** from
[Releases](https://github.com/kumaradarsh1993/wispr-fox/releases/latest):
- Apple Silicon (M1/M2/M3/M4): `wispr-fox_x.y.z_aarch64.dmg`
- Intel Mac: `wispr-fox_x.y.z_x64.dmg`
- Not sure? Apple menu → About This Mac → "Chip" line says "Apple M…"
  for Apple Silicon, "Intel" for Intel.

**2. Drag to Applications.** Open the DMG, drag the `wispr-fox` icon
to the Applications folder shortcut shown.

**3. First launch — pick the one that matches what you see:**

*If you see a dialog "wispr-fox cannot be opened because the
developer cannot be verified"* — the friendly case:
- Click **Cancel** (counterintuitive but necessary).
- Open Finder → Applications → **right-click** (or Control-click)
  on `wispr-fox` → **Open**.
- Now a different dialog appears with an **Open** button. Click it.
- App launches.

*If you see "wispr-fox is damaged and can't be opened. You should
move it to the Trash."* — newer macOS sets a quarantine attribute on
downloaded unsigned binaries. Strip it:
- Open Terminal (Spotlight → "Terminal" → Enter).
- Run: `xattr -dr com.apple.quarantine /Applications/wispr-fox.app`
- Now double-click the app normally.

**4. Grant the two permissions** macOS will prompt for:

*First prompt — Microphone* (appears the first time you press F8):
- Click **OK** / **Allow**. Without this, dictation can't record.
- If you accidentally click Deny: System Settings → Privacy &
  Security → Microphone → toggle wispr-fox ON.

*Second prompt — Accessibility* (appears the first time the app
tries to paste text into another app):
- Click **Open System Settings**.
- In the Accessibility list, toggle wispr-fox ON.
- You'll be asked for your Mac password to confirm.
- Switch back to wispr-fox and try F8 again.
- Without this, dictation still works but pastes via a clipboard
  fallback (slightly slower, briefly overwrites your clipboard).

**5. (Optional) Pin to Dock.** Right-click the app's Dock icon while
running → Options → Keep in Dock.

**Common gotchas:**

- *F8 does nothing on Mac.* Some Macs map F-keys to brightness/volume
  by default. Hold **Fn + F8**, or in System Settings → Keyboard →
  Keyboard Shortcuts → Function Keys → toggle "Use F1, F2, etc. as
  standard function keys". Alternatively change the hotkey in
  wispr-fox Settings → Hotkeys.
- *Mic indicator (orange dot in menu bar) stays on after I stop
  recording.* It doesn't — macOS shows it for ~5 seconds after the
  mic releases. Normal.
- *App won't launch after macOS update.* Sometimes a major macOS
  update re-quarantines unsigned apps. Re-run the `xattr` command
  from step 3.

### Linux

Builds are available but untested. `wispr-fox_x.y.z_amd64.AppImage`
from [Releases](../../releases/latest). Report what works.

---

## Get an API key

You need **one** of these to use the app. Both have generous free
tiers — you can use the app for weeks before paying anything.

### Groq (recommended for speed + cost)

1. Go to <https://console.groq.com/keys>
2. Sign in (Google / GitHub).
3. Create a key. Copy it (starts with `gsk_`).
4. Paste into wispr-fox Settings → Provider & Keys → Groq.

**Free tier:** 14,400 requests/day. Easily covers daily-driving the
app. Beyond that, transcription is ~$0.04/hour of audio and LLM
cleanup is fractions of a cent per request.

### Google Gemini (alternative for LLM cleanup)

1. Go to <https://aistudio.google.com/apikey>
2. Create a key.
3. Paste into Settings → Provider & Keys → Gemini.

Note: Gemini only does the LLM cleanup half (F9/F10). You still need
Groq for transcription (the F8 raw flow), since Gemini doesn't yet
expose a Whisper-equivalent API.

---

## First run

After install:

1. App launches into onboarding. Clippy walks you through it.
2. You'll paste your Groq key.
3. App tests the key (1 second), confirms it works.
4. You'll record a 3-second test clip to confirm your mic.
5. Done. Press F8 anywhere on your computer to dictate.

The app **lives in your system tray** (Windows) / **menu bar** (macOS).
Closing the main window doesn't quit — it stays running so your
hotkeys keep working. Right-click the tray icon to quit.

---

## Tips

- **F8 is for raw speed.** Default behavior. Use it when you know what
  you want to say. Toggle "Clean up raw" in Settings → Models → Modes
  to flip every F8 to auto-cleanup.
- **Shift+F8 = on-demand cleanup.** Useful when you usually prefer raw
  but want this *one* recording cleaned. Doesn't change the toggle.
- **F9 is for drafts.** Say *"Reply to my boss, agree to the meeting,
  push it to 3pm Friday"* and you'll get a full polished message.
- **History page has three tabs per row** — Raw, Cleaned, Drafted.
  Click a dimmed tab to generate that version on demand from the same
  recording. No need to re-dictate.
- **Long recordings work.** Files over ~3 min auto-chunk and stitch
  the transcripts back together. No "file too large" errors.
- **If you switch apps while it's processing**, wispr-fox won't yank
  you back — it copies the result to your clipboard and Clippy
  whispers "Copied to clipboard". Press Ctrl+V wherever you want it.
  Opt out in Settings → Hotkeys → Behaviour.
- **Customize the hotkeys.** Settings → Hotkeys → click Record →
  press any combo. `Win+F8` works great. Avoid `Win+Space` (Windows
  reserves it).
- **Customize the prompts.** Settings → Models → Show system prompt
  for any mode. Edit, save. Reset to default any time.
- **Pick a Clippy skin.** Sidebar Floater section: Off, Paperclip
  (dark outline, elephant ear), Cream (warm beige variant, easier on
  dark backgrounds), or Clippy (the real Microsoft sprite).
- **Dark mode.** Settings → Look & Feel → Theme: Dark. Or Retro
  (Windows 98 vibes).
- **Launch at login.** Settings → Startup → "Launch wispr-fox at
  login". Pairs nicely with "Open silently" so it boots into the tray
  without popping a window.

---

## Privacy

- **Audio recordings** are stored locally in your AppData folder
  (Windows: `%APPDATA%\wispr-fox\audio\`, macOS:
  `~/Library/Application Support/wispr-fox/audio/`). Default retention
  7 days, 500MB cap. Configurable in Settings → Retention.
- **API keys** are stored in your OS keychain (Windows Credential
  Manager / macOS Keychain). Never sent anywhere except to the
  provider whose key it is.
- **Transcripts** are sent to your chosen provider (Groq / Gemini)
  for transcription and optional cleanup. Read their privacy policies
  — they're the parties that see your audio + text.
- **wispr-fox itself sends nothing to us.** There is no "us". No
  servers. No analytics. No crash reporting. The app does not phone
  home.

---

## Troubleshooting

**"Thank you" or random short phrases appearing at the start of
transcripts:**
Whisper hallucinates when the audio buffer starts empty. On Windows
this is usually caused by Realtek's "Audio enhancements" putting your
mic to sleep between recordings. Fix:
1. Open `mmsys.cpl` (Win+R → type `mmsys.cpl` → Enter)
2. Recording tab → double-click your mic → Advanced
3. Uncheck "Allow applications to take exclusive control"
4. Enhancements tab → check "Disable all enhancements"
5. OK, OK.

**Hotkey does nothing:**
Some apps grab F8/F9/F10 first (Visual Studio, certain games).
Settings → Hotkeys → change to something like `Ctrl+Alt+Space`.

**Transcription is in the wrong language:**
By default we let Whisper auto-detect. If you only speak one language
and it's mis-detecting, Settings → Models → Language Hint → pick yours.

**App won't start on Mac:**
You probably double-clicked instead of right-click → Open. macOS
quarantines unsigned apps. After the first right-click → Open,
subsequent launches work normally.

---

## Building from source

If you want to build it yourself:

```bash
# Prerequisites: Rust 1.75+, Node 20+, pnpm

git clone https://github.com/YOUR-USERNAME/wispr-fox
cd wispr-fox
pnpm install
pnpm tauri dev      # development
pnpm tauri build    # production binary
```

Full dev notes in [GETTING_STARTED.md](./GETTING_STARTED.md).

---

## Roadmap

Things that are likely:

- macOS text injection polishing (current Mac build uses clipboard
  fallback — works, but slower than the Windows SendInput path)
- Linux testing pass
- Self-hosted / custom OpenAI-compatible endpoint provider (for
  enterprise + Ollama users)
- Streaming partial transcripts (once Groq exposes streaming Whisper)

Things that won't happen:

- Subscription model
- Account signup
- Telemetry
- Cloud sync of recordings (they stay on your machine)
- iOS or Android — there's a [separate Android project](../wispr-fox-android/)
  with a different architecture; iOS is not planned.

---

## License

MIT. Use it however you want.

Clippy's likeness is © Microsoft. The sprite asset is included under
fair-use precedent (nostalgia / non-commercial reference). If
Microsoft objects, we'll swap to the bundled hand-drawn "Paperclip"
or "Cream" SVG skins, which are original work.

---

## Credits

- Whisper Large v3 Turbo by [OpenAI](https://openai.com), hosted by
  [Groq](https://groq.com)
- Llama 3.3 by [Meta](https://ai.meta.com)
- Gemini by [Google DeepMind](https://deepmind.google)
- Clippy sprite via [clippyts](https://github.com/pi0/clippyts)
  (vendored + patched)
- Tauri 2 + Svelte 5 — the lean desktop stack
