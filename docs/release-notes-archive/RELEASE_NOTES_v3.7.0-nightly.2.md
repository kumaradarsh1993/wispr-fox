# wispr-fox v3.7.0-nightly.2

**Long conversations get their Whisper transcript.**

A 95-minute phone recording (a 65 MB .m4a) failed its Whisper step with "too large for Groq Whisper in one request". Groq takes at most 25 MB per request, and the app could only split WAV files. Long compressed files are now converted on the Mac to a small WAV (16 kHz mono, about 2 seconds for 95 minutes of audio), which is then sent in parts of about 20 MB, one after another. The time limit grows with the number of parts.

To fix a conversation that already failed, open it and press **Add Whisper transcript**.

Also in this nightly: on Mac, Drafting moves from Option+Enter to Shift+Option+Space, so Excel gets its in-cell new line back. Force clean moves to Ctrl+Shift+Option+Space.
