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

## Settings/sidebar checkpoint - Codex v1.4.0-nightly.8

The next user prompt asked for a P0 cleanup of Settings and the left sidebar,
with a specific emphasis on reducing information overload and making the
provider/usage area genuinely useful:

```text
Hey, so two things. We need to do a UI cleanup on this one, especially within settings. So take a comprehensive look at all the settings. You might want to rejig two things. One is where each of the settings reside, what tabs specifically we retain, where each of the settings sit within each of the tabs, what is the layout of each of the settings so that it's not information overload. This is like a P0 for us to solve for right now. Even on the, we are not touching homepage, we're just touching the settings submodule. And we are sort of doing things there. On the left-hand path, right, which is the left sub bar, we have a usage tracker also, right, where model, I think, where to the bottom left we show which speech-to-text and which cleaner we are using. That space needs to be tweaked a little. That's going to become a major feature for us now because I was thinking, why just use it to show information? So there are three parts. So we are rewriting that. But that entire module can move. So A, it should be always visible, right? Like it should be a part of a homepage itself. It can keep sitting there or it can actually become, you know, in continuation with how wherever we have put up the floater, etc., etc., right? So rather than being anchored to the bottom, it can be anchored inline. So history, stats, settings is fine. Then where you talk about hold to dictate, that section is fine. Then we provide the floater or avatar. We should name that avatar and not floater. Then there is two cat modules, right? Orugujia or I don't know whatever that is named, KhauMoney and IndieStar. There is one older one and then there is a newer one. Newer one looks shitty, just delete that. Then below that, there has to, so today's usage still stays at the bottom as an anchor. But that is slightly dicey and I'll tell you more nuances about it. But yeah, this speech-to-text picker and, so call it, so instead of speech-to-text, yeah, STT and LLM is fine. But we need to provide a picker there, right? And it has to be a dynamic dropdown. And in general, like the theme has to be nicer, cleaner, where the speech-to-text provides me a picker of sorts of what service and model I need to pick. Similarly, the LLM provides same thing, right? So now it provides a picker, and then against the LLM, there's a checkbox, which basically is the toggle that we have in settings, which is whether I want to auto-clean or not, right? So that toggle, that checkbox, we need takes care of it, and says keep the preference. And yeah, last thing, the usage part, right? So the admin module, whatever you're talking about, so that is taken care of there. Can be bottom, but you can take care of the UI. The usage part, and then correcting the post problem, there is, you know, there's an insight that in there's probably usage and that's onboarded, and then as well as the speech-to-text. But in case of Deepgram or something, there's a $200 limit that they have given us, right? So that you should track against $200 on how much usage we are doing against $200. You might need to check what is the translation rate. I can also validate. Okay. So and yeah, one tactical thing, the top bar, with most of that, right, which shows the prompts or something with the close button right at the top. That color does not change with the system theme. That looks odd. Can you please take care of it with the system theme matching colors, whatever thematic options we have should comply.
```

### What Codex changed for the settings/sidebar checkpoint

- Reworked the Settings IA to six tabs: Providers, Modes, Dictation, Avatar,
  General, and Security. The old Data tab now redirects into General, where
  retention/storage controls live.
- Made Providers less overwhelming: active STT and LLM service/model selectors
  are the first two cards; API keys are tucked behind a collapsed "Manage API
  keys" disclosure; key-storage diagnostics remain in Security.
- Added shared frontend provider metadata in `src/lib/provider-options.ts` so
  the sidebar and Providers page use the same STT/LLM providers, model lists,
  readiness checks, and labels.
- Turned the old passive sidebar "active models" footer into an inline Models
  panel: STT service/model picker, LLM service/model picker, a "Clean" checkbox
  for `auto_clean_in_light`, and a Keys shortcut. The controls disable while a
  dictation flow is active to avoid mid-flight provider swaps.
- Kept usage anchored at the bottom. Non-Deepgram STT continues to show the
  local daily call counter. Deepgram now shows an estimated cumulative spend
  against a $200 credit using the official Nova-3 multilingual pre-recorded
  price of $0.0092/min, recorded after successful Deepgram transcriptions.
