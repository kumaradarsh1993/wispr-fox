# wispr-fox 3.7.0

**Your notes stay in sync across devices, and long recordings transcribe in full.**

## What's new

- **Refresh checks sync.** The refresh button on History and Conversations compares this computer with your account, then fixes what's missing. It pulls in notes from your other devices and sends up any of this computer's notes that never arrived. A one-line summary says what changed.
- **Transcripts no longer vanish.** The 500 MB audio limit used to delete whole notes. It now clears only the oldest audio, and the text stays. Your first Refresh brings back transcripts lost this way.
- **Long recordings get their Whisper transcript.** A 95-minute phone recording used to fail the Whisper step for being too big. It's now sent in parts.
- **Mac: Draft is Shift+Option+Space.** Option+Enter is free again for a new line in an Excel cell. Force clean moves to Ctrl+Shift+Option+Space.

## Fixed

- Notes from a phone whose clock runs a little behind no longer get skipped.
