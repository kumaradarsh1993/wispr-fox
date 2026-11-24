# wispr-fox v3.7.0-nightly.1

**Refresh now checks sync properly, and the storage cap stops deleting transcripts.**

## Refresh is a sync check

The refresh button on History and Conversations used to reload the list from this computer only. It now:

1. Syncs as usual.
2. Compares every note on your account with what this computer has.
3. Adds notes from your other devices that are missing or out of date here, and removes notes deleted on the device that made them.
4. Re-sends this computer's notes that the server doesn't have.
5. Restores this computer's own older transcripts that the server still holds, without their audio.
6. Shows one line saying what it fixed, for example "Synced: 3 new from other devices, 2 re-sent from here."

## A phone note could be skipped by the pull

Every device stamps a note with its own clock when it sends it. If the phone's clock ran behind, a note could arrive stamped earlier than notes this computer had already pulled, and the pull skipped it. Each pull now re-reads the last 15 minutes and skips notes it already has.

## The storage cap deleted transcripts

The 500 MB cap on stored audio deleted the whole note along with its audio, so one long recording day could wipe every older transcript on this computer. It now deletes only the audio, and the transcript stays. Refresh brings back the transcripts that were lost this way, as long as the server still has them.
