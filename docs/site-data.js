window.PRODUCT_SITE = {
  name: "wispr-fox",
  mark: "WF",
  kicker: "Free desktop dictation with your own AI key",
  headline: "Talk anywhere on your desktop. Get clean text where you need it.",
  subhead: "wispr-fox turns a hotkey into quick dictation for emails, notes, chats, and documents. Record, transcribe, clean up, and paste without signing into another closed dictation service.",
  repoUrl: "https://github.com/kumaradarsh1993/wispr-fox",
  scene: "dictation",
  theme: {
    bg: "#f7f7fb",
    ink: "#171826",
    accent: "#2357d8",
    accent2: "#0d8f7b",
    accent3: "#d85257"
  },
  downloads: [
    {
      label: "Download for Windows",
      note: "Stable v1.3.0 installer",
      href: "https://github.com/kumaradarsh1993/wispr-fox/releases/download/v1.3.0/wispr-fox_1.3.0_x64-setup.exe"
    },
    {
      label: "Download for macOS",
      note: "Apple silicon DMG",
      href: "https://github.com/kumaradarsh1993/wispr-fox/releases/download/v1.3.0/wispr-fox_1.3.0_aarch64.dmg"
    },
    {
      label: "Download for Linux",
      note: "AppImage",
      href: "https://github.com/kumaradarsh1993/wispr-fox/releases/download/v1.3.0/wispr-fox_1.3.0_amd64.AppImage"
    },
    {
      label: "Beta builds",
      note: "Advanced releases",
      href: "https://github.com/kumaradarsh1993/wispr-fox/releases"
    }
  ],
  secondary: [
    { label: "View source", href: "https://github.com/kumaradarsh1993/wispr-fox" },
    { label: "All releases", href: "https://github.com/kumaradarsh1993/wispr-fox/releases" }
  ],
  stage: {
    title: "Dictation console",
    status: "Stable download ready",
    rail: [["Hotkey", "Record"], ["Clean", "Rewrite"], ["Paste", "Anywhere"]],
    surfaceTitle: "Voice to text",
    tiles: ["wave", "text", "mode", "history", "provider", "paste"],
    note: "Built for the everyday moments where typing is slower than thinking out loud."
  },
  storyTitle: "Desktop dictation without the ceremony",
  storyIntro: "Keep your hands on the keyboard. Trigger dictation, say the thing, and send the cleaned text into the app you were already using.",
  chapters: [
    {
      title: "Press the hotkey",
      body: "Start recording from anywhere on the desktop. You do not need to move your workflow into a new editor."
    },
    {
      title: "Choose the output",
      body: "Use raw transcription for speed, cleanup for readable prose, or drafting when you want the app to shape the text."
    },
    {
      title: "Paste into the right place",
      body: "The result lands where you need it: messages, notes, documents, forms, and other everyday writing surfaces."
    }
  ],
  downloadTitle: "Download the stable build",
  downloadIntro: "Stable is the right default. Beta releases are for newer features and advanced testing before promotion.",
  panels: [
    {
      title: "Stable",
      body: "Recommended for daily dictation and normal writing workflows."
    },
    {
      title: "Beta",
      body: "Useful when you want the newest improvements and can tolerate early-release rough edges."
    },
    {
      title: "Bring your own key",
      body: "Use supported providers such as Groq or Gemini from your own account instead of depending on a hosted app login."
    }
  ],
  setupTitle: "Get speaking",
  setupIntro: "A short setup, then the app can sit quietly until the hotkey is pressed.",
  setup: [
    { title: "Install wispr-fox", body: "Use the stable build for your operating system." },
    { title: "Add an AI provider key", body: "Open settings and enter your Groq or Gemini key." },
    { title: "Pick a hotkey", body: "Choose a trigger that is easy to remember and does not conflict with your other apps." },
    { title: "Speak and paste", body: "Record, let the app process, and send the text into your active app." }
  ],
  footer: "Open-source desktop dictation for fast writing without lock-in."
};