- Renamed user-facing "Floater" settings/sidebar labels to "Avatar."
- Retired the newer `duo-hd` Khaumani & Indy variant from selectable UI. Saved
  `duo-hd` values migrate back to the older `duo` avatar. The deep SVG/CSS
  implementation remains inert in `/clippy` for now; no user-visible picker can
  select it.
- Rewrote the Dictation settings page to reduce duplicated copy and stale macOS
  hotkey language, while keeping the same controls.
- Added responsive settings breakpoints so the settings shell collapses cleanly
  on narrower windows.
- Updated the macOS permission banner/top notice styling to use theme tokens
  instead of hard-coded warning colors.

### Verification for settings/sidebar checkpoint

- `npm run check`: passed with 0 errors. Existing unrelated warnings remain in
  History, onboarding, and the local missing `@types/node` setup.
- `npm run build`: passed. Existing bundle/a11y/unused-selector warnings remain
  in unrelated areas.
- `cargo check` in `src-tauri`: passed. Existing unrelated Rust warnings remain.

### Notes for the next assistant

- The Deepgram usage number is an estimate for the selected Nova-3 multilingual
  pricing, not a live Deepgram billing API readout.
- If the retired `duo-hd` code becomes distracting, prune it in a dedicated
  avatar-code cleanup pass. This checkpoint intentionally only removed it from
  user-selectable surfaces and added migration.

## Settings/sidebar polish checkpoint - Codex v1.4.0-nightly.9

The next user prompt asked for follow-up settings polish, visible usage
improvements, native titlebar theme sync, and a first look at Codex-style pet
assets:

```text
Hey, good job in the cleanup and the settings, etc., etc. Few minor changes from a settings point of view. One, the, under security key event log, let's just put it under some sort of expandable or so. We don't need to see this properly. But by the way, I'm just pasting key event log for you to decipher what's happening with the app. After this, then under general, I think rest of the things seem to be pretty fine. Yeah, I think that's fine. Under avatar, let's just get rid of cat lab. And yeah, by the way, all these assets that have been created were created by Claude, who is not a bad at doing illustrations and infographics. And because you have access to much better models for image generation, would you rather take a shot at doing some interesting illustrations? In fact, you might remember I told you that Claude, sorry, not Claude, Codex, which is technically you, has this whole pet feature. Your files would already have it. See how, I mean, you might have, want to go back to your app files and your installation files and see how what sort of assets have been created, what sort of harness has been created to host those assets, etc., etc. Because I really, really like that implementation. You can ask me for additional, you know, sort of permissions to go through my C drive installation for your directory, which is Codex, and then figure out, you know, what sort of assets are kept hosted, what harness, and all the detailing around it. We want to implement something very similar here. So two things, right? One is within my existing SDK, create a new avatar, cat and foxes, fine. I hope you have access to image gen models as a part of it. If not, then maybe just do whatever you can to sort of create those. But yeah, what is required is I want to see what kind of quality you bring to these avatars. Secondly, if you can reverse engineer whatever is done under Codex, under pet, I don't think that's like something very secret, right? It should be ethical to just see what has been done, the quality of assets. In fact, you can actually copy of the assets from there. Again, this is more of a pet project that we have. Lastly, the top bar, the whisper box with the minimize, maximize, close button, etc., etc. That color is still not changing with the overall theme. It's not syncing up. It should not be the same, it should be complementary of sorts, right? Imagine how you would typically have a color code for it. Then left-hand panel, I think we can have a thing where I can drag to expand and collapse using that left-hand bar. Because right now the model space has become too congested. The deepgram, Nova 3, both like over the text, the arrows sort of coincide. So you can make it wider and make it adjustable. And deepgram credit is cumulative thing, speech-to-text LLM. I think we should have some sort of a, you know, sort of overall usage thing, tokens. I'm not sure if Deepgram returns how many tokens have been used and what's the cost per token, etc., etc. Same thing for Grok and all. But if they do, then just ask for it and keep a track of it. Imagine it very similar to how we track words and length, etc., etc. That similar way we are using, we are tracking usage by model for both STT and LLM on a day-wise basis. So a log of that is kept. These are the few things that we need to work with. All right, cool.
```

### What Codex changed for nightly.9

- Made Settings -> Security's key event log a collapsed disclosure by default.
  It still shows a recent-event count in the summary and never shows secret
  values.
