# wispr-fox v3.6.0-nightly.2

## Fix: repeated Keychain password prompts on macOS

**What you saw:** opening wispr-fox asked for your Mac password up to 4 times, often on every launch.

**Why:** wispr-fox is signed with its own certificate, not an Apple developer account. For apps like that, macOS ties each Keychain item to the exact app build that created it. Every update is a new build, so every saved key (Groq, Gemini, Deepgram, and the sync sign-in token) asked for your password separately. Pressing Return after typing the password picks "Allow" instead of "Always Allow", so the prompts came back on every launch.

**What changed:**
- All saved keys and the sync token now live in one Keychain item, created by the running build.
- After an update, you get at most one prompt, on first launch. The new build copies the old item into one it owns, then deletes the old one. Later launches of that build don't prompt.
- The first launch of this version still asks once per old item (up to 4 times), because those items belong to the old builds. After that, launches don't prompt.
- If you deny a prompt, nothing is lost: that item is kept and read again on the next launch.

Windows and Linux are unchanged.
