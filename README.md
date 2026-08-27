<h1 align="center">wispr-fox 🦊</h1>

<p align="center">
  <strong>Press a key. Talk. Get text.</strong> Anywhere on your computer.
</p>

<p align="center">
  A free, open-source dictation app for Windows and macOS.<br/>
  Bring your own AI key. No subscription or telemetry. Optional account sync.
</p>

<p align="center">
  <a href="https://kumaradarsh1993.github.io/wispr-fox/">Interactive website</a>
</p>

<p align="center">
  <a href="https://github.com/kumaradarsh1993/wispr-fox/releases/latest">
    <img alt="Latest stable" src="https://img.shields.io/github/v/release/kumaradarsh1993/wispr-fox?display_name=tag&label=stable&color=22c55e" />
  </a>
  <a href="https://github.com/kumaradarsh1993/wispr-fox/releases">
    <img alt="Latest beta" src="https://img.shields.io/github/v/release/kumaradarsh1993/wispr-fox?display_name=tag&include_prereleases&label=beta&color=eab308" />
  </a>
  <a href="LICENSE">
    <img alt="MIT licensed" src="https://img.shields.io/badge/license-MIT-blue" />
  </a>
</p>

<p align="center">
  <img src="docs/images/hero.png" alt="wispr-fox in action — Clippy floater reacting to dictation" width="720" />
</p>

---

## ⬇️ Download

### → [**Get the latest stable release**](https://github.com/kumaradarsh1993/wispr-fox/releases/latest)

That link always points at the newest stable build. Grab the one file for your
platform and run it:

| Platform | File on the release page |
|---|---|
| 🪟 **Windows** | `wispr-fox_<version>_x64-setup.exe` |
| 🍎 **macOS** (Apple Silicon) | `wispr-fox_<version>_aarch64.dmg` |
| 🐧 **Linux** | `wispr-fox_<version>_amd64.AppImage` · `.deb` · `.rpm` |

The app walks you through a 2-minute onboarding on first launch.

