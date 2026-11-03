# wispr-fox v3.6.0

Stable roll-up of v3.6.0-nightly.1 and nightly.2.

## Conversations get their own page

- Every upload asks first: **Voice note** or **Conversation**. The app remembers the answer.
- A **Conversations** page between Home and Insights. Home now lists only dictations.
- Each conversation keeps **two transcripts**: Deepgram Nova-3 labels who said what across the whole recording, and Whisper Large v3 gives the more accurate wording.
- **Meeting notes** are written from both transcripts.
- **Each step retries on its own**: Retry speakers, Add Whisper transcript, Write notes. A failed step never throws away one that worked.
- **Name speakers** in one panel that shows each person's first words. Names apply to the transcript, notes, copied text and export, and sync to other devices.
- **Copy** Notes, Speakers, Whisper or Everything, or **Export** one Markdown file with both transcripts for an AI assistant.
- Long uploads get a time limit that grows with the file size.

## macOS: no more repeated Keychain password prompts

All API keys and the sign-in token now sit in one Keychain item owned by the running build. A build reads its own item without asking. After an update you get at most one prompt, on the first launch, while the keys move to the new build's item.
