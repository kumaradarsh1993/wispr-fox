# Codex handover - 2026-06-29

This file is for Claude Code (or any future assistant) taking the repo back
after the Codex handoff. It records the user request, what Codex changed, and
why the key-storage changes were made.

## User prompt captured for handoff

The user first asked Codex to take over from the Claude Code line, audit the
repo, keep a clear Claude checkpoint, expand providers, keep future work on
nightly, label Codex-authored releases clearly, and later look at avatar/pet
work. In this checkpoint turn, the user then gave the following instruction:

```text
So first of all, I would like you to create a... Can you please read this Claude sort of, I'll write. This is because you sort of took over the role from Claude. I might want to go back to it later. Every time... So this is a checkpoint, right, where we have done something important, done some edits to the thing. So we are go ahead and commit or sort of tell Claude that these changes have been done so that when I hand it over back to Claude at some point of time, any and all changes that you keep doing, reverting, etc., etc., right, that is known by Claude. And why you did it. You should also write what prompt I gave to you. You just dump it there. And then you take into consideration, like just walk through high-level details of what you ended up doing on Claude, like broadly do that file creation. Secondly, with respect to Windows, Key Management, we were having a lot of trouble. I see in your change log that you mentioned plain text key fallback. Like that is something which has been, I don't remember exactly, but vaguely, which has been an issue with Microsoft, where I'll tell you, during the onboarding flow, I would go ahead and implement, like this is older version where Grok is only working, but at least Grok was working. So when we did not have that fallback. And again, I'm not asking you to reintroduce that fallback, but like to fundamentally solve for this problem. And you can go as deep and implement dynamic programming, sorry, not dynamic programming, but dynamic coding to sort of solve for this. The problem statement was very simple. Before this fallback existed, we had a few releases where I would go ahead and enter the Grok key. So in the onboarding flow or in the settings, I would go and enter the Grok key, and then I would press save. And then somehow I don't think the Windows key manager was writing it, or I don't know whether it was not able to retrieve it, but the system would not work. Basically, the key would not be there. And I don't know if the code in that, like the other engineer tried to it for a bit but could not. So I want you to take a fresh attempt at it. Clearly audit how the key management system and service, like storage, recall, working, etc., etc., is there. Also put in some user-visible logs, right? You can have a log screen of sorts. Not an issue, you can introduce that as a feature under settings or, you know, top-right corner, how typically people do it, so that whatever I'm doing in terms of entering key, removing key, etc., etc., is all working. I will also use, like, whatever keys I use, and just remember this as a precedence, I will be using testing keys for debugging or otherwise. So it's fine. Even if I give you keys, I will rotate them after we are done with this entire process. But yeah, the end goal is to make it work in a secure manner and no plain text fallbacks, because I think even on GitHub a friend highlighted that my keys are there in plain text on GitHub because of the fallback, which becomes a problem, right? Worst case fallback is that locally it should be there as a fallback key when I enter, and then the system should be designed such that if Windows Key Manager doesn't work, then at least the key manager or the key should be stored as plain text locally, but at no point of time it should be leaked onto GitHub. And you should also check my GitHub if it is containing my plain text key, right? I think that is one problem that we should get rid of in general.
```

## State before this checkpoint

- `c2d33e3` is the explicit final Claude Code avatar checkpoint.
- `v1.4.0-nightly.6` at `29531eb` is the Codex provider expansion nightly.
- `3724124` (`Make landing page customer-first`) was already on `main` and
  `origin/main` when this key-storage work began.
- No stable promotion was requested. Continue using nightly tags for Codex work.

## What Codex had already done in this handoff

- Added STT providers: OpenAI GPT transcription, Deepgram Nova, and ElevenLabs
  Scribe.
- Added OpenAI cleanup/drafting through the Responses API.
- Expanded the Providers settings page with provider/model pickers and per-key
  connection tests.
- Kept the new Khaumani & Indy animated avatar/pet work in the avatar picker.
- Removed transcript logging, fixed Gemini key tests to avoid URL key exposure,
  removed unused filesystem plugin exposure, and fixed double-digit nightly
  update comparisons.
- Tagged and published `v1.4.0-nightly.6` as a Codex-authored prerelease.

## What this checkpoint changes

- Reworked `src-tauri/src/secrets.rs` so keyring writes are still verified by
  readback, but Windows fallback storage is now DPAPI-encrypted at
  `%APPDATA%/com.wispr-fox.app/.keys.enc.json`.
- Kept legacy `.keys.json` support as migration-only. If a key is found there,
  the app tries to move it to the OS keyring first; if that still does not
  verify, it moves it into the encrypted local fallback and deletes the legacy
  plaintext file only after replacement readback succeeds.
- Added a no-secret audit trail at
  `%APPDATA%/com.wispr-fox.app/secret-audit.jsonl`. Events include key label,
  storage location, action, outcome, and a sanitized detail string. API key
  values are never written.
- Added a new `secret_audit_log` Tauri command and TypeScript wrapper.
- Added Settings -> Security to show keyring status, each key's storage
  location, fallback paths, and recent no-secret key-storage events.
- Updated Providers -> Key storage status to distinguish OS keyring,
  encrypted fallback, legacy plaintext fallback, and not saved.
- Added `.gitignore` guardrails for `.keys.json`, `.keys.enc.json`, and
  `secret-audit.jsonl`.
- Updated docs that described the old plaintext fallback.

## Why this design

The older failure pattern was probably not just "keyring call returned an
error"; it may also have been "keyring write returned success but did not
survive readback." Codex preserved the verified-write logic because it directly
addresses that failure mode. The important change is that the fallback no
longer needs to be plaintext on Windows. DPAPI keeps the fallback local to the
same Windows user profile while still allowing the app to function if Windows
Credential Manager is broken or unreliable.

The legacy plaintext fallback is intentionally not deleted until a replacement
is verified. This avoids the earlier class of bug where the app tried to be
more secure, deleted the only working copy, and then later reported "no key
saved."

## GitHub/plaintext-key audit

On 2026-06-29 Codex refreshed refs with `git fetch --all --tags --prune` and
ran value-suppressed scans:

- Current checked-out tree: no matches for common Groq/OpenAI/Gemini/ElevenLabs
  token patterns.
- Full fetched git history: `NO_SECRET_PATTERN_HITS`.
- Full fetched history for `.keys.json`, `.keys.enc.json`, and
  `secret-audit.jsonl`: `NO_KEY_FALLBACK_FILES_IN_HISTORY`.
- GitHub secret-scanning alerts API for `kumaradarsh1993/wispr-fox`: returned
  an empty list.

This is not a mathematical proof that no unusual token format ever existed, but
it is clean for the provider key patterns and fallback filenames that matter for
this app.

## Verification performed

- `cargo check` in `src-tauri`: passed. Existing warnings remain in unrelated
  modules.
- `npm run check`: passed with 0 errors. Existing warnings remain in unrelated
  History/onboarding/settings files.

Do not run `npm run tauri build` locally on this machine unless the user
explicitly asks; the workspace instructions say CI should build installer
artifacts.

## Follow-up notes for Claude

- If the user reports the key still disappears, check Settings -> Security
  first. The audit rows should show whether the save verified in keyring,
  fell back to encrypted file, or failed fallback readback.
- Avoid reintroducing plaintext fallback as a normal path. The only plaintext
  handling now should be legacy migration from `.keys.json`.
- Keep future Codex-authored release names/descriptions visibly marked Codex.
- Stable promotion still requires an explicit user signal.