- Removed `cat-lab` from selectable Avatar surfaces (sidebar, Settings ->
  Avatar, and the floater context menu). Saved `cat-lab` values migrate back to
  `cat`. The old implementation remains in `/clippy` only as inert legacy code.
- Removed the fixed `"theme": "Light"` from the Tauri main window and added
  frontend native theme sync via Tauri's app `setTheme` API. `dark` maps to
  native dark, `light`/`retro` map to native light, and `auto` follows system.
- Made the sidebar drag-resizable, with a wider default and a wider minimum so
  model dropdown text and native select arrows no longer collide. Keyboard
  arrows resize the sidebar when the resize handle is focused.
- Moved the decorative sidebar fox/replay-onboarding extras into the scrollable
  sidebar area and kept the anchored bottom block limited to usage meters. This
  fixed the 720px-tall viewport overlap where the LLM picker could sit under
  the bottom usage block.
- Reworked `usage.json` into a backward-compatible `UsageFile { today, days }`
  shape with per-day `model_usage` buckets. Each bucket records stage (`stt` or
  `llm`), provider, model, call count, STT audio seconds, LLM token counts, and
  conservative estimated cost where available.
- STT recording now stores provider/model/audio seconds after successful
  transcription. Deepgram still updates the cumulative $200-credit estimate.
- LLM cleanup/drafting now records provider/model and token usage when returned
  by Groq chat completions, OpenAI Responses, or Gemini `usageMetadata`.
- The sidebar bottom readout now reflects the active model's audio/tokens/calls
  instead of only coarse global call counters.

### Codex pet/assets investigation

- `C:\Users\kadar\.codex\pets` existed but was empty at inspection time.
- The installed Codex app resources included Lottie-related third-party notices,
  but no obvious exposed pet/mascot/avatar assets were found outside bundled app
  internals. Codex did not copy bundled Codex artwork into wispr-fox.
- Codex generated a fresh wispr-fox avatar concept sprite sheet for direction
  at:
  `C:\Users\kadar\.codex\generated_images\019f0ff4-1c24-7a90-b917-2117c20eed47\ig_0c3422770c4b780d016a4246a4cb5081949468a93df4db7162.png`
- That generated image is **not yet packaged into the app**. To ship it as a
  real avatar, the next pass should slice/export state assets, add a skin id,
  wire `SkinIcon`, sidebar/settings/context-menu pickers, `/clippy` rendering,
  and update the avatar SDK docs if the manifest contract changes.

### Verification for nightly.9

- `npm run check`: passed with 0 errors. Existing unrelated warnings remain in
  History/onboarding and local `@types/node` setup.
- `npm run build`: passed. Existing unrelated bundle/a11y/unused-selector
  warnings remain.
- `cargo check` in `src-tauri`: passed. Existing unrelated Rust warnings remain.
- Local browser verification against `http://127.0.0.1:1420` confirmed:
  Security key event log starts collapsed, Cat lab is not visible on Avatar,
  bottom usage block is compact, and the LLM picker is fully above the anchored
  usage block at the 1280x720 viewport.

### Notes for the next assistant

- LLM token usage is recorded when providers return it; LLM dollar cost is not
  estimated yet because pricing is volatile. Add pricing as a separate
  provider-metadata layer if the user asks for spend tracking.
- Git status may show `commands.rs` / `lib.rs` as modified on Windows because
  cargo-formatting touched line endings, but `git diff --name-only` excludes
  them when there is no real content diff. Do not commit line-ending ghosts.

## Codex avatar pack checkpoint - v1.4.0-nightly.10

The next user prompt asked Codex to turn the generated fox concept into real
assets/avatars, add more states/animation, create cat avatars from photos of
Oru (orange tabby) and Gujia (white cat), and add a "Pikachu of sorts" while
using as much dynamic work as useful:

