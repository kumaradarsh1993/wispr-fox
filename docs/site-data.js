window.PRODUCT_SITE = {
  name: "wispr-fox",
  mark: "WF",
  product: "dictation",
  kicker: "Free, open-source desktop dictation",
  headline: "Press a key. Talk. Get text where your cursor already is.",
  subhead: "wispr-fox is for people who write more than they want to type. Hold a hotkey, speak naturally, and send a raw transcript, cleaned note, or polished draft into Gmail, Slack, docs, forms, or wherever you were already working.",
  insight: "The user promise is simple: high-quality speech-to-text without a monthly dictation subscription, without an account, and without moving your writing into someone else's editor.",
  repoUrl: "https://github.com/kumaradarsh1993/wispr-fox",
  scene: "dictation",
  theme: {
    bg: "#f7f7fb",
    ink: "#171826",
    accent: "#2357d8",
    accent2: "#0d8f7b",
    accent3: "#d85257"
  },
  assets: {
    avatar: "images/fox-recording.png",
    logo: "images/fox-logo.png"
  },
  downloads: [
    {
      label: "Download for Windows",
      note: "Stable v2.0.0 installer",
      href: "https://github.com/kumaradarsh1993/wispr-fox/releases/download/v2.0.0/wispr-fox_2.0.0_x64-setup.exe"
    },
    {
      label: "Download for macOS",
      note: "Apple silicon DMG",
      href: "https://github.com/kumaradarsh1993/wispr-fox/releases/download/v2.0.0/wispr-fox_2.0.0_aarch64.dmg"
    },
    {
      label: "Download for Linux",
      note: "AppImage",
      href: "https://github.com/kumaradarsh1993/wispr-fox/releases/download/v2.0.0/wispr-fox_2.0.0_amd64.AppImage"
    },
    {
      label: "Beta builds",
      note: "Newer, less tested",
      href: "https://github.com/kumaradarsh1993/wispr-fox/releases"
    }
  ],
  secondary: [
    { label: "View source", href: "https://github.com/kumaradarsh1993/wispr-fox" },
    { label: "All releases", href: "https://github.com/kumaradarsh1993/wispr-fox/releases" }
  ],
  proof: [
    "Raw transcript, cleanup, and drafting hotkeys",
    "Bring your own key: Deepgram, Groq, OpenAI, and more",
    "Fox, Paperclip, real Clippy, and cat avatars",
    "No telemetry, account, or hosted app login"
  ],
  hero: {
    title: "Hold to dictate",
    status: "Ready in any app",
    apps: ["Mail", "Slack", "Docs", "Browser", "Terminal"],
    modes: [
      ["F8", "Raw transcript"],
      ["F9", "Draft from a brief"],
      ["Shift+F8", "Clean punctuation"]
    ],
    transcript: [
      "can you turn this into a crisp project update",
      "Keep my tone. Fix grammar. Add bullets.",
      "Paste it into the active window."
    ],
    output: "Here is the short version: the landing page should explain the product first, then show exactly how to download and use it."
  },
  storyTitle: "The actual flow",
  storyIntro: "The page should feel like the app: quick, practical, slightly playful, and built around the moment where speaking is faster than typing.",
  beats: [
    {
      title: "Start from the app you are already using",
      body: "Hold F8 on Windows or Control-Option-D on Mac. The avatar wakes up, the waveform starts, and you speak without opening a separate editor.",
      tag: "Hotkey",
      visual: "listen"
    },
    {
      title: "Pick the amount of AI help",
      body: "Use raw transcription for speed, one-shot cleanup for punctuation and paragraphing, or drafting when you want a message shaped from a rough brief.",
      tag: "Modes",
      visual: "modes"
    },
    {
      title: "Send the text back to your cursor",
      body: "The result lands in the active text field: email, chat, notes, docs, forms, tickets, or any regular writing surface.",
      tag: "Paste",
      visual: "paste"
    },
    {
      title: "Make the assistant feel yours",
      body: "Pick the avatar style: Fox, Paperclip, original Clippy, Desk Cat, or the cat variants. It is useful, but it is allowed to have charm.",
      tag: "Avatar",
      visual: "avatar"
    }
  ],
  downloadTitle: "Start with the stable build",
  downloadIntro: "Most people should install the stable release first. Beta builds are where provider, usage, and avatar experiments appear before they are promoted.",
  panels: [
    {
      title: "Free speech-to-text",
      body: "Services like Groq and Deepgram offer generous free tiers and credits: Groq is free forever for daily dictation, and Deepgram's $200 signup credit covers heavy use for a year or more. Any spend stays under your own provider account, not a wispr-fox subscription."
    },
    {
      title: "Private by design",
      body: "No hosted wispr-fox account. API keys stay in your OS keychain or encrypted local fallback. Audio is sent only when you choose to dictate."
    },
    {
      title: "Personality without lock-in",
      body: "The avatar is part of the product feel, but the app remains open source and practical: hotkeys, models, prompts, history, and settings are inspectable."
    }
  ],
  setupTitle: "Two-minute setup",
  setupIntro: "Install the app, paste a provider key, choose your hotkeys and avatar, then keep writing wherever you already work.",
  setup: [
    { title: "Install wispr-fox", body: "Use the stable download for your operating system." },
    { title: "Add a provider key", body: "Groq gets you started free forever; Deepgram Nova-3 (with $200 signup credit) is the recommended transcription upgrade. Groq or Gemini handle cleanup and drafting for free." },
    { title: "Pick hotkeys and avatar", body: "Defaults work, but Settings lets you adjust dictation modes and choose Fox, Paperclip, Clippy, or Cat." },
    { title: "Speak into any text field", body: "Hold the hotkey, talk, release, and let wispr-fox paste the result back where your cursor is." }
  ],
  footer: "Open-source desktop dictation for fast writing without lock-in."
};
