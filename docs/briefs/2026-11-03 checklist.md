# Checklist for the 03-Nov-2026 brief

Status: [ ] open, [x] done, [?] unclear (ask Adarsh).

## A. Releases
- [x] A1 Latest desktop nightly on this Mac: v3.6.0-nightly.2 is installed and is the newest release.
- [ ] A2 Promote desktop to stable v3.6.0 (Conversations + Keychain fix).
- [ ] A3 Promote Android to stable v2.3.0 (Conversations), before the revamp starts.
- [ ] A4 Revamp work goes into the next nightlies (desktop v3.7.0-nightly.x, Android v2.4.0-nightly.x).

## B. Sync (desktop + web, and Android History)
- [ ] B1 Find why the phone's second upload of the same audio (2 speakers, speakers renamed) never reached desktop.
- [ ] B2 Refresh button runs a full consistency check: fetch rows on the server missing here, push local rows the server lacks, re-push local edits, then report what it fixed.
- [ ] B3 Same refresh on web.

## C. Android revamp
- [ ] C1 Floating fox / auto-pop on text fields OFF by default, with a clear on/off toggle next to Tap to speak. When off, nothing should draw over other apps, so banking apps (ICICI iMobile) stop warning. Check why the fox appears on some fields and not others.
- [ ] C2 Bottom bar: Speak, Upload, History, Settings (Usage inside Settings).
- [ ] C3 Speak tab: one fixed screen, no scrolling. Model picker more visible as quick settings. Remove the Recent list.
- [ ] C4 Share from Voice Recorder opens a small sheet with 2 choices: Transcript, Transcript with speakers.
- [ ] C5 Upload tab: one screen, no scrolling, little text. Choose-file icon top right. The 3 newest recordings from the recordings folder shown to pick (multi-select). Folder set in Settings.
- [ ] C6 Presets: Transcript (Whisper, no cleanup) / Meeting with speakers (Deepgram Nova-3 + Whisper, both kept) / Custom (Whisper + cleanup by default, full pickers).
- [ ] C7 Custom block: provider dropdown + model dropdown + Label speakers checkbox. Ticking Label speakers switches to Deepgram Nova-3 and says so. Outputs: Clean transcript, Draft, Meeting notes, with a model picker.
- [ ] C8 Meeting copy: Deepgram speaker transcript first with a line saying what it is, a separator, then the Whisper transcript with a line saying it is the same audio. Speaker names change only the first part. Checkbox for including Whisper, on by default.
- [ ] C9 Copied audio files are cleaned up. Settings shows storage used by copies, with a delete button.
- [ ] C10 History: Copy, Read, Rerun as small icons on the same row as Raw/Clean/Draft (no duplicate copy). Refresh button at top. Rerun greys out Transcribe and Label speakers when the audio isn't on this phone, and says why. Find why they look grey for phone rows. Fix reading/scrolling to the bottom of long text.
- [ ] C11 Leaner UI: text 1-2 points smaller, smaller icons and padding, sized for the S23 Ultra.
- [ ] C12 Check for new or retired models at Groq, OpenAI, Gemini, Deepgram, ElevenLabs.

## D. Docs
- [ ] D1 Update CLAUDE.md, HANDOVER.md for desktop, Android and web. THETA_MEMORY.md doesn't exist in these repos yet: create one in each (append-only log), per Adarsh's 3-file norm.
