# wispr-fox

**Press a key. Talk. Get text.** Anywhere on your computer.

A free, open-source dictation app for Windows and macOS. Powered by
Whisper. Bring your own API key. No subscription, no telemetry, no
account.

Yes, that's Clippy. He's back.

![wispr-fox screenshot placeholder — F8 popup with Clippy reacting](docs/screenshot.png)

---

## What it does

Three hotkeys, three behaviors:

| Press | What happens |
|---|---|
| **F8** | You talk → raw transcript pastes into whatever app you're in |
| **F9** | You talk → cleaned-up transcript (grammar fixes, no rewriting) |
| **F10** | You talk → a polished draft based on your brief |

Hold the key while talking, release when done. Or press once to start
and once to stop ("sticky mode" — toggle in Settings, or use `Win+F8/F9/F10`).

Works in any app with a text field: browser, Slack, VS Code, Word,
your terminal, anywhere.

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

## Install

### Windows

1. Download the latest `wispr-fox_x.y.z_x64-setup.exe` from
   [Releases](../../releases/latest).
2. Run it. Click through the installer.
3. First launch will walk you through onboarding (~2 minutes).

### macOS

1. Download `wispr-fox_x.y.z_aarch64.dmg` (Apple Silicon) or
   `wispr-fox_x.y.z_x64.dmg` (Intel) from [Releases](../../releases/latest).
2. Drag to `/Applications`.
3. First launch: right-click → Open (to bypass Gatekeeper since the
   app isn't notarized yet).
4. Grant **Accessibility** and **Microphone** permission when prompted.

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

- **F8 is for raw speed.** Use it when you know what you want to say.
- **F9 is for cleanup.** Use it when you mumbled, said "uh" a lot,
  or want grammar fixes — but you don't want the AI rewriting your
  voice.
- **F10 is for drafts.** Say *"Reply to my boss, agree to the meeting,
  push it to 3pm Friday"* and you'll get a full polished message.
- **Customize the hotkeys.** Settings → Hotkeys → click Record →
  press any combo. `Win+F8` works great. Avoid `Win+Space` (Windows
  reserves it).
- **Customize the prompts.** Settings → Models → Show system prompt
  for any mode. Edit, save. Reset to default any time.
- **Switch Clippy off.** Settings → Look & Feel → Skin: Off. The
  app still works the same.
- **Dark mode.** Settings → Look & Feel → Theme: Dark. Or Retro
  (Windows 98 vibes).

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
Microsoft objects, we'll swap to the bundled "Chippy" potato-chip
mascot.

---

## Credits

- Whisper Large v3 Turbo by [OpenAI](https://openai.com), hosted by
  [Groq](https://groq.com)
- Llama 3.3 by [Meta](https://ai.meta.com)
- Gemini by [Google DeepMind](https://deepmind.google)
- Clippy sprite via [clippyts](https://github.com/pi0/clippyts)
  (vendored + patched)
- Tauri 2 + Svelte 5 — the lean desktop stack
