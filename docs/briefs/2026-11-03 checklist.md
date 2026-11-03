# Checklist for the 03-Nov-2026 brief

Status: [ ] open, [x] done, [?] unclear (ask Adarsh).

## A. Releases
- [x] A1 Latest desktop nightly on this Mac: v3.6.0-nightly.2 is installed and is the newest release.
- [x] A2 Promote desktop to stable v3.6.0 (Conversations + Keychain fix).
- [x] A3 Promote Android to stable v2.3.0 (Conversations), before the revamp starts.
- [x] A4 Revamp work goes into the next nightlies (desktop v3.7.0-nightly.x, Android v2.4.0-nightly.x).

## B. Sync (desktop + web, and Android History)
- [x] B1 Find why the phone's second upload of the same audio (2 speakers, speakers renamed) never reached desktop.
- [x] B2 Refresh button runs a full consistency check: fetch rows on the server missing here, push local rows the server lacks, re-push local edits, then report what it fixed.
- [x] B3 Same refresh on web.

## C. Android revamp
- [x] C1 Floating fox / auto-pop on text fields OFF by default, with a clear on/off toggle next to Tap to speak. When off, nothing should draw over other apps, so banking apps (ICICI iMobile) stop warning. Check why the fox appears on some fields and not others.
- [x] C2 Bottom bar: Speak, Upload, History, Settings (Usage inside Settings).
- [x] C3 Speak tab: one fixed screen, no scrolling. Model picker more visible as quick settings. Remove the Recent list.
- [x] C4 Share from Voice Recorder opens a small sheet with 2 choices: Transcript, Transcript with speakers.
- [x] C5 Upload tab: one screen, no scrolling, little text. Choose-file icon top right. The 3 newest recordings from the recordings folder shown to pick (multi-select). Folder set in Settings.
- [x] C6 Presets: Transcript (Whisper, no cleanup) / Meeting with speakers (Deepgram Nova-3 + Whisper, both kept) / Custom (Whisper + cleanup by default, full pickers).
- [x] C7 Custom block: provider dropdown + model dropdown + Label speakers checkbox. Ticking Label speakers switches to Deepgram Nova-3 and says so. Outputs: Clean transcript, Draft, Meeting notes, with a model picker.
- [x] C8 Meeting copy: Deepgram speaker transcript first with a line saying what it is, a separator, then the Whisper transcript with a line saying it is the same audio. Speaker names change only the first part. Checkbox for including Whisper, on by default.
- [x] C9 Copied audio files are cleaned up. Settings shows storage used by copies, with a delete button.
- [x] C10 History: Copy, Read, Rerun as small icons on the same row as Raw/Clean/Draft (no duplicate copy). Refresh button at top. Rerun greys out Transcribe and Label speakers when the audio isn't on this phone, and says why. Find why they look grey for phone rows. Fix reading/scrolling to the bottom of long text.
- [x] C11 Leaner UI: text 1-2 points smaller, smaller icons and padding, sized for the S23 Ultra.
- [x] C12 Check for new or retired models at Groq, OpenAI, Gemini, Deepgram, ElevenLabs.

## D. Docs
- [x] D1 Update CLAUDE.md, HANDOVER.md for desktop, Android and web. THETA_MEMORY.md doesn't exist in these repos yet: create one in each (append-only log), per Adarsh's 3-file norm.

## Progress log
- 19:35 IST: v3.6.0 (desktop) and v2.3.0 (Android) tagged. Android v2.3.0 published as Latest.
- 19:50 IST: B1 causes found. (1) Android applied its own pushed rows back as pulled copies, so a phone row stopped pushing after its first sync; speaker-name edits never left the phone. (2) A conversation waiting on a retried step was never pushed. (3) Pull read strictly after the cursor, so a row stamped by a slower clock could be skipped. (4) Desktop's storage-cap sweep deleted whole rows, which is why this Mac only holds today's 18 rows.
- 19:52 IST: B2 desktop on branch `sync-check` (Refresh on History and Conversations). Android sync fixes on branch `revamp`; Refresh button lands with the History redesign (C10).
- S10 models check (03-Nov): Groq's live model page lists Whisper Large v3, Whisper Large v3 Turbo, GPT-OSS 20B/120B and Qwen 3.8 27B. All 5 are already in the apps, and Llama 3.3 70B is still Enterprise-only. Deepgram's current model is still Nova-3. ElevenLabs' current speech-to-text model is still Scribe v2 (v1 is deprecated, and the apps only migrate it). The Gemini and OpenAI model pages render client-side and couldn't be fetched, and web search was blocked. The Android and desktop lists are identical and were last refreshed 30-Oct. No changes made.
- 20:55 IST: v3.7.0-nightly.1 (desktop) and v2.4.0-nightly.1 (Android) published as pre-releases; v3.6.0 and v2.3.0 stay Latest. Desktop nightly installed on this Mac after a 2-minute recording-idle check (backup of 3.6.0-nightly.2 in session scratch). Docs (D1) done in all three repos.
