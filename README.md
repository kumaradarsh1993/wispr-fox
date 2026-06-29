<h1 align="center">wispr-fox 🦊</h1>

<p align="center">
  <strong>Press a key. Talk. Get text.</strong> Anywhere on your computer.
</p>

<p align="center">
  A free, open-source dictation app for Windows and macOS.<br/>
  Bring your own AI key. No subscription. No telemetry. No account.
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

| Platform | 🟢 Stable (recommended) | 🟡 Beta builds (newer, less tested) |
|---|---|---|
| 🪟 **Windows** | [**wispr-fox setup.exe**](https://github.com/kumaradarsh1993/wispr-fox/releases/download/v1.3.0/wispr-fox_1.3.0_x64-setup.exe) | [Browse beta builds →](https://github.com/kumaradarsh1993/wispr-fox/releases) |
| 🍎 **macOS** (Apple Silicon) | [**wispr-fox.dmg**](https://github.com/kumaradarsh1993/wispr-fox/releases/download/v1.3.0/wispr-fox_1.3.0_aarch64.dmg) | [Browse beta builds →](https://github.com/kumaradarsh1993/wispr-fox/releases) |
| 🐧 **Linux** | [**wispr-fox AppImage**](https://github.com/kumaradarsh1993/wispr-fox/releases/download/v1.3.0/wispr-fox_1.3.0_amd64.AppImage) | [Browse beta builds →](https://github.com/kumaradarsh1993/wispr-fox/releases) |

Run the installer. The app walks you through a 2-minute onboarding on first launch.

> ℹ️ **Mac first-launch:** the build isn't code-signed (Apple charges $99/yr for that). After dragging to Applications, right-click → Open the first time. If you see *"app is damaged"*, run `xattr -dr com.apple.quarantine /Applications/wispr-fox.app` once and double-click as normal.

---

## ✨ What it does

Hold a hotkey, talk, release. The text appears in whatever app you're in — Slack, Gmail, Word, your terminal, anywhere there's a text field.

| Press | What you get |
|---|---|
| 🎙️ **F8** *(Win)* / **⌃⌥D** *(Mac)* | Raw transcript — exactly what you said |
| ✏️ **F9** *(Win)* / **⌃⌥F** *(Mac)* | Drafted output — give a brief, get a polished email/doc/message back |
| 🧹 **Shift+F8** / **⌃⌥C** *(Mac)* | One-shot cleaned version (spell + punctuation + paragraphing) |
| ⏏️ **Esc** | Stop a recording in flight |

Add `Win` (or `Shift` on Mac) to any combo to make it sticky — tap once to start, tap again to stop.

---

## 🚀 Setup in 3 steps

**1. Install.** Download the installer above and run it.

**2. Get a free API key.** Onboarding will deep-link you to <https://console.groq.com/keys> — sign in with Google or GitHub, click "Create API Key", paste it back into wispr-fox.

**3. Press your hotkey and talk.** That's it.

<p align="center">
  <img src="docs/images/onboarding.png" alt="Onboarding flow — Foxy walks you through three steps" width="560" />
</p>

---

## 💸 How is this free?

You bring your own AI provider key. Groq's free tier gives you **14,400 transcription requests/day** — that's about 20 hours of dictation, daily. Most people never hit the limit. Heavy daily use: **$3–8/month** if you exceed the free tier. Most users pay **$0**.

---

## 🤫 Privacy

- 🔐 **API keys** live in your OS keychain — Windows Credential Manager / macOS Keychain. Never logged, never synced.
- 🎧 **Audio recordings** stay on your machine. Default: 7-day retention, 500 MB cap, both configurable.
- ☁️ **Only the audio you choose to dictate** is sent to your chosen provider (Groq / Gemini) for transcription. Read their privacy policies — they're the parties that see the audio.
- 📡 **wispr-fox phones nothing home.** No analytics. No crash reporting. No account. There is no "us" with a server. The repo, the binary, your machine — that's the whole stack.

---

## 🎨 Make it yours

- 🦊 **Pick a floater** from the sidebar: Off, Fox (default), Paperclip, real Clippy, Desk Cat, or the experimental Cat (lab).
- 🌗 **Dark, Light, or Retro themes** in Settings → Appearance.
- ⌨️ **Rebind any hotkey** — Settings → Dictation. Defaults are sensible on each platform.
- 🎭 **Customise the LLM prompts** per mode if you want a specific tone — Settings → Modes → Show system prompt.
- 🚀 **Launch at login** — Settings → General.

---

## 🛟 Common questions

<details>
<summary><strong>The Mac hotkey does nothing — F8 just plays/pauses music</strong></summary>

That's macOS treating the function row as media keys by default. The Mac defaults are **⌃⌥D** (transcribe) and **⌃⌥F** (draft) for exactly this reason. If you'd rather use F-keys: System Settings → Keyboard → Keyboard Shortcuts → Function Keys → toggle "Use F1, F2, etc. as standard function keys". Or rebind in wispr-fox Settings → Dictation.

</details>

<details>
<summary><strong>Transcription starts with random "Thank you" or other phrases</strong></summary>

Whisper hallucinates on a silent buffer. On Windows, Realtek audio enhancements can put your mic to sleep between recordings — open `mmsys.cpl`, find your mic, uncheck "Allow exclusive control" + check "Disable all enhancements".

</details>

<details>
<summary><strong>Wrong language detected</strong></summary>

Whisper auto-detects by default. If you only speak one language and it's mis-detecting, set Settings → Models → Language Hint.

</details>

<details>
<summary><strong>Want to use a different AI provider (Gemini)?</strong></summary>

Settings → Providers & Keys → add a Gemini key from <https://aistudio.google.com/apikey>. Gemini handles drafting/cleanup; Groq still handles transcription (Gemini doesn't expose Whisper-equivalent yet).

</details>

<details>
<summary><strong>Why is the macOS build unsigned?</strong></summary>

Apple charges $99/year for a Developer ID certificate. We're not there as a free single-developer project. macOS treats unsigned apps as quarantined on first launch — the `xattr` workaround above gets past it once and forever for that install.

</details>

---

## 📦 What's next

See the [**Roadmap**](docs/ROADMAP.md) for what's planned. Highlights: time-saved / words-saved stats, Sarvam Saaras as a Hindi-friendly STT provider, plugin-based avatar system.

---

<details>
<summary><h2 style="display:inline">🧑‍💻 For developers</h2></summary>

### Build from source

```bash
# Prerequisites: Rust 1.75+, Node 20+, pnpm
git clone https://github.com/kumaradarsh1993/wispr-fox
cd wispr-fox
pnpm install
pnpm tauri dev      # development
pnpm tauri build    # production binary (heavy, needs ≥16 GB RAM)
```

Full dev notes: [GETTING_STARTED.md](./GETTING_STARTED.md).

### Architecture, 90-second tour

- **Frontend** — SvelteKit + Svelte 5 (runes). Routes: `/` redirects to onboarding or history, `/clippy` is the always-on-top floater, `/settings/*` is the config UI.
- **Backend** — Rust + Tauri 2. Modules: `audio/` (cpal capture → WAV), `stt/groq.rs` (Whisper Large v3 Turbo), `llm/` (Groq Llama + Gemini), `inject/` (SendInput on Windows, CGEvent on macOS), `flow.rs` (state machine), `hotkey.rs` (global shortcuts), `secrets.rs` (keychain).
- **Storage** — SQLite for history, `tauri-plugin-store` for settings, OS keychain for API keys.
- **CI** — Every tag push builds Win NSIS + macOS DMG + Linux installers on GitHub Actions.

### License

MIT for the code we wrote. The vendored Microsoft Clippy sprite is © Microsoft — included under fair-use precedent (nostalgia / non-commercial reference). If Microsoft objects, the hand-drawn Paperclip / Fox / Cat SVG skins are original work and the swap is one line.

### Credits

- 🎙️ **Whisper Large v3 Turbo** by [OpenAI](https://openai.com), hosted by [Groq](https://groq.com)
- 🦙 **Llama 3.3** by [Meta AI](https://ai.meta.com)
- ✨ **Gemini** by [Google DeepMind](https://deepmind.google)
- 📎 **Clippy sprite** via [clippyts](https://github.com/pi0/clippyts) (vendored + patched)
- 🦀 **Tauri 2** + **Svelte 5** — the lean desktop stack

</details>
