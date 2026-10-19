# wispr-fox v3.5.0

**Pause and resume, a calmer Home, sign-in that sticks, and current models.**

This is the stable roll-up of `v3.5.0-nightly.1` through `nightly.8`. If you
are on any nightly in that range, nothing here is new to you. Update anyway so
the app stops reporting itself as a pre-release.

## Dictation

**Pause and resume.** Press **Ctrl+Option+Space** on a Mac (**Ctrl+F8** on
Windows) while dictating, and the microphone is released. Press it again to
carry on in the same recording. **Option+Space** or **Escape** finishes from
either state. The microphone is really off while paused, and your OS mic
indicator goes out. All the stretches are joined into one file before upload,
so the provider gets a single request however many times you paused. The fox
stays on screen, dimmed, and says how many parts are saved. The key is
rebindable in Settings → Dictation.

**Interrupted recordings are kept.** A recording cut off by a crash or a
forced quit used to come back as "contains no audio", because the file's
length is only written when recording stops. The app now repairs that the
first time you open, play or re-transcribe the file. A session that was
paused when the app was killed is welded back together on the next launch.

## Home

**The note's name leads each card.** Five markers on every row became one
chip that appears only on unusual rows (Failed, Meeting, Uploaded). Everything
that was removed from the card is still in Details. Action icons stay dimmed
until you hover a row. The Raw / Cleaned / Drafted tabs stay on the card.

**The sidebar keeps your controls.** Engine, Polish, microphone and avatar are
in the left pane, with the dictation keys and the fox. The nav reads
**Home · Insights · Settings**, with an account chip at the bottom.

**Filters say what they mean:** All · Dictations · Drafts · Meetings ·
Uploads · Failed. **Clear all** moved to Settings › App & data › Danger zone.

**The fox stays where you put it.** It used to creep up and to the left once
per session, because the app saved the corner of a window that grows when a
bubble or menu opens. It now saves the point the fox stands on, keeps it on a
real monitor, and pulls it back if a display is unplugged.

## Sign-in and keys

**Sign-in survives a restart.** The keychain library had been built with no
backend, so the sign-in token lived only in memory and was gone the next
morning. API keys are now really in the macOS Keychain and Windows Credential
Manager. Keys are read once per launch, so there is no password prompt per
dictation. If macOS asks once after updating, click **Always Allow**.

**Sign-in works on networks that block Supabase.** Several Indian providers
block the sync service's addresses. The app now looks them up through
encrypted DNS (Cloudflare, then Google) and still checks the real certificate.

## Models

Checked against each provider's documentation on 19 October 2026.

- **Groq cleanup works again.** Its models were spending their whole reply
  budget thinking, or returning their reasoning inside the answer. Llama models
  are Enterprise-only now, so GPT-OSS 20B and 120B are the defaults, with Qwen
  3.8 as a preview option. Qwen 3.6 was shut down and switches to 3.8.
- **OpenAI cleanup works.** The GPT-5.4 and 5.5 ids we offered did not exist.
  The list is the GPT-5.6 family: Terra, Sol and Luna.
- **Gemini defaults to 3.8 Flash.** 3.7 Flash and 3.5 Flash are removed,
  because Google quietly routes them to other models.

Saved choices that name a removed model switch to its replacement on their
own.

## Retention

New installs keep recordings for 3 days instead of 7. Existing installs keep
their setting.
