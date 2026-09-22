<script lang="ts">
  import { tick } from "svelte";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { history } from "./history-store.svelte";
  import { api, type Recording } from "./api";
  import { settings } from "./settings-store.svelte";
  import { shortModel } from "./provider-options";
  import DeleteDialog from "./DeleteDialog.svelte";
  import RerunDialog from "./RerunDialog.svelte";
  import SpeakerLabelsDialog from "./SpeakerLabelsDialog.svelte";
  import ReadingMode from "./ReadingMode.svelte";
  import { applySpeakerNames, namedSpeaker, speakerNames, speakerTurns } from "./meeting-text";
  import { fleet } from "./fleet-store.svelte";
  import { deviceGlyph, deviceDisplayName } from "./device-icons";
  import { askConfirm, showMessage } from "./dialogs";

  let { rec } = $props<{ rec: Recording }>();

  type Tab = "raw" | "cleaned" | "drafted" | "meeting_notes";

  // Active tab. Defaults to the "most refined" version that exists:
  // drafted > cleaned > raw. User can switch by clicking the tabs. The
  // default is DERIVED from `rec` so it updates when a cleaned/drafted
  // version is generated in place; `tabOverride` records an explicit user
  // pick and wins once set. (Deriving avoids the state_referenced_locally
  // warning from capturing `rec` at init.)
  function defaultTab(r: Recording): Tab {
    if (r.meeting_notes_text) return "meeting_notes";
    if (r.drafted_text) return "drafted";
    if (r.cleaned_text) return "cleaned";
    return "raw";
  }
  let tabOverride = $state<Tab | null>(null);
  let activeTab = $derived<Tab>(tabOverride ?? defaultTab(rec));

  let expanded = $state(false);
  let audioUrl = $state<string | null>(null);
  let audioEl = $state<HTMLAudioElement | null>(null);
  let playing = $state(false);
  let busy = $state(false);
  let rerunOpen = $state(false);
  let speakersOpen = $state(false);
  let readerOpen = $state(false);

  // When the generate-on-demand command for THIS row is in flight, track
  // which kind so the corresponding tab can show a spinner instead of
  // letting the user fire a second request.
  let generating = $state<null | "cleaned" | "drafted" | "meeting_notes">(null);

  let displayedText = $derived.by(() => {
    if (activeTab === "meeting_notes") return rec.meeting_notes_text || "(no meeting notes yet)";
    if (activeTab === "drafted") return rec.drafted_text || "(no draft yet)";
    if (activeTab === "cleaned") return rec.cleaned_text || "(no cleaned version yet)";
    return rec.transcript || "(no transcript)";
  });
  let names = $derived(speakerNames(rec));
  let turns = $derived(activeTab === "raw" ? speakerTurns(rec) : []);
  let copyableText = $derived(applySpeakerNames(displayedText, names));
  let readableBlocks = $derived(copyableText.split(/\n\s*\n/).map((part) => part.trim()).filter(Boolean));
  let isError = $derived(rec.status === "error");
  // An AI version that fell back to the raw transcript. clippy.rs returns the
  // INPUT unchanged whenever the model times out or errors, recording why in
  // clippy_note — so without this the tab shows the raw transcript and looks
  // like the feature simply does nothing. That is exactly how "meeting notes
  // gives back raw text" presents. The note was only visible in the expanded
  // row and the (i) inspector, i.e. nowhere near the text it explains.
  let fellBackToRaw = $derived(
    activeTab !== "raw" && rec.clippy_used === false && !!rec.clippy_note,
  );
  let fallbackReason = $derived.by(() => {
    switch (rec.clippy_note) {
      case "clippy_timeout":
        return "the model took too long to answer";
      case "clippy_rate_limited":
        return "the provider rate-limited the request";
      case "clippy_auth":
        return "the provider rejected the API key";
      case "clippy_upstream":
        return "the provider had a server error";
      case "light_length_drift":
        return "the result differed too much from what you said, so it was discarded";
      default:
        return "the request to the model failed";
    }
  });
  // Retry must be available at ALL statuses — including 'transcribing'
  // and 'cleaning'. The whole reason this exists is to recover from
  // recordings that got stranded mid-flow (the flow crashed before
  // status flipped to 'error' or 'done'). Disabling on status would
  // lock the user out of the exact case they need to fix.
  //
  // Rust retry_recording is idempotent: bumps retry_count, resets
  // status, re-runs. If a real job somehow IS still running, the
  // second pass wins on UPSERT — annoying but not catastrophic.
  // Only block while THIS row's button has a request in flight.
  let retryDisabled = $derived(busy);

  // Inspector — (i) button. Shows: full error, retry count, providers
  // used, Clippy note, audio path. Surfaced for every recording so the
  // user can see what happened on success too. Red dot when there's an
  // error worth noticing.
  let showInspector = $state(false);
  let inspectorHasNews = $derived(isError || !!rec.error);

  // Per-recording flight recorder (nightly.7). The Rust pipeline records how
  // long STT / cleanup took and a timestamped event log, so a slow or failed
  // run is diagnosable right here instead of being a mystery spinner.
  type TlEvent = { ms: number; msg: string };
  let timeline = $derived.by<TlEvent[]>(() => {
    if (!rec.event_log) return [];
    try {
      const arr = JSON.parse(rec.event_log);
      return Array.isArray(arr) ? arr : [];
    } catch {
      return [];
    }
  });
  let hasTiming = $derived(
    rec.stt_ms != null || rec.cleanup_ms != null || rec.total_ms != null || timeline.length > 0,
  );
  // Draw the eye to a slow transcription — anything past ~8s is worth noticing
  // for a normal dictation. Deepgram/Groq usually answer in 1-3s.
  let sttSlow = $derived(rec.stt_ms != null && rec.stt_ms > 8000);

  // Capture gap: the mic dropped mid-recording, so less audio reached the WAV
  // than the timer ran — the transcript is truncated because the audio is. We
  // only claim this when we actually measured it (nightly.8+ rows), allowing
  // 1s of slack for normal tail-drain rounding.
  let captureGap = $derived(
    rec.audio_captured_ms != null &&
      rec.duration_ms > 0 &&
      rec.audio_captured_ms + 1000 < rec.duration_ms,
  );
  let captureLostS = $derived(
    captureGap ? Math.max(0, Math.round((rec.duration_ms - (rec.audio_captured_ms ?? 0)) / 1000)) : 0,
  );
  let capturedS = $derived(Math.max(0, Math.round((rec.audio_captured_ms ?? 0) / 1000)));

  function fmtMs(ms: number | null | undefined): string {
    if (ms == null) return "—";
    if (ms < 1000) return `${ms}ms`;
    return `${(ms / 1000).toFixed(1)}s`;
  }

  // Kebab (3-dot) menu — holds Retry + Delete per the v0.4.0 design
  // playbook. Closes on outside click or Escape.
  let kebabOpen = $state(false);
  $effect(() => {
    if (!kebabOpen) return;
    const onDoc = (e: MouseEvent) => {
      const target = e.target as HTMLElement | null;
      if (target && target.closest(".kebab-wrap")) return;
      kebabOpen = false;
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") kebabOpen = false;
    };
    window.addEventListener("click", onDoc);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("click", onDoc);
      window.removeEventListener("keydown", onKey);
    };
  });

  function timeShort(iso: string): string {
    try {
      const d = new Date(iso);
      const now = new Date();
      const diff = now.getTime() - d.getTime();
      const sameDay = d.toDateString() === now.toDateString();
      if (diff < 60_000) return "just now";
      if (diff < 3600_000) return `${Math.floor(diff / 60_000)}m ago`;
      if (sameDay) return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
      const yesterday = new Date(now);
      yesterday.setDate(yesterday.getDate() - 1);
      const isYesterday = d.toDateString() === yesterday.toDateString();
      if (isYesterday)
        return `Yesterday ${d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}`;
      return d.toLocaleString([], {
        month: "short",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      });
    } catch {
      return iso;
    }
  }

  // Map the internal mode value (light / drafting / advanced) to the
  // canonical user-facing name. DB/enum values stay as-is; this is display.
  function modeLabel(mode: string): string {
    if (mode === "light") return "Transcribe";
    if (mode === "drafting") return "Draft";
    if (mode === "advanced") return "Cleanup";
    return mode;
  }

  function durationShort(ms: number): string {
    if (!ms) return "—";
    const s = Math.round(ms / 1000);
    if (s < 60) return `${s}s`;
    const m = Math.floor(s / 60);
    return `${m}m ${s % 60}s`;
  }

  // Platform badge — Desktop / Web / Mobile — shown next to the "Uploaded"
  // badge. Rows synced from another device carry remote=1 and have no local
  // audio, so playback is hidden for them.
  function platformLabel(p: string): string {
    if (p === "web") return "Web";
    if (p === "mobile") return "Mobile";
    return "Desktop";
  }
  let platformTitle = $derived(
    rec.device_name ? `${platformLabel(rec.platform)} · ${rec.device_name}` : platformLabel(rec.platform),
  );
  // Only surface a platform badge for rows that came from elsewhere — a plain
  // local Desktop row doesn't need a "Desktop" tag cluttering every card. Once
  // any sync has happened (remote rows exist, or a row is explicitly web/
  // mobile), the badge earns its place.
  let showPlatformBadge = $derived(rec.remote || rec.platform === "web" || rec.platform === "mobile");

  // Which device made this recording.
  // `device_id` is the reliable join, but it only exists on rows created
  // after that column shipped. Two fallbacks, in order of trustworthiness:
  // a LOCAL row is by definition this device (that is what `remote` means),
  // and failing that we match on the recorded device name. Name matching is
  // last because two machines can share a name — it is a display hint, not
  // an identity.
  let sourceDevice = $derived.by(() => {
    const byId = fleet.byId(rec.device_id);
    if (byId) return byId;
    if (!rec.remote) return fleet.thisDevice;
    if (rec.device_name) {
      return fleet.devices.find((d) => d.name === rec.device_name) ?? null;
    }
    return null;
  });

  // On a one-device account this chip is pure noise — the answer is always
  // "this computer". It earns its place only once there is a second device.
  let showDeviceChip = $derived(fleet.isMultiDevice);

  let deviceChipGlyph = $derived(
    deviceGlyph(sourceDevice?.icon, sourceDevice?.platform ?? rec.platform),
  );
  let deviceChipName = $derived(
    sourceDevice
      ? deviceDisplayName(sourceDevice)
      : (rec.device_name ?? "Unknown device"),
  );

  // ONE collapsed chip, and only for a row that is not an ordinary local
  // dictation. The old header carried an Uploaded badge, a platform badge, a
  // retry counter, a device glyph and a "Failed — see details" pill, all
  // competing on every single card; a marker that appears on every row tells
  // the eye nothing. Ordinary dictations now show no chip at all, so a chip
  // means "this one is different" and reads at a glance. Everything that used
  // to be a chip is still on the row — it moved into Details, where it is
  // labelled rather than abbreviated.
  let kindChip = $derived.by(() => {
    if (isError) return "Failed";
    if (rec.is_meeting) return "Meeting";
    if (rec.source === "upload") return "Uploaded";
    if (rec.platform === "mobile") return "From phone";
    if (rec.platform === "web") return "From web";
    // A synced row from another machine, once there IS another machine.
    if (rec.remote && showDeviceChip) return deviceChipName;
    return "";
  });

  async function ensureAudioUrl() {
    if (audioUrl) return;
    try {
      // Use the data: URL command — bypasses Tauri's asset protocol entirely.
      // Backend reads the WAV file and returns base64. Heavier on memory than
      // streaming but bulletproof on Windows where the asset protocol scope
      // doesn't play nice with AppData paths.
      audioUrl = await api.audioDataUrlFor(rec.id);
      console.log("[audio] data URL loaded for", rec.id, "size=", audioUrl.length);
    } catch (e) {
      console.error("[audio] audioDataUrlFor failed", e);
    }
  }

  async function togglePlay() {
    await ensureAudioUrl();
    // Wait for Svelte to render the <audio> element (it's gated by {#if audioUrl}).
    await tick();
    if (!audioEl) {
      console.warn("[audio] audio element not yet bound after tick — retrying once");
      await tick();
    }
    if (!audioEl) {
      console.error("[audio] audio element still null, aborting");
      return;
    }
    if (audioEl.paused) {
      try {
        await audioEl.play();
        playing = true;
      } catch (e) {
        console.error("[audio] play() failed", e);
      }
    } else {
      audioEl.pause();
      playing = false;
    }
  }

  async function copyText() {
    await writeText(copyableText);
  }

  /// Click handler for the Cleaned / Drafted tabs. If the version exists,
  /// just switch the active tab. If it doesn't, fire the on-demand
  /// generate command, wait for the result, then switch to it. Errors
  /// surface as a simple alert — the row already shows status info via
  /// the (i) inspector, no need to over-engineer.
  async function onTabClick(kind: "cleaned" | "drafted" | "meeting_notes") {
    if (kind === "cleaned" && rec.cleaned_text) { tabOverride = "cleaned"; return; }
    if (kind === "drafted" && rec.drafted_text) { tabOverride = "drafted"; return; }
    if (kind === "meeting_notes" && rec.meeting_notes_text) { tabOverride = "meeting_notes"; return; }
    if (!rec.transcript) {
      await showMessage("No raw transcript yet — retry the recording first.");
      return;
    }
    await runAlt(kind);
  }

  /// Fire the backend generate command and fold the result back into the row.
  /// Shared by the tab click (generate-if-missing) and the kebab's re-run
  /// (regenerate over existing text) — the backend call is identical, it
  /// always regenerates against the CURRENTLY selected LLM provider + model.
  async function runAlt(kind: "cleaned" | "drafted" | "meeting_notes") {
    generating = kind;
    try {
      await api.generateAltVersion(rec.id, kind);
      // Pull the freshly-generated text into our local rec via a refresh.
      // The history-store refresh re-fetches and re-renders the list.
      await history.refresh();
      tabOverride = kind;
    } catch (e) {
      await showMessage(`Could not generate ${kind} version: ${e}`);
    } finally {
      generating = null;
    }
  }

  // Label the current LLM pick so the re-run menu items read as "try it with
  // THIS model" — the whole point of the feature is switching model in the
  // sidebar and having another go.
  let llmLabel = $derived(shortModel(settings.s.llm_model));

  /// Re-run cleanup / draft on an existing recording, the LLM-side twin of
  /// "Re-run transcription". Change the model in the sidebar, come here, run
  /// it again. Confirms when it would overwrite text that already exists,
  /// for the same reason retry() does: it's a one-click way to lose output.
  async function rerunAlt(kind: "cleaned" | "drafted") {
    if (!rec.transcript) {
      await showMessage("No raw transcript yet — re-run transcription first.");
      return;
    }
    const existing = kind === "cleaned" ? rec.cleaned_text : rec.drafted_text;
    const what = kind === "cleaned" ? "cleanup" : "draft";
    if (existing) {
      const ok = await askConfirm(
        `Re-run ${what} with ${llmLabel}? The current ${what} will be replaced.`,
      );
      if (!ok) return;
    }
    await runAlt(kind);
  }

  // Per-row delete now opens the reworked dialog (voice files / transcripts,
  // this-device / everywhere). Signed-in users get the "Everywhere" option
  // for free via the shared dialog.
  let deleteOpen = $state(false);
  function remove() {
    deleteOpen = true;
  }
  function onDeleted() {
    void history.refresh();
  }

  async function retry() {
    // On non-failed rows, confirm before nuking the existing transcript.
    // Done rows are the easy mis-click target — "I'll just check this
    // recording" → accidentally re-burn an STT call.
    if (!isError) {
      const ok = await askConfirm(
        "Re-run transcription on this recording? The current transcript and cleaned text will be replaced.",
      );
      if (!ok) return;
    }
    busy = true;
    try {
      await api.retryRecording(rec.id);
      await history.refresh();
    } catch (e) {
      await showMessage(`Retry failed: ${e}`);
    } finally {
      busy = false;
    }
  }

  function fmtFullTime(iso: string): string {
    try {
      const d = new Date(iso);
      return d.toLocaleString([], {
        year: "numeric",
        month: "short",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit",
        second: "2-digit",
      });
    } catch {
      return iso;
    }
  }