📓 [**All releases and their notes →**](https://github.com/kumaradarsh1993/wispr-fox/releases) — every version, newest first.
Builds tagged **Pre-release** are nightlies: newer, less tested, and safe to skip
unless you want to try something early. Anything not tagged Pre-release is stable.

> ℹ️ **Mac first-launch:** the build is ad-hoc code-signed but not notarized (notarization needs Apple's $99/yr Developer Program), so Gatekeeper stops it once. Drag to Applications, double-click, click **Done** on the "cannot verify the developer" dialog, then go to **System Settings → Privacy & Security** and click **Open Anyway**. One-line equivalent: `xattr -dr com.apple.quarantine /Applications/wispr-fox.app`.
>
> Right-click → Open stopped working as a Gatekeeper bypass in macOS 15 (Sequoia) — use **Open Anyway**.

---

## ✨ What it does

Tap a hotkey to latch recording, or hold it for push-to-talk. The text appears in whatever app you're in — Slack, Gmail, Word, your terminal, anywhere there's a text field.

| Press | What you get |
|---|---|
| 🎙️ **F8** *(Win)* / **Option+Space** *(Mac)* | Raw transcript — exactly what you said |
| ✏️ **F9** *(Win)* / **Option+Enter** *(Mac)* | Drafted output — give a brief, get a polished email/doc/message back |
| 🧹 **Shift+F8** / **Shift+Option+Space** *(Mac)* | One-shot cleaned version (spell + punctuation + paragraphing) |
| ⏏️ **Esc** | Stop and send the active recording |

Every dictation key is adaptive: release before **700 ms** to latch recording, then press any dictation key (or Esc) to stop and send. Hold for **700 ms or longer** to stop and send on release.

### Beyond dictation

- 🗂️ **History** keeps every recording with its Raw, Cleaned, Drafted, and Meeting
  Notes versions side by side — searchable, replayable, and re-runnable through a
  different provider or model without re-recording.
- 🎧 **Transcribe files you already have.** Drag any audio file onto the window,
  or upload from your phone — same pipeline, same versions.
- 🗣️ **Meetings.** Turn on speaker labels and a recording renders as named speaker
  turns, with a leadership-ready summary available as its own version.
- 📊 **Insights** shows lifetime time-saved, words, sessions and streaks, plus an
  on-device portrait of how you actually speak. Kept even after you clear recordings.
- ☁️ **Optional account sync.** Sign in and transcripts and provider keys follow you
  between devices. Your audio never leaves the machine it was recorded on. Signed
  out, the app behaves exactly as it always did and talks to no backend.

---

## 🚀 Setup in 3 steps

**1. Install.** Download the installer above and run it.

**2. Get a free API key.** Onboarding walks you through it with deep links to the right signup and keys pages — pick **Deepgram** (recommended: Nova-3, **$200 free credit**, no card) or **Groq** (free forever). Sign in with Google, create a key, paste it back into wispr-fox. Two minutes.

**3. Press your hotkey and talk.** That's it.

> 💡 **Why Deepgram is the recommended engine:** Nova-3 beats Whisper on both accuracy and speed (noticeably so for Indian-English accents), and heavy daily dictation costs roughly **$1/week**, so the free credit lasts a year or more. Keep Groq as the LLM "brain" for cleanup and drafting — it stays free.

<p align="center">
  <img src="docs/images/onboarding.png" alt="Onboarding flow — Foxy walks you through three steps" width="560" />
</p>

---

## 💸 How is this free?

You bring your own AI provider key, and services like Groq and Deepgram offer generous free tiers and credits:

- **Groq** — free forever. The free tier gives you **14,400 transcription requests/day** (about 20 hours of dictation, daily). Most people never hit the limit. It also powers the LLM cleanup/drafting for free.
- **Deepgram** — **$200 free credit on signup, no credit card**. Even heavy daily dictation runs about **$1/week** on Nova-3, so the credit typically lasts a year or more.

Either way, most users pay **$0**. There's no wispr-fox subscription — your usage bills (if any) go straight to your own provider account.

---

## 🤫 Privacy

- 🔐 **API keys** live in your OS keychain — Windows Credential Manager / macOS Keychain. If you explicitly sign in, selected provider keys are also synced through your Supabase account so your devices can share them.
- 🎧 **Saved audio files** stay on your machine and are never synced to the wispr-fox account backend. Default: 7-day retention, 500 MB cap, both configurable.
- ☁️ **Only the audio you choose to dictate** is sent to your chosen provider (Groq, Deepgram, OpenAI, or ElevenLabs for transcription; Groq, Gemini, or OpenAI for cleanup). Read their privacy policies — they're the parties that see your data.
- 📡 **No analytics or crash reporting.** Signed-out mode has no wispr-fox account traffic. Optional sign-in syncs transcripts and selected API keys through Supabase; update checks use GitHub Releases.

---

## 🎨 Make it yours

- 🦊 **Pick a floater** from Settings → Avatar: Fox (default), Codex Fox, Clippo,
  the real Clippy, Blacky, Uru & Gujia, Mochi & Marmalade, Pikachu, the pixel-pet
  set, or the minimal Wavy / Siri skins. Whether it shows is a separate choice —
  always, only while dictating, or hidden.
- 🌗 **Auto, Light, Dark, or Retro themes** in Settings → Appearance.
- ⌨️ **Rebind any hotkey** — Settings → Dictation. Defaults are sensible on each platform.
- 🎭 **Customise the LLM prompts** per mode if you want a specific tone — Settings → Modes → Show system prompt.
- 🚀 **Launch at login** — Settings → General.

---

## 🛟 Common questions

<details>
<summary><strong>The Mac hotkey does nothing — F8 just plays/pauses music</strong></summary>

That's macOS treating the function row as media keys by default. The Mac defaults are **Option+Space** (transcribe) and **Option+Enter** (draft) for exactly this reason. If you'd rather use F-keys: System Settings → Keyboard → Keyboard Shortcuts → Function Keys → toggle "Use F1, F2, etc. as standard function keys". Or rebind in wispr-fox Settings → Dictation.

</details>

<details>
<summary><strong>Transcription starts with random "Thank you" or other phrases</strong></summary>

Whisper-family models hallucinate on a silent buffer (Deepgram Nova-3 does this far less — another reason it's the recommended provider). On Windows, Realtek audio enhancements can put your mic to sleep between recordings — open `mmsys.cpl`, find your mic, uncheck "Allow exclusive control" + check "Disable all enhancements".

</details>

<details>
<summary><strong>Wrong language detected</strong></summary>

The speech model auto-detects by default. If you only speak one language and it's mis-detecting, set Settings → Models → Language Hint.

</details>

<details>
<summary><strong>Want to use a different AI provider?</strong></summary>

Settings → Providers & Keys. For **transcription** you can pick Groq (Whisper), Deepgram (Nova-3 — the recommended one), OpenAI, or ElevenLabs. For **cleanup/drafting** you can pick Groq, Gemini (key from <https://aistudio.google.com/apikey>), or OpenAI. Mix and match — a common combo is Deepgram for listening + Groq for the free LLM brain.

</details>

<details>
<summary><strong>Why is the macOS build unsigned?</strong></summary>

Apple charges $99/year for a Developer ID certificate. We're not there as a free single-developer project. macOS treats unsigned apps as quarantined on first launch — the `xattr` workaround above gets past it once and forever for that install.

</details>

---

## 📦 What's next

See the [**Roadmap**](docs/ROADMAP.md) for what's planned. Highlights: merging
usage and insights across all your signed-in devices, Sarvam Saaras as a
Hindi-friendly STT provider, and a plugin-based avatar system.

---

<details>
<summary><h2 style="display:inline">🧑‍💻 For developers</h2></summary>

### Build from source

```bash
# Prerequisites: Rust 1.75+, Node 20+
git clone https://github.com/kumaradarsh1993/wispr-fox
cd wispr-fox
npm install
npm run tauri dev      # development
npm run tauri build    # production binary (heavy, needs ≥16 GB RAM)
```

Full dev notes: [GETTING_STARTED.md](./GETTING_STARTED.md).

### Architecture, 90-second tour

- **Frontend** — SvelteKit + Svelte 5 (runes). Routes: `/` redirects to onboarding or history, `/clippy` is the always-on-top floater, `/settings/*` is the config UI.
- **Backend** — Rust + Tauri 2. Modules: `audio/` (cpal capture → WAV), `stt/` (Groq Whisper, Deepgram Nova-3, OpenAI, ElevenLabs), `llm/` (Groq Llama, Gemini, OpenAI), `inject/` (SendInput on Windows, CGEvent on macOS), `flow.rs` (state machine), `hotkey.rs` (global shortcuts), `secrets.rs` (keychain).
- **Storage** — SQLite for history, `tauri-plugin-store` for settings, OS keychain for API keys.
- **CI** — Every tag push builds Win NSIS + macOS DMG + Linux installers on GitHub Actions.

### License

MIT for the code we wrote. The vendored Microsoft Clippy sprite is © Microsoft — included under fair-use precedent (nostalgia / non-commercial reference). If Microsoft objects, the hand-drawn Paperclip / Fox / Cat SVG skins are original work and the swap is one line.

### Credits

- 🎙️ **Nova-3** by [Deepgram](https://deepgram.com) and **Whisper Large v3 Turbo** by [OpenAI](https://openai.com), hosted by [Groq](https://groq.com)
- 🦙 **Llama 3.3** by [Meta AI](https://ai.meta.com)
- ✨ **Gemini** by [Google DeepMind](https://deepmind.google)
- 📎 **Clippy sprite** via [clippyts](https://github.com/pi0/clippyts) (vendored + patched)
- 🦀 **Tauri 2** + **Svelte 5** — the lean desktop stack

</details>