```text
Oh, I like this one. Can you actually, you know, sort of create the assets for it? And I like it. In fact, you can make it more high-fidelity, richer, you know, maybe a little more states, but in addition to these six. I'm not sure to what extent the SDK supports it. And a little bit of animation, right? How the entire avatar sort of shakes around or bounces around, or those little subtle touches which actually makes it beautiful. So, that's some actually good job that you've done. I think you have access to good image generation models, which I'm impressed at. So yeah, you can go ahead with that. Secondly, can you also create an avatar for the cat theme? And I'm pasting a photo of my cats to you. There's orange one is called Oru, and white one is called Gujia. And maybe a one-on-one or a combo of them, and then a whole lot of, you know, funny stuff that they can say. Again, implement dynamic agents and dynamic coping, and use as much resources that you want to run with. But yeah, make this happen, please. I would really love to have a few avatars. In fact, I would love to have a Pikachu of sorts, right? So please, two or three avatars, create assets you're getting access to, and I'm giving you go-ahead, right? If you can create these high-fidelity assets via them and create a few avatars, I think that would be super nice. If they can also do some fancy animations and all, even better, right? I'm just giving you a photo of my cats.
```

### What Codex changed for nightly.10

- Used the image generation skill to create three high-fidelity concept sheets:
  Codex Fox, Oru & Gujia, and Spark Buddy. The Spark Buddy is an original
  electric mascot, deliberately not a Pokemon/Pikachu clone.
- Copied those sheets into:
  - `static/avatar-concepts/codex-fox-deluxe-sheet.png`
  - `static/avatar-concepts/oru-gujia-duo-sheet.png`
  - `static/avatar-concepts/spark-buddy-sheet.png`
- Added `src/lib/RichAvatar.svelte` for the new code-native SVG avatars. This
  keeps the rich art/animation separate from the already-large `/clippy` page.
- Added selectable skins:
  - `codex-fox`
  - `oru-gujia`
  - `spark-buddy`
- Wired those skins through `skin-store`, `SkinIcon`, the sidebar Avatar picker,
  Settings -> Avatar, the floater context menu, and `/clippy` sizing/rendering.
- Added state animation for the new skins: idle breathing, listening bounce and
  waves, thinking effects, writing/tapping, paste celebration, hover reactions,
  and phew transition support.
- Added skin-specific hover quips and bubble color themes.
- Kept `cat-lab` retired from active UI. Existing `cat-lab` saved values still
  migrate to `cat`; the large old renderer branch remains unreachable legacy
  code and can be pruned in a later cleanup-only pass.
- Updated `docs/AVATAR_SDK.md` so it points future agents to both
  `src/routes/clippy/+page.svelte` and `src/lib/RichAvatar.svelte`.

### Verification for nightly.10

- `npm run check`: passed with 0 errors. Existing unrelated warnings remain in
  History/onboarding and local `@types/node` setup.
- `npm run build`: passed. Existing unrelated bundle/a11y/unused-selector
  warnings remain.
- Browser verification against `http://127.0.0.1:1420` confirmed Settings ->
  Avatar shows all 9 tiles without horizontal overflow; the expanded sidebar
  avatar picker shows all 9 icon buttons above the anchored Usage block at
  1280x720; and `/clippy` renders `codex-fox`, `oru-gujia`, and `spark-buddy`
  as nonblank SVG avatars with the expected bubble skin IDs.

## Codex avatar renderer correction - v1.4.0-nightly.11

After testing nightly.10, the user correctly rejected the implementation: the
generated concept sheets were high-fidelity raster art, but Codex had shipped
small hand-coded SVG approximations through `RichAvatar.svelte`. The live
avatar quality was therefore far below the approved concepts.

User correction prompt:

```text
Wait a second, am I getting any of this wrong or the kind of assets that you created, or at least showed for me in the illustrations versus what you have actually implemented is miles apart. Like it's so different. And I can't even, like what is happening? Why the shitty implementation when the concepts were so nice? Why you're not able to generate them? Like what's happening? I want you to actually implement identically the same. If you think the SDK is an issue, I would be more than happy to spin up a panel SDK or, you know, evolve the SDK or increase its scope and the kind of assets that it can, you know, digest. But this, whatever you have created is so bad. Like just compare against the three options, the three images that you sent me in the last one. This, like where the gap is. Like before you go ahead and commit anything, tell me where the gap exists. Why were you not able to realize that same vision?

Yeah, I think you have, these two better, and you can go ahead and see. Yes, absolutely, the assets that you created, it even more high fidelity, if we can generate 3D assets even better. Not shader, I don't know shader and all. You know, 3D assets which look even much more interactive, nice, real, of sorts. I am giving you again, full, how do you say, full freedom to spend up as many agents, whatever cost. Just go ahead and implement it. Right, but I need that richer thing, also backwards compatible, right? So, my existing avatars, especially with the classic one, PPTA one, etc., etc., whatever have been created for me, those are the ones which should not break. Right, so if you want to have some compatibility mode within this revised SDK, I mean, feel free to change it bottoms up, right? Not an issue. But this is a core part of my application, and then I want it to be of top-notch quality. 2D supported, raster supported or whatever. You can start off with raster, and you can also create some 3D assets and then, you know, go for it. All of that is feasible, should be done. And I'm giving you unlimited time and resources to do it.
```