</script>

<!-- A recording is an article, not a giant button containing other buttons.
     Expansion has one explicit control, so keyboard and screen-reader focus
     never inherit the old nested-interactive ambiguity. -->
<article
  class="row"
  class:expanded
  class:error-row={isError}
  class:meeting-row={rec.is_meeting}
>
  <!-- The whole header is the expand control, replacing the old dedicated caret
       button (two ways to open one row was the nested-interactive ambiguity the
       surveys flagged). It must stay keyboard-operable, so this is a real
       role=button with tabindex and aria-expanded, handling Enter and Space —
       a bare click handler here would have made expansion mouse-only. Nested
       buttons stop propagation, so they still do their own job. -->
  <header
    class="row-head"
    role="button"
    tabindex="0"
    aria-expanded={expanded}
    aria-label={`${rec.title || "Untitled recording"} — ${expanded ? "collapse" : "expand"}`}
    onclick={() => (expanded = !expanded)}
    onkeydown={(e) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        expanded = !expanded;
      }
    }}
  >
    <div class="meta">
      <!-- Line 1 is the NAME of the note. It used to sit third, behind time and
           duration, which made a list of cards read as a list of timestamps
           rather than a list of things you said. Title first, clock to the far
           right — the newspaper order. -->
      <span class="rec-title" class:untitled={!rec.title} title={rec.title || "Untitled recording"}>
        {rec.title || "Untitled recording"}
      </span>

      <!-- One chip, and only when this row is NOT a plain dictation. A badge on
           every row carries no information; a badge on the exceptions does. -->
      {#if kindChip}
        <span class="kind-chip" class:kind-failed={isError}>{kindChip}</span>
      {/if}

      <span class="meta-spacer"></span>

      <span class="dur">{durationShort(rec.duration_ms)}</span>
      <span class="when">{timeShort(rec.created_at)}</span>
    </div>

    <!-- Right rail: version tabs (always visible, so every card's Raw /
         Cleaned / Drafted sits at the same aligned x-position) followed by
         the hover-revealed action buttons. Clicks stop propagation so they
         don't toggle row expansion. -->
    <div class="tail" onclick={(e) => e.stopPropagation()} role="presentation">
      {#if rec.transcript || rec.cleaned_text || rec.drafted_text || rec.meeting_notes_text}
        <span class="tabs-inline">
          <button
            class="tab"
            class:active={activeTab === "raw"}
            class:quiet={activeTab !== "raw" && !!rec.transcript}
            class:dim={!rec.transcript}
            disabled={!rec.transcript}
            onclick={() => (tabOverride = "raw")}
            title="Raw transcript"
          >Raw</button>
          <button
            class="tab"
            class:active={activeTab === "cleaned"}
            class:quiet={activeTab !== "cleaned" && !!rec.cleaned_text}
            class:dim={!rec.cleaned_text}
            class:loading={generating === "cleaned"}
            disabled={generating !== null}
            onclick={() => onTabClick("cleaned")}
            title={rec.cleaned_text ? "Cleaned" : "Generate a cleaned version"}
          >{generating === "cleaned" ? "…" : "Cleaned"}</button>
          <button
            class="tab"
            class:active={activeTab === "drafted"}
            class:quiet={activeTab !== "drafted" && !!rec.drafted_text}
            class:dim={!rec.drafted_text}
            class:loading={generating === "drafted"}
            disabled={generating !== null}
            onclick={() => onTabClick("drafted")}
            title={rec.drafted_text ? "Drafted" : "Generate a drafted version"}
          >{generating === "drafted" ? "…" : "Drafted"}</button>
          {#if rec.is_meeting}
            <button
              class="tab"
              class:active={activeTab === "meeting_notes"}
              class:quiet={activeTab !== "meeting_notes" && !!rec.meeting_notes_text}
              class:dim={!rec.meeting_notes_text}
              class:loading={generating === "meeting_notes"}
              disabled={generating !== null}
              onclick={() => onTabClick("meeting_notes")}
              title={rec.meeting_notes_text ? "Meeting notes" : "Generate meeting notes"}
            >Meeting notes</button>
          {/if}
        </span>
      {/if}

      <div class="actions">
      <!-- Play is hidden for remote rows (synced from another device): audio
           never leaves the device that recorded it, so there's nothing local
           to play. The slot is still reserved, otherwise Copy + the kebab
           slide left on synced rows and stop lining up with local ones. -->
      {#if rec.remote}
        <span class="action-slot" aria-hidden="true"></span>
      {:else}
        <button class="action-btn play" onclick={togglePlay} disabled={busy} title="Play / pause audio">
          {#if playing}
            <svg viewBox="0 0 16 16" width="14" height="14"><rect x="4" y="3" width="3" height="10" fill="currentColor"/><rect x="9" y="3" width="3" height="10" fill="currentColor"/></svg>
          {:else}
            <svg viewBox="0 0 16 16" width="14" height="14"><path d="M 5 3 L 13 8 L 5 13 Z" fill="currentColor"/></svg>
          {/if}
        </button>
      {/if}

      <!-- Copy: only meaningful when there's actual text. Hidden on
           rows that errored before producing a transcript. -->
      {#if !isError || rec.transcript || rec.cleaned_text}
        <button class="action-btn" onclick={copyText} disabled={busy} title="Copy text">
          <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
            <rect x="4" y="3" width="8" height="10" rx="1.2" fill="none" stroke="currentColor" stroke-width="1.6"/>
            <rect x="2.5" y="1.5" width="8" height="10" rx="1.2" fill="none" stroke="currentColor" stroke-width="1.4" opacity="0.55"/>
          </svg>
        </button>
      {/if}

      <!-- 3-dot kebab menu — destination for less-frequently-used row
           actions (Retry, Delete) per the v0.4.0 design playbook. Retry
           used to be a top-level button; on the new layout we keep Play
           + Copy prominent and tuck the rest behind this menu. -->
      <button class="action-btn" onclick={() => (readerOpen = true)} disabled={busy || !displayedText} title="Focused reading mode" aria-label="Open focused reading mode">
        <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><path d="M5.5 2.5h-3v3M10.5 2.5h3v3M5.5 13.5h-3v-3M10.5 13.5h3v-3" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>
      </button>

      <div class="kebab-wrap">
        <button
          class="action-btn kebab"
          class:emphasized={isError}
          onclick={(e) => { e.stopPropagation(); kebabOpen = !kebabOpen; }}
          aria-haspopup="menu"
          aria-expanded={kebabOpen}
          title="More"
        >
          <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
            <circle cx="8" cy="3" r="1.6" fill="currentColor" />
            <circle cx="8" cy="8" r="1.6" fill="currentColor" />
            <circle cx="8" cy="13" r="1.6" fill="currentColor" />
          </svg>
        </button>
        {#if kebabOpen}
          <!-- role="menu" needs a focus target; tabindex=-1 makes it
               programmatically focusable. Clicks inside are already stopped
               from toggling the row by the parent .actions handler, so no
               onclick (and thus no missing-keyboard-handler warning) here. -->
          <div class="kebab-menu" role="menu" tabindex="-1">
            <button
              class="kebab-item"
              class:emphasized={isError}
              role="menuitem"
              disabled={retryDisabled}
              onclick={() => { kebabOpen = false; rerunOpen = true; }}
            >
              <svg viewBox="0 0 16 16" width="13" height="13" aria-hidden="true">
                <path d="M 13 4 L 13 8 L 9 8" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>
                <path d="M 13 8 A 5 5 0 1 1 11 4.5" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>
              </svg>
              <span>Rerun...</span>
            </button>
            <!-- LLM-side twins of "Re-run transcription": swap the model in
                 the sidebar, then take another pass at cleanup or draft
                 without burning a fresh STT call. Both need a transcript to
                 work from, so they're hidden until one exists. -->
            {#if false && rec.transcript}
              <button
                class="kebab-item"
                role="menuitem"
                disabled={busy || generating !== null}
                onclick={() => { kebabOpen = false; rerunAlt("cleaned"); }}
              >
                <svg viewBox="0 0 16 16" width="13" height="13" aria-hidden="true">
                  <path d="M 13 4 L 13 8 L 9 8" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>
                  <path d="M 13 8 A 5 5 0 1 1 11 4.5" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>
                  <path d="M 3 12 L 6 12" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>
                </svg>
                <span>{rec.cleaned_text ? "Re-run cleanup" : "Cleanup"} · {llmLabel}</span>
              </button>
              <button
                class="kebab-item"
                role="menuitem"
                disabled={busy || generating !== null}
                onclick={() => { kebabOpen = false; rerunAlt("drafted"); }}
              >
                <svg viewBox="0 0 16 16" width="13" height="13" aria-hidden="true">
                  <path d="M 13 4 L 13 8 L 9 8" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>
                  <path d="M 13 8 A 5 5 0 1 1 11 4.5" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>
                  <path d="M 3 11 L 7 11 M 3 13.5 L 5.5 13.5" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>
                </svg>
                <span>{rec.drafted_text ? "Re-run draft" : "Draft"} · {llmLabel}</span>
              </button>
            {/if}
            {#if rec.is_meeting}
              <button class="kebab-item" role="menuitem" onclick={() => { kebabOpen = false; speakersOpen = true; }}>
                <svg viewBox="0 0 16 16" width="13" height="13" aria-hidden="true"><circle cx="5.5" cy="5" r="2" fill="none" stroke="currentColor" stroke-width="1.4"/><circle cx="11" cy="6" r="1.6" fill="none" stroke="currentColor" stroke-width="1.3"/><path d="M2.5 13c.3-2.3 1.5-3.5 3.2-3.5S8.7 10.7 9 13M9 10c1.9-.7 3.8.2 4.3 2" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/></svg>
                <span>Name speakers</span>
              </button>
            {/if}
            <!-- Delete is ownership-scoped: only rows this device originated can
                 be deleted, so it's hidden entirely on rows synced from another
                 device (a disabled-but-visible control reads as a bug). -->
            {#if !rec.remote}
              <button
                class="kebab-item danger"
                role="menuitem"
                disabled={busy}
                onclick={() => { kebabOpen = false; remove(); }}
              >
                <svg viewBox="0 0 16 16" width="13" height="13" aria-hidden="true">
                  <path d="M 3 4 L 13 4 M 5 4 V 13 A 1 1 0 0 0 6 14 H 10 A 1 1 0 0 0 11 13 V 4 M 7 7 V 11 M 9 7 V 11 M 6 4 V 3 A 1 1 0 0 1 7 2 H 9 A 1 1 0 0 1 10 3 V 4" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>
                </svg>
                <span>Delete recording</span>
              </button>
            {/if}
          </div>
        {/if}
      </div>
      </div>
    </div>
  </header>

  <div class="body" class:clamped={!expanded}>
    {#if isError}
      <div class="err">
        <strong>Failed:</strong> {rec.error || "unknown error"}
      </div>
    {/if}
    {#if captureGap}
      <!-- The mic dropped mid-recording: less audio reached the file than the
           timer ran, so this transcript is cut short. Retrying re-reads the same
           short audio — it can't recover what was never captured. -->
      <div class="capgap">
        <strong>Mic dropped mid-recording.</strong> Only ~{capturedS}s of your
        {durationShort(rec.duration_ms)} recording was captured (~{captureLostS}s
        lost), so this transcript is cut short. Re-record to get the rest —
        retrying won't recover audio that wasn't captured.
      </div>
    {/if}
    {#if turns.length}
      <div class="speaker-blocks">
        {#each turns as turn}
          <div class="speaker-block"><strong>{namedSpeaker(turn.speaker, names)}</strong><p>{turn.text}</p></div>
        {/each}
      </div>
    {:else}
      <div class="text readable-text">
        {#each readableBlocks as block}
          {#if block.split("\n").every((line) => /^[-*] /.test(line))}
            <ul>{#each block.split("\n") as line}<li>{line.replace(/^[-*]\s+/, "")}</li>{/each}</ul>
          {:else if /^#{1,3}\s/.test(block)}
            <h3>{block.replace(/^#{1,3}\s+/, "")}</h3>
          {:else}
            <p>{block}</p>
          {/if}
        {/each}
      </div>
    {/if}
    {#if fellBackToRaw}
      <p class="fallback-warn">
        This is the raw transcript, not a generated version — {fallbackReason}.
        Try Rerun, or pick a different model in Settings.
      </p>
    {/if}
    {#if expanded && rec.clippy_note}
      <p class="note">Clippy note: {rec.clippy_note}</p>
    {/if}
    {#if expanded}
      <!-- Expanded action area stops click-bubbling so its buttons don't
           re-toggle the row when clicked. -->
      <div role="presentation" onclick={(e) => e.stopPropagation()}>
      <!-- Details moved off the collapsed header. It used to be an always-on
           (i) button on every row, competing with the version tabs and the
           action rail for the same strip; it is reference material, so it
           belongs one deliberate click inside the opened row. The red dot still
           rides along when there is an error worth reading. -->
      <div class="expanded-actions">
        <button
          class="details-link"
          class:has-news={inspectorHasNews}
          onclick={() => (showInspector = !showInspector)}
          aria-expanded={showInspector}
        >
          <span class="details-caret" class:open={showInspector}>›</span>
          {showInspector ? "Hide details" : "Details"}
        </button>
        <!-- Delete used to live in the action button row. Kept here so it takes
             a deliberate click instead of being a thumb-reachable danger button
             alongside Play/Copy/Retry. Still confirms. Hidden on rows synced
             from another device — ownership-scoped delete can only remove what
             this device originated. -->
        {#if !rec.remote}
          <button class="delete-link" onclick={remove} disabled={busy}>Delete recording</button>
        {/if}
      </div>
      </div>
    {/if}
  </div>

  {#if showInspector}
    <!-- Inline details panel. Sits below the body so it doesn't cover
         anything; collapses cleanly without layout shift elsewhere. -->
    <div class="inspector" role="region" aria-label="Recording details">
      <div class="insp-grid">
        <div class="insp-k">Status</div>
        <div class="insp-v">
          <span class="insp-badge insp-badge-{rec.status}">{rec.status}</span>
        </div>

        {#if rec.error}
          <div class="insp-k">Last error</div>
          <div class="insp-v">
            <pre class="insp-err">{rec.error}</pre>
          </div>
        {/if}

        <div class="insp-k">Retries</div>
        <div class="insp-v">{rec.retry_count}</div>

        <div class="insp-k">Mode</div>
        <div class="insp-v">{modeLabel(rec.mode)}</div>

        <div class="insp-k">Duration</div>
        <div class="insp-v">{durationShort(rec.duration_ms)}</div>

        {#if rec.audio_captured_ms != null}
          <div class="insp-k">Audio captured</div>
          <div class="insp-v insp-timing" class:insp-slow={captureGap}>
            {fmtMs(rec.audio_captured_ms)}{#if captureGap}<span class="slow-tag"
                >~{captureLostS}s lost</span
              >{/if}
          </div>
        {/if}

        {#if rec.stt_ms != null}
          <div class="insp-k">STT time</div>
          <div class="insp-v insp-timing" class:insp-slow={sttSlow}>
            {fmtMs(rec.stt_ms)}{#if sttSlow}<span class="slow-tag">slow</span>{/if}
          </div>
        {/if}
        {#if rec.cleanup_ms != null}
          <div class="insp-k">Cleanup time</div>
          <div class="insp-v insp-timing">{fmtMs(rec.cleanup_ms)}</div>
        {/if}
        {#if rec.total_ms != null}
          <div class="insp-k">Turnaround</div>
          <div class="insp-v insp-timing">{fmtMs(rec.total_ms)}</div>
        {/if}

        <div class="insp-k">STT provider</div>
        <div class="insp-v insp-mono">{rec.stt_provider ?? "—"}</div>

        <div class="insp-k">LLM provider</div>
        <div class="insp-v insp-mono">{rec.llm_provider ?? "—"}</div>

        {#if rec.clippy_note}
          <div class="insp-k">Clippy note</div>
          <div class="insp-v">{rec.clippy_note}</div>
        {/if}

        <div class="insp-k">Created</div>
        <div class="insp-v">{fmtFullTime(rec.created_at)}</div>

        <!-- Moved out of the collapsed header, which used to carry five
             competing chips. Labelled here rather than abbreviated to a glyph:
             a row that needs this information needs it named. -->
        {#if showDeviceChip || rec.remote}
          <div class="insp-k">Recorded on</div>
          <div class="insp-v">{deviceChipGlyph} {deviceChipName}</div>
        {/if}

        <div class="insp-k">Source</div>
        <div class="insp-v">
          {rec.source === "upload" ? "Uploaded audio file" : "Dictated"}
          {#if showPlatformBadge}· {platformLabel(rec.platform)}{/if}
        </div>

        {#if rec.retry_count > 0}
          <div class="insp-k">Retries</div>
          <div class="insp-v">
            {rec.retry_count}
            {rec.retry_count === 1 ? "attempt" : "attempts"} after the first
          </div>
        {/if}

        <div class="insp-k">Audio</div>
        <div class="insp-v insp-mono insp-small">{rec.audio_path}</div>

        <div class="insp-k">ID</div>
        <div class="insp-v insp-mono insp-small">{rec.id}</div>
      </div>

      {#if timeline.length > 0}
        <!-- Flight-recorder timeline: each line is elapsed-since-start + what
             happened. This is where a slow/failed run explains itself. -->
        <div class="insp-timeline">
          <div class="insp-tl-head">Timeline</div>
          <ol class="insp-tl">
            {#each timeline as ev}
              <li>
                <span class="tl-ms">{fmtMs(ev.ms)}</span>
                <span class="tl-msg">{ev.msg}</span>
              </li>
            {/each}
          </ol>
        </div>
      {:else if !hasTiming}
        <p class="insp-noteline">
          No timing recorded for this run (it predates the flight recorder, added
          in this build). New recordings will show a full timeline here.
        </p>
      {/if}
    </div>
  {/if}

  <!-- (Old "Polished / Base" toggle removed — replaced by the 3-tab bar
       at the top of the body.) -->

  {#if audioUrl}
    <audio
      bind:this={audioEl}
      src={audioUrl}
      onended={() => (playing = false)}
      onpause={() => (playing = false)}
      onplay={() => (playing = true)}
      class="hidden-audio"
    ></audio>
  {/if}
</article>

<!-- Sits outside the row so the overlay isn't clipped by the card. Clicks on
     the dialog stop propagation, so opening it never toggles row expansion. -->
<DeleteDialog bind:open={deleteOpen} ids={[rec.id]} label="this recording" onDone={onDeleted} />
<RerunDialog bind:open={rerunOpen} {rec} />
<SpeakerLabelsDialog bind:open={speakersOpen} {rec} />
<ReadingMode bind:open={readerOpen} {rec} version={activeTab} text={displayedText} />

<style>
  /* Each recording is its own floating card on the cream surface (design
     playbook mock) — rounded, bordered, with gaps between cards instead of
     rule lines between flat rows. */
  .row {
    display: flex;
    flex-direction: column;
    padding: 13px 16px;
    background: var(--bg-card);
    border: 1px solid var(--border-subtle);
    border-radius: 14px;
    transition: border-color var(--motion-fast) ease, box-shadow var(--motion-base) ease, transform var(--motion-base) var(--ease-standard);
    position: relative;
    container-type: inline-size;
  }

  .row:hover {
    border-color: var(--border);
    box-shadow: var(--shadow-sm);
    transform: translateY(-1px);
  }

  .row:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  .row.error-row {
    background: var(--danger-fade);
    border-color: var(--danger-fade);
  }

  .row.meeting-row {
    background: color-mix(in srgb, var(--bg-card) 86%, #dceeff 14%);
    border-color: color-mix(in srgb, var(--border) 72%, #8eb9da 28%);
  }

  .row:has(.kebab-menu) {
    z-index: 120;
  }

  .row-head {
    display: flex;
    align-items: center;
    gap: 10px;
    /* The whole header is the expand control now. The old design had TWO ways
       to open a row — a dedicated caret button AND the header — which is the
       nested-interactive ambiguity the surveys flagged. Buttons inside stop
       propagation, so they still do their own job. */
    cursor: pointer;
  }

  /* Keyboard users must be able to see what they are about to open. */
  .row-head:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 3px;
    border-radius: 6px;
  }

  .meta {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--text-secondary);
    min-width: 0;
    /* nowrap + ellipsis on the title keeps every card's header a single
       aligned line — no staggered second rows. */
    flex-wrap: nowrap;
  }

  /* Pushes duration + time to the right edge, so the clock sits in one column
     down the whole list instead of floating after a variable-length title. */
  .meta-spacer {
    flex: 1 1 auto;
    min-width: 8px;
  }

  .when {
    font-weight: 500;
    color: var(--text-secondary);
    flex-shrink: 0;
    font-variant-numeric: tabular-nums;
  }

  /* The auto-title now LEADS the card — it is the name of the note, and a list
     of names reads as a list of things you said, where a list of timestamps
     does not. Full size, primary colour, ellipsis rather than wrap. */
  .rec-title {
    font-size: 13.5px;
    font-weight: 600;
    color: var(--text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
    flex: 0 1 auto;
  }

  /* Until the auto-title arrives (a beat after the run finishes) the row still
     needs a name, so it says so quietly rather than showing an empty slot. */
  .rec-title.untitled {
    font-weight: 500;
    color: var(--text-secondary);
    font-style: italic;
  }

  /* The single "this row is not an ordinary dictation" chip that replaced five
     competing header badges. */
  .kind-chip {
    flex-shrink: 0;
    padding: 1px 7px;
    border-radius: 999px;
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.02em;
    background: var(--bg-subtle);
    color: var(--text-secondary);
    border: 1px solid var(--border);
    max-width: 140px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .kind-chip.kind-failed {
    background: color-mix(in srgb, var(--danger) 12%, transparent);
    border-color: color-mix(in srgb, var(--danger) 35%, transparent);
    color: var(--danger);
  }

  .tail {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-shrink: 0;
    margin-left: auto;
  }

  .dur {
    color: var(--text-tertiary, var(--text-secondary));
    flex-shrink: 0;
    font-variant-numeric: tabular-nums;
  }

  /* The "Uploaded" pill, the platform badge and the dot separators were all
     header furniture. They are gone from the collapsed card: the single
     .kind-chip names the one thing that makes a row unusual, and the rest is
     labelled in Details. */

  .mode {
    padding: 2px 9px;
    border-radius: 9999px;
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .mode.light {
    background: var(--accent-fade);
    color: var(--accent);
  }

  .mode.advanced {
    background: rgba(175, 82, 222, 0.18);
    color: #af52de;
  }

  .mode.drafting {
    background: rgba(255, 159, 10, 0.20);
    color: #c47a30;
  }

  /* .err-pill and .retry-count retired with the busy header — a failed row is
     now named by .kind-chip, and the retry count is labelled in Details. */

  .actions {
    display: flex;
    align-items: center;
    gap: 6px;
    /* Dimmed at rest, full strength on hover or keyboard focus.
       Deliberately NOT hidden: with ~14 rows on screen, four solid icons per
       row is the noise the redesign is fixing, but hiding them outright means
       reaching for a control that is not there. Dimming quiets the list while
       keeping every button discoverable and in the same place. */
    opacity: 0.45;
    visibility: visible;
    transition: opacity 120ms ease;
  }

  .row:hover .actions,
  .row:focus-within .actions,
  .row.expanded .actions {
    opacity: 1;
  }

  /* Touch and reduced-motion users get no hover, so never dim for them. */
  @media (hover: none) {
    .actions {
      opacity: 1;
    }
  }

  /* Keep an open kebab menu (and its trigger cluster) visible even if the
     pointer drifts off the row while the menu is up. */
  .actions:focus-within {
    opacity: 1;
    visibility: visible;
  }

  /* Placeholder for the play button on synced rows — same footprint, no
     paint, so every row's action rail stays on the same grid. */
  .action-slot {
    width: 34px;
    height: 34px;
    flex: 0 0 auto;
  }

  .action-btn {
    background: var(--bg-card);
    border: 1px solid var(--border);
    cursor: pointer;
    width: 34px;
    height: 34px;
    border-radius: 9px;
    color: var(--text-primary);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    transition: background 120ms ease, border-color 120ms ease, color 120ms ease, transform 120ms ease;
  }

  .action-btn:hover:not(:disabled) {
    background: var(--bg-subtle);
    border-color: var(--text-secondary);
    transform: translateY(-1px);
  }

  .action-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .action-btn.play:hover:not(:disabled) {
    color: var(--accent);
    border-color: var(--accent);
  }

  .action-btn.delete:hover:not(:disabled) {
    background: var(--danger-fade);
    border-color: var(--danger);
    color: var(--danger);
  }

  /* Retry is now always-visible. Default look matches the other action
     buttons (subtle, monochrome). `emphasized` is added when the row
     errored, making Retry the obvious recovery action. */
  .action-btn.retry:hover:not(:disabled) {
    color: var(--warning, var(--accent));
    border-color: var(--warning, var(--accent));
  }

  /* Kebab (3-dot menu) button + popover. Sits at the right end of the
     row's action group; opens a small popover with Retry + Delete. */
  .kebab-wrap {
    position: relative;
  }
  .action-btn.kebab:hover:not(:disabled) {
    color: var(--accent);
    border-color: var(--accent);
  }
  .action-btn.kebab.emphasized {
    color: var(--warning);
    border-color: var(--warning);
    background: var(--warning-fade);
  }
  .kebab-menu {
    position: absolute;
    right: 0;
    top: 100%;
    margin-top: 6px;
    min-width: 180px;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 10px;
    box-shadow: 0 6px 24px rgba(120, 80, 30, 0.18);
    padding: 4px;
    display: flex;
    flex-direction: column;
    gap: 1px;
    z-index: 1000;
    isolation: isolate;
  }
  .kebab-item {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 7px 10px;
    background: transparent;
    border: none;
    border-radius: 6px;
    font-size: 12px;
    color: var(--text-primary);
    cursor: pointer;
    text-align: left;
    font-family: inherit;
  }
  .kebab-item:hover:not(:disabled) {
    background: var(--bg-subtle);
  }
  .kebab-item:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
  .kebab-item.danger {
    color: var(--danger);
  }
  .kebab-item.danger:hover:not(:disabled) {
    background: var(--danger-fade);
  }
  .kebab-item.emphasized {
    color: var(--warning);
    font-weight: 500;
  }

  .action-btn.retry.emphasized {
    background: var(--warning-fade);
    border-color: var(--warning);
    color: var(--warning);
  }
  .action-btn.retry.emphasized:hover:not(:disabled) {
    background: var(--warning);
    color: #fff;
  }

  /* Round (i) details button. Italic serif "i" — the classic affordance.
     Pulses a red dot in the top-right when there's an error to surface. */
  /* The (i) circle is gone from the collapsed header. Details is now a text
     link inside the opened row, so it reads as a label rather than a glyph the
     user has to learn — and it stops competing with the version tabs and the
     action rail for the same strip on every single card. */
  .details-link {
    background: transparent;
    border: none;
    padding: 0;
    cursor: pointer;
    color: var(--text-secondary);
    font-size: 12px;
    font-family: inherit;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    position: relative;
    margin-right: auto;
  }

  .details-link:hover {
    color: var(--text-primary);
  }

  .details-caret {
    display: inline-block;
    transition: transform 160ms cubic-bezier(0.32, 0.72, 0, 1);
    font-size: 13px;
    line-height: 1;
  }

  .details-caret.open {
    transform: rotate(90deg);
  }

  /* Still flags an error worth reading without needing to be opened. */
  .details-link.has-news::after {
    content: "";
    position: absolute;
    top: -1px;
    right: -9px;
    width: 6px;
    height: 6px;
    background: var(--danger);
    border-radius: 50%;
  }

  /* Delete moved out of the icon button row into a subtle text link
     that only appears when the row is expanded. Less mis-click surface. */
  .expanded-actions {
    margin-top: 10px;
    display: flex;
    justify-content: flex-end;
  }
  .delete-link {
    background: transparent;
    border: none;
    color: var(--text-secondary);
    font-size: 11px;
    cursor: pointer;
    padding: 2px 6px;
    border-radius: 4px;
    text-decoration: underline dotted;
    text-underline-offset: 3px;
  }
  .delete-link:hover:not(:disabled) {
    color: var(--danger);
    text-decoration-color: var(--danger);
  }
  .delete-link:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  /* Inspector panel — inline expansion below the row body. Two-column
     key/value grid; the error text gets its own monospace block. */
  .inspector {
    /* Was indented 36px to clear the old caret column; that column is gone. */
    margin: 10px 0 0;
    padding: 12px 14px;
    background: var(--bg-subtle);
    border: 1px solid var(--border-subtle);
    border-radius: 8px;
  }
  .insp-grid {
    display: grid;
    grid-template-columns: 110px 1fr;
    gap: 6px 12px;
    font-size: 12px;
    align-items: baseline;
  }
  .insp-k {
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    font-size: 10px;
    font-weight: 600;
  }
  .insp-v {
    color: var(--text-primary);
    overflow-wrap: anywhere;
  }
  .insp-v.insp-mono {
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
  }
  .insp-v.insp-small {
    font-size: 11px;
  }
  .insp-badge {
    display: inline-block;
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    padding: 1px 7px;
    border-radius: 9999px;
    border: 1px solid var(--border);
  }
  .insp-badge-done {
    background: var(--accent-fade);
    color: var(--accent);
    border-color: var(--accent);
  }
  .insp-badge-error {
    background: var(--danger-fade);
    color: var(--danger);
    border-color: var(--danger);
  }
  .insp-badge-recording,
  .insp-badge-transcribing,
  .insp-badge-cleaning,
  .insp-badge-injecting {
    background: var(--bg-card);
    color: var(--text-secondary);
  }
  .insp-err {
    margin: 0;
    padding: 8px 10px;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 4px;
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
    font-size: 11px;
    color: var(--danger);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    max-height: 200px;
    overflow-y: auto;
  }

  /* Stage timings — tabular figures so 1.2s / 19.4s line up. A slow STT is
     flagged in the warning colour so the eye lands on it immediately. */
  .insp-v.insp-timing {
    font-variant-numeric: tabular-nums;
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
  }
  .insp-v.insp-slow {
    color: var(--warning, #c47a30);
    font-weight: 600;
  }
  .slow-tag {
    margin-left: 6px;
    font-family: inherit;
    font-size: 9px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    background: var(--warning-fade, rgba(196, 122, 48, 0.16));
    color: var(--warning, #c47a30);
    padding: 1px 5px;
    border-radius: 9999px;
    vertical-align: middle;
  }

  /* Timeline — the run's flight recorder. Compact ordered list; left column
     is elapsed-since-start, right column the event. Monospace ms keeps the
     timeline scannable top-to-bottom. */
  .insp-timeline {
    margin-top: 10px;
    border-top: 1px dashed var(--border);
    padding-top: 8px;
  }
  .insp-tl-head {
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    font-size: 10px;
    font-weight: 600;
    margin-bottom: 6px;
  }
  .insp-tl {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .insp-tl li {
    display: grid;
    grid-template-columns: 56px 1fr;
    gap: 10px;
    font-size: 11px;
    align-items: baseline;
  }
  .tl-ms {
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
    font-variant-numeric: tabular-nums;
    color: var(--text-secondary);
    text-align: right;
  }
  .tl-msg {
    color: var(--text-primary);
    overflow-wrap: anywhere;
  }
  .insp-noteline {
    margin: 10px 0 0;
    font-size: 11px;
    color: var(--text-secondary);
    line-height: 1.5;
  }

  /* Device chip. Sized and baseline-aligned to sit quietly next to the
     timestamp rather than competing with it. */
  /* The device glyph left the collapsed header — on a one-device account it
     always answered "this computer", and on a fleet the answer belongs in
     Details under a "Recorded on" label rather than as an emoji to decode. */

  .body {
    margin-top: 10px;
    /* No caret column to clear any more, so the text lines up under the title
       instead of being indented past a control that no longer exists. */
    padding-left: 0;
  }

  .body.clamped .text {
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .body.clamped .speaker-blocks {
    max-height: 4.7em;
    overflow: hidden;
  }

  .speaker-blocks {
    display: grid;
    gap: 10px;
  }

  .speaker-block {
    display: grid;
    grid-template-columns: 92px minmax(0, 1fr);
    gap: 14px;
    padding: 10px 12px;
    background: color-mix(in srgb, var(--bg-card) 78%, transparent);
    border: 1px solid var(--border-subtle);
    border-radius: 10px;
  }

  .speaker-block strong {
    color: var(--accent);
    font-size: 11px;
    line-height: 1.5;
  }

  .speaker-block p {
    margin: 0;
    font-size: 14px;
    line-height: 1.65;
    white-space: pre-wrap;
  }

  /* Inline version tabs — Raw / Cleaned / Drafted. v0.4.0 design playbook
     gives them their own visual identity: the active tab is filled with
     the accent orange + white text (the playbook's "Cleaned" pill), other
     tabs are subtle cream-fill outlines. Dim (= not yet generated) tabs
     are translucent and italic to read as a CTA, not data. */
  .tabs-inline {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }

  .tab {
    background: var(--bg-card);
    border: 1px solid var(--border);
    padding: 2px 10px;
    font-size: 10px;
    line-height: 1.5;
    font-weight: 500;
    color: var(--text-secondary);
    cursor: pointer;
    border-radius: 999px;
    transition: background 100ms ease, color 100ms ease, border-color 100ms ease;
    font-family: inherit;
    letter-spacing: 0.01em;
  }

  .tab:hover:not(:disabled) {
    color: var(--text-primary);
    border-color: var(--text-secondary);
  }

  .tab.active {
    background: var(--accent);
    border-color: var(--accent);
    color: #ffffff;
    box-shadow: 0 1px 2px rgba(184, 84, 18, 0.25);
  }
  .tab.active:hover {
    background: var(--accent-hover, var(--accent));
    border-color: var(--accent-hover, var(--accent));
    color: #ffffff;
  }

  /* Quiet = version exists but isn't the active tab. Plain outline, no
     fill — keeps at most one filled-orange element (the active tab) per row. */
  .tab.quiet {
    background: transparent;
    color: var(--text-secondary);
    border-color: var(--border);
  }
  .tab.quiet:hover:not(:disabled) {
    color: var(--text-primary);
    border-color: var(--text-secondary);
  }

  /* Dim = version doesn't exist yet; still a click target. */
  .tab.dim {
    background: transparent;
    color: var(--text-secondary);
    opacity: 0.6;
    font-style: italic;
    border-style: dashed;
  }
  .tab.dim:hover:not(:disabled) {
    opacity: 1;
    color: var(--accent);
    border-color: var(--accent);
    background: var(--accent-fade);
  }

  .tab.loading {
    color: var(--accent);
    opacity: 1;
    font-style: italic;
    border-color: var(--accent);
    background: var(--accent-fade);
  }

  .tab:disabled {
    cursor: not-allowed;
  }

  .text {
    font-size: 14px;
    line-height: 1.55;
    color: var(--text-primary);
    margin: 0;
    white-space: pre-wrap;
    word-break: break-word;
  }

  /* Transcript text fills the card. It used to carry a fixed `max-width:
     88ch` reading measure, which does not scale with the card: the same
     sentence wrapped at the same word in a 900px window and a maximised
     one, so on a wide window the copy stopped around half-way across the
     card and the rest sat empty. The card is the measure — it already sits
     in a padded column inside the history pane — so cap at 100% and let the
     line length follow the window the user chose. */
  .readable-text {
    max-width: 100%;
    letter-spacing: 0.004em;
  }

  .readable-text p {
    margin: 0 0 0.9em;
  }

  .readable-text p:last-child,
  .readable-text ul:last-child {
    margin-bottom: 0;
  }

  .readable-text h3 {
    margin: 1.15em 0 0.45em;
    font-size: 14px;
  }

  .readable-text h3:first-child {
    margin-top: 0;
  }

  .readable-text ul {
    margin: 0 0 0.9em;
    padding-left: 1.35em;
  }

  /* Compact variant toggle — inline pill, tiny chevrons. */
  .variant-toggle {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin: 6px 0 0;
    padding: 3px 8px;
    background: var(--bg-subtle);
    border: 1px solid var(--border-subtle);
    border-radius: 9999px;
    cursor: pointer;
    color: var(--text-secondary);
    font-size: 11px;
    transition: color 120ms ease, background 120ms ease, border-color 120ms ease;
  }
  .variant-toggle:hover {
    background: var(--accent-fade);
    color: var(--accent);
    border-color: var(--accent);
  }
  .variant-name-compact {
    font-weight: 600;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  @container (max-width: 700px) {
    .row {
      padding: 12px 13px;
    }

    .row-head {
      /* No caret column at narrow widths either — the header is one flow, with
         the tail wrapping beneath it. */
      display: grid;
      grid-template-columns: minmax(0, 1fr);
      align-items: start;
      gap: 6px 8px;
    }

    .meta {
      min-height: 26px;
      gap: 6px;
      flex-wrap: wrap;
      white-space: normal;
    }

    .rec-title {
      flex-basis: 100%;
      order: -1;
      font-size: 13px;
    }

    /* Narrow: the clock does not need pushing to a far edge that no longer
       exists, so the spacer collapses and duration/time sit together. */
    .meta-spacer {
      display: none;
    }

    .tail {
      grid-column: 1;
    }

    .tail {
      width: 100%;
      margin-left: 0;
      justify-content: space-between;
      gap: 8px;
    }

    .tabs-inline {
      min-width: 0;
      overflow-x: auto;
      padding-bottom: 2px;
    }

    .tab {
      flex: 0 0 auto;
    }

    .action-slot,
    .action-btn {
      width: 30px;
      height: 30px;
    }

    .body {
      padding-left: 38px;
    }
  }

  .err {
    color: #b3261e;
    font-size: 12px;
    margin-bottom: 4px;
  }

  /* Capture-gap notice — the mic dropped mid-recording. Warning (amber) rather
     than error (red): the user still got a partial transcript, but needs to
     know it's incomplete and that a re-record — not a retry — is the fix. */
  .capgap {
    color: var(--warning, #c47a30);
    background: var(--warning-fade, rgba(196, 122, 48, 0.12));
    border: 1px solid var(--warning, #c47a30);
    border-radius: 8px;
    font-size: 12px;
    line-height: 1.5;
    padding: 8px 11px;
    margin-bottom: 8px;
  }
  .capgap strong {
    color: var(--warning, #c47a30);
  }

  .fallback-warn {
    margin: 6px 0 0;
    padding: 7px 10px;
    border-radius: 8px;
    border: 1px solid color-mix(in srgb, #b3261e 30%, transparent);
    background: color-mix(in srgb, #b3261e 8%, transparent);
    color: #8c1d18;
    font-size: 12px;
    line-height: 1.45;
  }
  .note {
    color: #ff9f0a;
    font-size: 11px;
    margin: 6px 0 0;
  }

  .hidden-audio {
    display: none;
  }
</style>
