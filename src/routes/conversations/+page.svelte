<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { listen } from "@tauri-apps/api/event";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { save as saveDialog } from "@tauri-apps/plugin-dialog";
  import { api, onFlowState, type Recording } from "$lib/api";
  import UploadDialog from "$lib/UploadDialog.svelte";
  import SpeakerLabelsDialog from "$lib/SpeakerLabelsDialog.svelte";
  import Button from "$lib/ui/Button.svelte";
  import Segmented from "$lib/ui/Segmented.svelte";
  import {
    accuracyText,
    applySpeakerNames,
    conversationDoc,
    mmss,
    namedSpeaker,
    speakerLabels,
    speakerNames,
    speakerTurns,
    speakersAsText,
    type PassName,
    type PassState,
  } from "$lib/meeting-text";

  const AUDIO_EXTS = ["wav", "mp3", "m4a", "aac", "ogg", "oga", "opus", "flac", "webm", "mp4"];

  let list = $state<Recording[]>([]);
  let loading = $state(true);
  let search = $state("");
  let selectedId = $state<string | null>(null);
  let uploadOpen = $state(false);
  let stagedPaths = $state<string[]>([]);
  let dragActive = $state(false);
  let notice = $state<{ text: string; tone: "ok" | "error" } | null>(null);
  let noticeTimer: ReturnType<typeof setTimeout> | undefined;

  function flash(text: string, tone: "ok" | "error" = "ok") {
    notice = { text, tone };
    clearTimeout(noticeTimer);
    noticeTimer = setTimeout(() => (notice = null), tone === "error" ? 8000 : 2500);
  }

  async function refresh() {
    try {
      list = await api.listConversations();
    } catch (e) {
      console.error("conversations.refresh failed", e);
    } finally {
      loading = false;
    }
  }

  function isAudioPath(p: string): boolean {
    return AUDIO_EXTS.includes((p.split(".").pop() || "").toLowerCase());
  }

  function openUpload() {
    stagedPaths = [];
    uploadOpen = true;
  }

  onMount(() => {
    void refresh();
    const unsubs: (() => void)[] = [];
    listen("wispr:history_changed", () => void refresh()).then((u) => unsubs.push(u)).catch(() => {});
    onFlowState((s) => { if (s === "idle") void refresh(); }).then((u) => unsubs.push(u)).catch(() => {});
    // Dropping audio here means "this is a conversation".
    getCurrentWebview()
      .onDragDropEvent((event) => {
        const p = event.payload;
        if (p.type === "enter") {
          dragActive = (p.paths ?? []).some(isAudioPath) || dragActive;
        } else if (p.type === "leave") {
          dragActive = false;
        } else if (p.type === "drop") {
          dragActive = false;
          const audio = (p.paths ?? []).filter(isAudioPath);
          if (audio.length > 0) {
            stagedPaths = audio;
            uploadOpen = true;
          }
        }
      })
      .then((u) => unsubs.push(u))
      .catch(() => {});
    return () => {
      unsubs.forEach((u) => u());
      clearTimeout(noticeTimer);
    };
  });

  let filtered = $derived.by(() => {
    const q = search.trim().toLowerCase();
    if (!q) return list;
    return list.filter((r) =>
      [r.title, r.meeting_notes_text, r.transcript, accuracyText(r), Object.values(speakerNames(r)).join(" ")]
        .filter(Boolean)
        .join(" ")
        .toLowerCase()
        .includes(q),
    );
  });

  let selected = $derived(list.find((r) => r.id === selectedId) ?? null);

  // ── Formatting ───────────────────────────────────────────────────────
  function titleOf(r: Recording): string {
    return r.title?.trim() || "Conversation";
  }
  function dateOf(r: Recording): string {
    const d = new Date(r.created_at);
    return d.toLocaleDateString([], { weekday: "short", day: "numeric", month: "short", year: "numeric" }) +
      " · " + d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  }
  function lengthOf(r: Recording): string {
    const total = Math.round((r.duration_ms || 0) / 1000);
    if (!total) return "";
    const h = Math.floor(total / 3600);
    const m = Math.floor((total % 3600) / 60);
    const s = total % 60;
    return h ? `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}` : `${m}:${String(s).padStart(2, "0")}`;
  }
  function chips(r: Recording): string[] {
    const names = speakerNames(r);
    return speakerLabels(r).map((l) => namedSpeaker(l, names));
  }
  /** First 2 lines of the notes (markup stripped), else of the transcript. */
  function preview(r: Recording): string {
    const names = speakerNames(r);
    const source = r.meeting_notes_text?.trim() || speakerTurns(r).map((t) => `${namedSpeaker(t.speaker, names)}: ${t.text}`).join("\n") || accuracyText(r) || "";
    const lines = applySpeakerNames(source, names)
      .split("\n")
      .map((l) => l.replace(/^#+\s*/, "").replace(/^[-*]\s+/, "").replace(/\*\*/g, "").trim())
      .filter((l) => l && !/^(summary|key points)$/i.test(l));
    return lines.slice(0, 2).join(" · ");
  }

  const PASS_LABEL: Record<PassName, string> = { speakers: "Speakers", accuracy: "Whisper", notes: "Notes" };
  const PASS_ORDER: PassName[] = ["speakers", "accuracy", "notes"];

  function passes(r: Recording): { pass: PassName; state: PassState | null; error: string | null }[] {
    const doc = conversationDoc(r);
    return PASS_ORDER.map((pass) => {
      let state = (doc?.passes?.[pass] as PassState | undefined) ?? null;
      // Rows made before v2 carry no pass states: infer from the content.
      if (!state) {
        if (pass === "speakers" && speakerTurns(r).length) state = "done";
        if (pass === "accuracy") state = doc?.accuracy?.text ? "done" : "skipped";
        if (pass === "notes") state = r.meeting_notes_text?.trim() ? "done" : "skipped";
      }
      return { pass, state, error: doc?.errors?.[pass] ?? null };
    });
  }

  function statusLine(r: Recording): { text: string; tone: "busy" | "error" } | null {
    const ps = passes(r);
    const running = ps.filter((p) => p.state === "running");
    if (running.length) return { text: `Working on ${running.map((p) => PASS_LABEL[p.pass].toLowerCase()).join(" and ")}…`, tone: "busy" };
    const failed = ps.filter((p) => p.state === "failed");
    if (failed.length) return { text: `${failed.map((p) => PASS_LABEL[p.pass]).join(", ")} failed — open to retry`, tone: "error" };
    if (r.status === "error") return { text: r.error ? `Failed: ${r.error}` : "Failed", tone: "error" };
    if (r.status !== "done") return { text: "Working…", tone: "busy" };
    return null;
  }

  // ── Reader ───────────────────────────────────────────────────────────
  type View = "notes" | "speakers" | "whisper";
  let view = $state<View>("notes");
  let speakersOpen = $state(false);
  let focusSpeaker = $state<string | null>(null);
  let copyOpen = $state(false);
  let retrying = $state<Partial<Record<PassName, boolean>>>({});

  function openReader(r: Recording) {
    selectedId = r.id;
    view = r.meeting_notes_text?.trim() ? "notes" : "speakers";
    copyOpen = false;
    retrying = {};
  }

  let selNames = $derived(selected ? speakerNames(selected) : {});
  let selTurns = $derived(selected ? speakerTurns(selected) : []);
  let selWhisper = $derived(selected ? accuracyText(selected) : null);
  let selPasses = $derived(selected ? passes(selected) : []);
  let viewOptions = $derived([
    { value: "notes" as View, label: "Notes" },
    { value: "speakers" as View, label: "Speakers" },
    ...(selWhisper ? [{ value: "whisper" as View, label: "Whisper" }] : []),
  ]);
  $effect(() => {
    if (view === "whisper" && !selWhisper) view = "speakers";
  });

  /** Notes as blocks for display: headings, bullets and paragraphs, with
   *  **bold** runs. Built as data, never as HTML. */
  type Run = { text: string; bold: boolean };
  type Block = { kind: "h" | "li" | "p"; runs: Run[] };
  function runs(text: string): Run[] {
    return text.split(/(\*\*[^*]+\*\*)/g).filter(Boolean).map((part) =>
      part.startsWith("**") && part.endsWith("**") && part.length > 4
        ? { text: part.slice(2, -2), bold: true }
        : { text: part, bold: false },
    );
  }
  let noteBlocks = $derived.by<Block[]>(() => {
    const text = selected?.meeting_notes_text?.trim();
    if (!text) return [];
    return applySpeakerNames(text, selNames)
      .split("\n")
      .map((l) => l.trimEnd())
      .filter((l) => l.trim())
      .map((l) => {
        const h = l.match(/^\s*#{1,6}\s+(.*)$/);
        if (h) return { kind: "h", runs: runs(h[1]) };
        const li = l.match(/^\s*(?:[-*•]|\d+[.)])\s+(.*)$/);
        if (li) return { kind: "li", runs: runs(li[1]) };
        return { kind: "p", runs: runs(l.trim()) };
      });
  });

  function renameSpeaker(label: string | null) {
    focusSpeaker = label;
    speakersOpen = true;
  }

  async function retry(pass: PassName) {
    if (!selected) return;
    const id = selected.id;
    retrying = { ...retrying, [pass]: true };
    try {
      await api.retryConversationPass(id, pass);
      flash(`${PASS_LABEL[pass]} done`);
    } catch (e) {
      flash(`${PASS_LABEL[pass]} failed: ${e}`, "error");
    } finally {
      retrying = { ...retrying, [pass]: false };
      await refresh();
    }
  }

  function retryLabel(pass: PassName, state: PassState | null): string {
    if (pass === "accuracy") return state === "failed" ? "Retry Whisper transcript" : "Add Whisper transcript";
    if (pass === "notes") return state === "failed" ? "Retry notes" : state === "done" ? "Rewrite notes" : "Write notes";
    return "Retry speakers";
  }
  function canRetry(pass: PassName, state: PassState | null): boolean {
    if (!selected || state === "running") return false;
    const needsAudio = pass !== "notes";
    if (needsAudio && selected.remote) return false;
    if (pass === "notes") return true;
    return state !== "done";
  }

  async function copy(kind: "notes" | "speakers" | "whisper" | "all") {
    copyOpen = false;
    if (!selected) return;
    let text = "";
    if (kind === "notes") text = applySpeakerNames(selected.meeting_notes_text || "", selNames);
    else if (kind === "speakers") text = speakersAsText(selected);
    else if (kind === "whisper") text = selWhisper || "";
    else text = await api.conversationMarkdown(selected.id).catch(() => "");
    if (!text.trim()) {
      flash("Nothing to copy yet", "error");
      return;
    }
    await writeText(text);
    flash("Copied");
  }

  async function exportMarkdown() {
    if (!selected) return;
    try {
      const defaultPath = await api.conversationExportName(selected.id);
      const path = await saveDialog({
        defaultPath,
        filters: [{ name: "Markdown", extensions: ["md"] }],
      });
      if (!path) return;
      await api.exportConversation(selected.id, path);
      flash("Exported");
    } catch (e) {
      flash(`Export failed: ${e}`, "error");
    }
  }

  function engineLine(r: Recording): string {
    const doc = conversationDoc(r);
    const parts: string[] = [];
    if (doc?.engine) parts.push(`Speakers: ${doc.engine.provider} ${doc.engine.model}`);
    if (doc?.accuracy?.text) parts.push(`Whisper: ${doc.accuracy.provider} ${doc.accuracy.model}`);
    return parts.join(" · ");
  }
</script>

<svelte:window onclick={() => (copyOpen = false)} onkeydown={(e) => { if (e.key === "Escape") copyOpen = false; }} />

<section class="convos">
  {#if selected}
    <!-- Reader -->
    <header class="reader-head">
      <Button tone="ghost" size="sm" onclick={() => (selectedId = null)} aria-label="Back to all conversations">
        <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><path d="M10 3.5 5.5 8 10 12.5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>
        All conversations
      </Button>
      <div class="reader-actions">
        <Button tone="secondary" size="sm" onclick={() => renameSpeaker(null)} disabled={!selTurns.length}>Name speakers</Button>
        <div class="copy-wrap">
          <Button tone="secondary" size="sm" aria-haspopup="menu" aria-expanded={copyOpen} onclick={(e) => { e.stopPropagation(); copyOpen = !copyOpen; }}>
            Copy
            <svg viewBox="0 0 16 16" width="12" height="12" aria-hidden="true"><path d="M4 6l4 4 4-4" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>
          </Button>
          {#if copyOpen}
            <div class="menu" role="menu" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
              <button role="menuitem" onclick={() => copy("notes")} disabled={!selected.meeting_notes_text}>Notes</button>
              <button role="menuitem" onclick={() => copy("speakers")} disabled={!selTurns.length}>Speakers</button>
              <button role="menuitem" onclick={() => copy("whisper")} disabled={!selWhisper}>Whisper</button>
              <button role="menuitem" onclick={() => copy("all")}>Everything</button>
            </div>
          {/if}
        </div>
        <Button tone="primary" size="sm" onclick={exportMarkdown}>Export</Button>
      </div>
    </header>

    <div class="reader-body">
      <div class="reader-title">
        <h1>{titleOf(selected)}</h1>
        <p class="meta">{dateOf(selected)}{#if lengthOf(selected)} · {lengthOf(selected)}{/if}</p>
        {#if engineLine(selected)}<p class="meta faint">{engineLine(selected)}</p>{/if}
        {#if selTurns.length}
          <div class="chips">
            {#each speakerLabels(selected) as label (label)}
              <button class="chip" onclick={() => renameSpeaker(label)} title="Rename {label}">{namedSpeaker(label, selNames)}</button>
            {/each}
          </div>
        {/if}
      </div>

      <div class="passes" aria-label="Passes">
        {#each selPasses as p (p.pass)}
          <div class="pass" data-state={p.state ?? "none"}>
            <span class="pass-name">{PASS_LABEL[p.pass]}</span>
            <span class="pass-state">
              {#if retrying[p.pass] || p.state === "running"}Working…
              {:else if p.state === "done"}Done
              {:else if p.state === "failed"}Failed
              {:else}Not made{/if}
            </span>
            {#if p.state === "failed" && p.error}<span class="pass-err" title={p.error}>{p.error}</span>{/if}
            {#if canRetry(p.pass, p.state) && !retrying[p.pass] && (p.state !== "done" || p.pass === "notes")}
              <Button tone={p.state === "failed" ? "primary" : "ghost"} size="sm" onclick={() => retry(p.pass)}>{retryLabel(p.pass, p.state)}</Button>
            {/if}
            {#if p.pass !== "notes" && selected.remote && p.state !== "done"}
              <span class="pass-err">Audio is on another device.</span>
            {/if}
          </div>
        {/each}
      </div>

      <Segmented options={viewOptions} bind:value={view} ariaLabel="Conversation view" />

      <article class="view">
        {#if view === "notes"}
          {#if noteBlocks.length}
            <div class="notes">
              {#each noteBlocks as b, i (i)}
                {#if b.kind === "h"}
                  <h3>{#each b.runs as r, j (j)}{#if r.bold}<strong>{r.text}</strong>{:else}{r.text}{/if}{/each}</h3>
                {:else if b.kind === "li"}
                  <p class="li">{#each b.runs as r, j (j)}{#if r.bold}<strong>{r.text}</strong>{:else}{r.text}{/if}{/each}</p>
                {:else}
                  <p>{#each b.runs as r, j (j)}{#if r.bold}<strong>{r.text}</strong>{:else}{r.text}{/if}{/each}</p>
                {/if}
              {/each}
            </div>
          {:else}
            <p class="empty-view">No meeting notes yet.</p>
          {/if}
        {:else if view === "speakers"}
          {#if selTurns.length}
            <ol class="turns">
              {#each selTurns as t, i (i)}
                <li>
                  <span class="ts">{t.start != null ? mmss(t.start) : ""}</span>
                  <div>
                    <button class="who" onclick={() => renameSpeaker(t.speaker)} title="Rename {t.speaker}">{namedSpeaker(t.speaker, selNames)}</button>
                    <p>{t.text}</p>
                  </div>
                </li>
              {/each}
            </ol>
          {:else}
            <p class="empty-view">No speaker transcript yet.</p>
          {/if}
        {:else if selWhisper}
          <div class="whisper">
            {#each selWhisper.split(/\n\s*\n/) as para, i (i)}<p>{para}</p>{/each}
          </div>
        {/if}
      </article>
    </div>

    <SpeakerLabelsDialog bind:open={speakersOpen} rec={selected} focus={focusSpeaker} onSaved={() => void refresh()} />
  {:else}
    <!-- List -->
    <header class="list-head">
      <h1 class="page-title">Conversations</h1>
      <input type="search" class="search" placeholder="Search conversations…" bind:value={search} />
      <Button tone="secondary" onclick={openUpload} title="Upload a recorded conversation">
        <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
          <path d="M8 10V2M5 5l3-3 3 3" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/>
          <path d="M3 10v2.5a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1V10" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
        Upload
      </Button>
    </header>

    {#if loading && list.length === 0}
      <div class="empty"><p class="empty-title">Loading conversations…</p></div>
    {:else if list.length === 0}
      <div class="empty">
        <img class="empty-fox" src="/fox/fox-empty-state.png" alt="" />
        <p class="empty-title">No conversations yet</p>
        <p class="empty-body">Record a meeting on your phone, then drop the file here. You'll get who said what, plus meeting notes.</p>
        <Button tone="primary" onclick={openUpload}>Upload a conversation</Button>
      </div>
    {:else if filtered.length === 0}
      <div class="empty"><p class="empty-title">Nothing matches</p></div>
    {:else}
      <div class="cards">
        {#each filtered as r (r.id)}
          {@const status = statusLine(r)}
          <button class="card" onclick={() => openReader(r)}>
            <span class="card-top">
              <span class="card-title">{titleOf(r)}</span>
              <span class="card-meta">{dateOf(r)}{#if lengthOf(r)} · {lengthOf(r)}{/if}</span>
            </span>
            {#if chips(r).length}
              <span class="chips">
                {#each chips(r) as c, i (i)}<span class="chip static">{c}</span>{/each}
              </span>
            {/if}
            {#if preview(r)}<span class="card-preview">{preview(r)}</span>{/if}
            {#if status}<span class="card-status" data-tone={status.tone}>{status.text}</span>{/if}
          </button>
        {/each}
      </div>
    {/if}
  {/if}

  {#if notice}
    <div class="notice" data-tone={notice.tone} role="status">{notice.text}</div>
  {/if}

  {#if dragActive}
    <div class="drag-overlay" aria-hidden="true">
      <div class="drag-card"><p>Drop to transcribe as a conversation</p></div>
    </div>
  {/if}

  <UploadDialog bind:open={uploadOpen} bind:paths={stagedPaths} kind="conversation" onDone={() => void refresh()} />
</section>

<style>
  .convos {
    container: convos / inline-size;
    position: relative;
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow-y: auto;
    background: var(--bg-surface);
    color: var(--text-primary);
  }
  .list-head,
  .reader-head {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding: 22px 28px 12px;
    flex-shrink: 0;
  }
  .page-title {
    margin: 0;
    font-size: var(--fs-xl);
    font-weight: 700;
    letter-spacing: -0.02em;
    line-height: var(--lh-tight);
  }
  .search {
    flex: 1;
    max-width: 420px;
    margin-left: auto;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    padding: 9px 12px;
    font-size: var(--fs-base);
    background: var(--bg-card);
    color: var(--text-primary);
    outline: none;
  }
  .search:focus { border-color: var(--accent); box-shadow: 0 0 0 3px var(--accent-fade); }

  /* Cards: a different kind of object from a Home dictation card — a left
     accent rule, a speaker chip row and a notes preview. */
  .cards {
    display: grid;
    grid-template-columns: 1fr;
    gap: var(--sp-3);
    padding: 4px 28px 40px;
  }
  @container convos (min-width: 860px) {
    .cards { grid-template-columns: 1fr 1fr; }
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    text-align: left;
    padding: var(--sp-4) var(--sp-4) var(--sp-4) calc(var(--sp-4) + 2px);
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-left: 4px solid var(--accent);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-xs);
    color: inherit;
    font-family: inherit;
    cursor: pointer;
    transition: box-shadow 120ms ease, transform 120ms ease;
  }
  .card:hover { box-shadow: 0 6px 20px rgba(60, 40, 15, 0.12); transform: translateY(-1px); }
  .card:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .card-top { display: flex; flex-direction: column; gap: 2px; }
  .card-title { font-size: var(--fs-md); font-weight: 700; color: var(--text-primary); }
  .card-meta { font-size: var(--fs-xs); color: var(--text-secondary); }
  .card-preview {
    font-size: var(--fs-sm);
    color: var(--text-secondary);
    line-height: 1.45;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .card-status { font-size: var(--fs-xs); font-weight: 600; }
  .card-status[data-tone="busy"] { color: var(--accent); }
  .card-status[data-tone="error"] { color: var(--danger); }

  .chips { display: flex; flex-wrap: wrap; gap: 6px; }
  .chip {
    font-size: var(--fs-xs);
    font-weight: 600;
    padding: 2px 9px;
    border-radius: var(--radius-pill);
    background: var(--accent-fade);
    color: var(--accent);
    border: 1px solid transparent;
    font-family: inherit;
  }
  button.chip { cursor: pointer; }
  button.chip:hover { border-color: var(--accent); }

  /* Reader */
  .reader-actions { display: flex; gap: var(--sp-2); margin-left: auto; align-items: center; }
  .copy-wrap { position: relative; }
  .menu {
    position: absolute;
    right: 0;
    top: calc(100% + 4px);
    z-index: 30;
    min-width: 150px;
    display: flex;
    flex-direction: column;
    padding: 4px;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    box-shadow: 0 10px 30px rgba(40, 25, 10, 0.18);
  }
  .menu button {
    text-align: left;
    background: transparent;
    border: 0;
    padding: 7px 10px;
    border-radius: var(--radius-sm);
    font: inherit;
    font-size: var(--fs-base);
    color: var(--text-primary);
    cursor: pointer;
  }
  .menu button:hover:not(:disabled) { background: var(--bg-subtle); }
  .menu button:disabled { opacity: 0.45; cursor: not-allowed; }

  .reader-body {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    padding: 0 28px 48px;
    max-width: 920px;
    width: 100%;
    box-sizing: border-box;
  }
  .reader-title h1 { margin: 0; font-size: var(--fs-xl); font-weight: 700; letter-spacing: -0.02em; }
  .meta { margin: 4px 0 0; font-size: var(--fs-sm); color: var(--text-secondary); }
  .meta.faint { font-size: var(--fs-xs); opacity: 0.85; }
  .reader-title .chips { margin-top: var(--sp-2); }

  .passes {
    display: grid;
    grid-template-columns: 1fr;
    gap: var(--sp-2);
  }
  @container convos (min-width: 760px) {
    .passes { grid-template-columns: repeat(3, 1fr); }
  }
  .pass {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
    padding: var(--sp-3);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg-card);
  }
  .pass[data-state="failed"] { border-color: var(--danger); background: var(--danger-fade); }
  .pass-name { font-size: var(--fs-xs); font-weight: 700; text-transform: uppercase; letter-spacing: 0.05em; color: var(--text-secondary); }
  .pass-state { font-size: var(--fs-base); font-weight: 600; }
  .pass[data-state="done"] .pass-state { color: var(--text-primary); }
  .pass[data-state="running"] .pass-state { color: var(--accent); }
  .pass[data-state="failed"] .pass-state { color: var(--danger); }
  .pass-err {
    font-size: var(--fs-xs);
    color: var(--text-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }

  .view {
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: var(--sp-5);
    font-size: var(--fs-base);
    line-height: 1.6;
  }
  .notes h3 { margin: var(--sp-4) 0 var(--sp-1); font-size: var(--fs-md); }
  .notes h3:first-child { margin-top: 0; }
  .notes p { margin: 0 0 var(--sp-2); }
  .notes p.li { padding-left: 16px; position: relative; }
  .notes p.li::before { content: "•"; position: absolute; left: 2px; color: var(--accent); }
  .turns { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: var(--sp-3); }
  .turns li { display: grid; grid-template-columns: 48px 1fr; gap: var(--sp-2); }
  .ts { font-size: var(--fs-xs); color: var(--text-secondary); font-variant-numeric: tabular-nums; padding-top: 3px; }
  .who {
    background: transparent;
    border: 0;
    padding: 0;
    font: inherit;
    font-weight: 700;
    color: var(--accent);
    cursor: pointer;
  }
  .who:hover { text-decoration: underline; }
  .turns p { margin: 2px 0 0; }
  .whisper p { margin: 0 0 var(--sp-3); }
  .empty-view { color: var(--text-secondary); margin: 0; }

  .empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--sp-2);
    padding: 40px;
    text-align: center;
    color: var(--text-secondary);
  }
  .empty-fox { width: 160px; height: 160px; object-fit: contain; }
  .empty-title { margin: 0; font-size: 17px; font-weight: 600; color: var(--text-primary); }
  .empty-body { margin: 0 0 var(--sp-2); max-width: 360px; font-size: var(--fs-base); line-height: 1.5; }

  .notice {
    position: fixed;
    bottom: 22px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 300;
    padding: 8px 14px;
    border-radius: var(--radius-md);
    background: var(--bg-card);
    border: 1px solid var(--border);
    box-shadow: 0 10px 30px rgba(40, 25, 10, 0.2);
    font-size: var(--fs-sm);
    max-width: min(560px, 90vw);
  }
  .notice[data-tone="error"] { border-color: var(--danger); color: var(--danger); }

  .drag-overlay {
    position: absolute;
    inset: 0;
    z-index: 150;
    display: flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in srgb, var(--bg-surface) 75%, transparent);
    backdrop-filter: blur(2px);
    pointer-events: none;
  }
  .drag-card {
    padding: 32px 44px;
    border: 2px dashed var(--accent);
    border-radius: 18px;
    background: var(--bg-card);
  }
  .drag-card p { margin: 0; font-size: 15px; font-weight: 700; }

  @container convos (max-width: 560px) {
    .list-head, .reader-head { padding: 18px 16px 10px; flex-wrap: wrap; }
    .cards, .reader-body { padding-inline: 16px; }
  }
</style>