### What Codex changed for nightly.11

- Added a manifest-v2 raster state-pack layer for built-in high-fidelity
  avatars:
  - `src/lib/avatar-packs.ts`
  - `src/lib/RasterAvatar.svelte`
- Added a reproducible extraction script:
  - `scripts/extract-avatar-sheets.ps1`
- Sliced the approved concept sheets into transparent per-state PNG assets:
  - `static/avatars/codex-fox/{idle,listening,thinking,writing,pasting,error,sleeping,excited,thumbnail}.png`
  - `static/avatars/oru-gujia/{idle,listening,thinking,writing,pasting,error,sleeping,excited,thumbnail}.png`
  - `static/avatars/spark-buddy/{idle,listening,thinking,writing,pasting,error,sleeping,excited,thumbnail}.png`
- Added `avatar.json` manifests for all three raster packs.
- Updated `SkinIcon` so the picker thumbnails show the actual raster art.
- Updated `/clippy` so `codex-fox`, `oru-gujia`, and `spark-buddy` render via
  `RasterAvatar`, not `RichAvatar`.
- Preserved compatibility for the legacy avatars:
  - `fox` still uses the original watercolor PNG layer stack.
  - `stylized`, `cat`, `duo`, and retired migration-only branches remain SVG.
  - `real-clippy` still uses the vendored Clippy sprite runtime.
  - Existing saved skin migrations remain unchanged.
- Updated the avatar SDK docs to state that raster state packs are now
  implemented for built-in avatars; multi-frame timelines and real-time 3D
  remain future work.

### Why this fixes the nightly.10 gap

The rejected nightly.10 path used the generated images only as visual
references and rebuilt them manually in SVG. Nightly.11 uses the generated
images themselves as the live app artwork, with CSS transforms only for subtle
motion and state transitions. This keeps the visual fidelity aligned with the
approved concepts while staying backward-compatible with all existing skins.

## Codex raster avatar QA correction - v1.4.0-nightly.12

After nightly.11, the user reported that the new raster avatars still failed in
the live floater: they did not follow the S/M/L scale setting, rotated/rolled
into the right edge, showed duplicated cut fragments at the right border, and
the Oru & Gujia cutout looked visibly rough.

User correction prompt:

```text
All right, I'll test the Android part later, but the part that you have created, which is the, okay, first of all, consolidate. Secondly, the part that you have created is becoming problematic because the, say the raster-based graphic that you created does not follow scaling. I think it also does like cut and rotate, so... So, let's say that as of, so all our existing ones are looking good and nice. The new ones that you've created, they are basically rolling over to the right side. So if the image sort of moves to the left to a certain extent, then the cut part shows up towards the right border. And it does not scale. The quality also looks dicey. The two cat thing, I'm not sure if you have done some sort of edge reduction and removal of background thingy. But that also looks really bad. It has cut really, really badly. So, a lot of problems with that whole thing. And I'm not sure like how you are doing this, but we need better quality control. I'll give you a few examples. I'm quick pasting screenshots.
```

### Root causes Codex found

- `RasterAvatar.svelte` ignored the floater `--fscale` variable, so the window
  scale changed but the raster frame stayed at the original dimensions.
- `/clippy/+page.svelte` still had broad legacy SVG animation selectors that
  matched `.character.raster-avatar`. Raster avatars were therefore getting both
  their own raster animation and the old whole-character rotate/jitter/bounce
  transforms.
