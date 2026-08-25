# v3.4.0-nightly.1

**Your devices, as one account.** Insights stops answering "how much have you
dictated *on this computer*" and starts answering "how much have you dictated",
and every transcript now tells you which machine it came from.

## One set of numbers, not one per computer

If you run wispr-fox on more than one machine you have had two different
answers to the same question — one desktop counting from 11 July, another from
24 July, each with its own time-saved total, and neither of them the truth.

Insights now merges every signed-in device:

- **"Since" is the date your *account* started**, not the date this install did.
- Words, sessions, time saved and the activity chart are the totals across
  every device.
- A new **All devices / This device** switch is there when you want the old
  per-machine view back. It only appears once a second device has actually
  reported — a toggle whose two sides show the same number is just clutter.
- A **per-device breakdown** shows the split, so merging never hides where the
  work happened.

Two honest caveats the UI states plainly rather than papering over:

- A device that is signed in but has not synced since this build is listed as
  **not reporting yet** instead of being silently counted as zero.
- **Voice signature stays a this-device measurement.** It is derived on-device
  from retained raw transcripts, which never leave the machine that recorded
  them, so it is now explicitly labelled rather than sitting under an
  "all devices" heading it cannot support.

## Which device was this?

- Every history card carries a small **device icon before the timestamp**, so
  you can see at a glance that a note came from the laptop and not the desktop.
- The chip stays hidden on a single-device account, where the answer is always
  "this one" and the icon would be noise.

## My devices

Settings → Account now lists every device on your account — the registry has
existed since v3.0.0 but was never shown.

- **Assign an icon** from a small set organised by *where a machine lives*:
  Home, Office, Work, Desk, Laptop, Mobile, Tablet, Travel, Studio, Testing.
- **Rename any device** to something you would actually recognise. The name you
  set beats the hostname everywhere.
- You can label any device **from any device** — you are as likely to name the
  laptop while sitting at the desktop.
- Each row shows its platform and when it was last seen.

## What is and isn't sent

This adds **counts and dates** to what sync already carried, and nothing else.
Audio still never leaves the machine that recorded it. It needed no changes to
your database — device names, icons and rollups ride on tables that have been
there since v3.0.0.

## Also

- Signing out now clears the cached device list, so signing into a *different*
  account no longer briefly shows the previous account's devices.
- Recordings are stamped with the device that made them. Older recordings have
  no stamp and fall back to the recorded device name, or to "this device" for
  anything local.

## Validation

- Svelte diagnostics: 327 files, 0 errors, 0 warnings.
- Rust: `cargo check` clean (only the pre-existing dead-code warnings).
- The merge, the scope switch, the per-device breakdown, the history chips, and
  the device list + icon picker were each driven and measured in a live browser
  against a three-device account.
- Fleet rollup logic: 4/4 unit tests. They run in CI — the app crate's test
  binary cannot launch on the dev machine (it links the WebView2 stack), so
  they were additionally verified in an isolated harness built from the
  shipped source.

This is a nightly. The interesting thing to try is signing a second machine in
and watching the two sets of numbers become one.
