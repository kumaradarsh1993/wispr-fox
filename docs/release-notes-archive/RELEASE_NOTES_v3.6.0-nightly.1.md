# wispr-fox v3.6.0-nightly.1

**Conversations get their own page.** Upload a meeting or call and get two
transcripts, who said what, meeting notes, and a file you can hand to an AI
assistant.

## Voice note or Conversation

Every upload now asks one question first: is this **just you talking**, or a
**meeting or call**? The app remembers your answer. Dropping a file on the
Conversations page picks Conversation for you.

- **Voice note** works as before: Whisper, optional cleanup, saved to Home.
- **Conversation** runs three steps, each on its own:
  1. **Speakers.** Deepgram Nova-3 labels who said what, on the whole
     recording in one go, so "Speaker 2" is the same person from start to
     finish. ElevenLabs Scribe and OpenAI's diarize model are options.
  2. **Whisper transcript** (on when you have a Groq key). Whisper Large v3
     on the same audio. Its wording is usually more accurate, so it is kept
     next to the speaker version rather than replacing it.
  3. **Meeting notes** (on when you have a cleanup key). Summary, decisions,
     action items and open questions, written from the speaker transcript
     with the Whisper version as a reference.

## The Conversations page

It sits between Home and Insights. Conversations no longer appear on Home,
so Home stays a list of dictations.

- Each card shows the title, length, the speakers, and the start of the notes.
- Opening one shows **Notes**, **Speakers** (with timestamps) and **Whisper**.
- **Name your speakers** in one panel that shows the first thing each person
  said, so you can tell who is who. Names apply to the transcript, the notes,
  copied text and the export, and they sync to your other devices.

## When a step fails

A failed step never throws away a step that worked. The card says which step
failed, and each one has its own button: **Retry speakers**, **Add Whisper
transcript**, **Write notes**.

## Copy and export

- **Copy** Notes, the speaker transcript, the Whisper transcript, or
  everything.
- **Export** saves one Markdown file with the notes and both transcripts. Its
  first lines tell an AI assistant what each transcript is good for: the
  speaker version for who said what, Whisper for the exact wording. Paste it
  into Claude or Kiro and ask your questions.

## Long uploads

Deepgram and ElevenLabs uploads used to time out after 90 seconds, which a
long meeting on a slow connection could exceed. The limit now grows with the
file size, up to 15 minutes.