- Several generated PNG state files contained small secondary alpha islands
  touching the image edge. These were invisible in concept-sheet review but
  became visible as duplicated slivers when the live floater animated.
- The first Oru & Gujia source used white cats on a white/off-white sheet.
  Background removal therefore erased some of Gujia's white fur; those missing
  pixels looked acceptable on white previews but turned into dark holes on the
  live dark floater.
- The raster pack head metadata was too low for the generated sheets, so the
  status bubble/provider label sat over faces instead of above the head.

### What Codex changed for nightly.12

- Updated `src/lib/RasterAvatar.svelte` so raster avatars scale all dimensions
  through `--fscale`, including the outer box, internal frame, and signal waves.
- Added an internal safe frame and `overflow: hidden` boundary for raster packs
  so hover/bounce motion and soft shadows do not crop against the window border.
- Scoped legacy `/clippy` state animations away from `.raster-avatar`, leaving
  the existing SVG avatars unchanged.
- Raised the `head` anchor for `codex-fox`, `oru-gujia`, and `spark-buddy` in
  both `src/lib/avatar-packs.ts` and each `avatar.json` manifest.
- Cleaned non-primary edge-touching PNG fragments from affected raster states:
  - `static/avatars/codex-fox/error.png`
  - `static/avatars/codex-fox/thinking.png`
  - `static/avatars/spark-buddy/error.png`
- Regenerated the entire Oru & Gujia state pack from a Codex chroma-key sheet:
  - `static/avatar-concepts/oru-gujia-duo-sheet-chromakey.png`
  - `static/avatars/oru-gujia/{idle,listening,thinking,writing,pasting,error,sleeping,excited,thumbnail}.png`
- Updated `scripts/extract-avatar-sheets.ps1` so future extractions use the
  chroma-key Oru & Gujia source instead of the old white-background sheet.
- Updated the Oru & Gujia art frame to `222x214` with head anchor `214`, so the
  taller clean sprites scale without being squeezed into the old shorter frame.

### Verification for nightly.12

- `npm run check`: passed with 0 errors. Existing unrelated warnings remain.
- `npm run build`: passed. Existing unrelated bundle/a11y warnings remain.
- A local contact sheet checked Codex Fox, Oru & Gujia, and Spark Buddy at 80%,
  100%, and 125% scale.
- A PNG alpha-component scan found no remaining secondary edge-touching
  fragments after cleanup.
- A dark-background preview checked all eight regenerated Oru & Gujia states.

## Stable promotion and session-reset checkpoint - v1.4.0

On 2026-06-30, the user tested `v1.4.0-nightly.12` and approved it for stable:

```text
works great
lets make this the latest stable release

also - we will handover and reset this session post this commit and changes - so make sure you get the handover docs updated
```

### What this promotion means

- `v1.4.0` is the stable promotion of the Codex `v1.4.0-nightly.12` line.
- App code is the same tested code path as nightly.12; the stable-promotion
  commit updates release notes, public download links, project memory, and this
  handover checkpoint.
- The release title/notes still make Codex provenance visible, per the user's
  standing request.
- This is not a new feature branch. Future work should start after the stable
  tag unless the user explicitly asks to go back.

### Stable-promotion docs updated

- `docs/RELEASE_NOTES_v1.4.0.md` added for the stable release workflow.
- `README.md`, `docs/site-data.js`, and `docs/index.html` updated so public
  download links point to `v1.4.0` artifacts.
- `CLAUDE.md` updated so future Claude/Codex sessions see `v1.4.0` as the live
  stable baseline.
- `docs/ROADMAP.md` updated with a `v1.4.0 stable` done entry.
- `docs/AVATAR_SDK.md` updated so the raster-avatar implementation note refers
  to `v1.4.0`, not the earlier nightly.

### Reset guidance for the next session

Start by checking:

```powershell
git status --short --branch
git log -5 --oneline
gh release view v1.4.0 --repo kumaradarsh1993/wispr-fox
```

Expected state after this handover completes:

- Branch `main` clean and pushed.
- Stable tag `v1.4.0` pushed and published as Latest.
- `v1.4.0-nightly.12` remains as the pre-stable test checkpoint.
- The next desktop task should branch conceptually from `v1.4.0` stable, not
  from any earlier Claude checkpoint.
