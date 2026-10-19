<script lang="ts">
  import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
  import { listen } from "@tauri-apps/api/event";
  import { goto } from "$app/navigation";
  import { api, type SecretCheck } from "$lib/api";
  import { history } from "$lib/history-store.svelte";
  import { settings } from "$lib/settings-store.svelte";
  import Disclosure from "$lib/ui/Disclosure.svelte";
  import {
    STT_PROVIDERS,
    LLM_PROVIDERS,
    sttModelsFor,
    llmModelsFor,
    sttReady,
    llmReady,
  } from "$lib/provider-options";

  type Kind = "voice_note" | "conversation";

  // `open` toggles the modal; `paths` is the staged file list, shared with the
  // parent so a window-wide drag-drop can push files straight in. Both
  // bindable. `kind` forces the first choice (the Conversations page passes
  // "conversation" for a drop); null = the last answer the user gave.
  let {
    open = $bindable(false),
    paths = $bindable<string[]>([]),
    kind: forcedKind = null,
    onDone,
  } = $props<{
    open?: boolean;
    paths?: string[];
    kind?: Kind | null;
    onDone?: () => void;
  }>();

  const AUDIO_EXTS = ["wav", "mp3", "m4a", "aac", "ogg", "oga", "opus", "flac", "webm", "mp4"];

  function savedKind(): Kind {
    return settings.s.upload_kind === "conversation" ? "conversation" : "voice_note";
  }
  let kind = $state<Kind>(savedKind());

  function chooseKind(k: Kind) {
    if (running) return;
    kind = k;
    if (settings.s.upload_kind !== k) void settings.set("upload_kind", k);
  }

  // ── Voice note engine + options. Seeded from the global picks; the dialog
  // can override them for this batch without touching saved settings.
  let sttProvider = $state(settings.s.stt_provider || "groq");
  let sttModel = $state(settings.s.stt_model || sttModelsFor(settings.s.stt_provider || "groq")[0].id);
  let llmProvider = $state(settings.s.llm_provider || "groq");
  let llmModel = $state(settings.s.llm_model || llmModelsFor(settings.s.llm_provider || "groq")[0].id);
  let draftLlmProvider = $state(settings.s.draft_llm_provider || "groq");
  let draftLlmModel = $state(settings.s.draft_llm_model || llmModelsFor(settings.s.draft_llm_provider || "groq")[0].id);
  let cleanup = $state(false);
  let draft = $state(false);

  // ── Conversation engine + options.
  const SPEAKER_ENGINES = [
    { provider: "deepgram", model: "nova-3", label: "Deepgram Nova-3", note: "Recommended. Speaker labels are a small add-on charge (~$0.002/min)." },
    { provider: "elevenlabs", model: "scribe_v2", label: "ElevenLabs Scribe v2", note: "Speaker labels included at no extra charge." },
    { provider: "openai", model: "gpt-4o-transcribe-diarize", label: "OpenAI GPT-4o Diarize", note: "Files over ~20 MB are refused; use Deepgram for long meetings." },
  ];
  let speakerEngine = $state("deepgram");
  const speakerChoice = $derived(SPEAKER_ENGINES.find((e) => e.provider === speakerEngine) ?? SPEAKER_ENGINES[0]);
  let accuracy = $state(false);
  let notes = $state(false);
  let optionsOpen = $state(false);

  let secrets = $state<SecretCheck | null>(null);

  // Per-file progress: keyed by path → "queued" | "working" | "done" | "error".
  type FileState = "queued" | "working" | "done" | "error";
  let statuses = $state<Record<string, FileState>>({});
  let errors = $state<Record<string, string>>({});
  let running = $state(false);
  let finished = $state(false);
  let finishedKind = $state<Kind>("voice_note");

  // Reset when the dialog OPENS (closed → open transition only). Re-running
  // on every tracked change once wiped the "N transcribed / Close" state the
  // moment a batch finished, so keep this edge-triggered.
  let wasOpen = false;
  $effect(() => {
    if (open && !wasOpen) {
      kind = forcedKind ?? savedKind();
      statuses = {};
      errors = {};
      finished = false;
      stageText = "";
      optionsOpen = false;
      api
        .checkSecrets()
        .then((s) => {
          secrets = s;
          // Defaults per the contract: Whisper on when a Groq key exists,
          // notes on when the Draft engine has a key.
          accuracy = Boolean(s.stt);
          notes = llmReady(s, draftLlmProvider);
        })
        .catch(() => {});
    }
    wasOpen = open;
  });

  // Live pipeline stage from the backend, shown on the in-flight file row.
  // Files run one at a time, so a single latest-stage string is unambiguous.
  let stageText = $state("");
  $effect(() => {
    const un = listen<{ id: string; stage: string }>("wispr:job_stage", (e) => {
      stageText = e.payload.stage;
    });
    return () => {
      un.then((f) => f());
    };
  });

  const sttModels = $derived(sttModelsFor(sttProvider));
  const llmModels = $derived(llmModelsFor(llmProvider));
  const draftLlmModels = $derived(llmModelsFor(draftLlmProvider));
  const sttHasKey = $derived(sttReady(secrets, sttProvider));
  const llmHasKey = $derived(!cleanup || llmReady(secrets, llmProvider));
  const draftLlmHasKey = $derived(!draft || llmReady(secrets, draftLlmProvider));
  const speakersHasKey = $derived(sttReady(secrets, speakerChoice.provider));
  const groqHasKey = $derived(!secrets || secrets.stt);
  const notesHasKey = $derived(!notes || llmReady(secrets, draftLlmProvider));

  const canTranscribe = $derived(
    paths.length > 0 &&
      !running &&
      (kind === "voice_note"
        ? sttHasKey && llmHasKey && draftLlmHasKey
        : speakersHasKey && (!accuracy || groqHasKey) && notesHasKey),
  );

  // Keep the selected models valid when the provider changes.
  $effect(() => {
    if (!sttModels.some((m) => m.id === sttModel)) sttModel = sttModels[0].id;
  });
  $effect(() => {
    if (!llmModels.some((m) => m.id === llmModel)) llmModel = llmModels[0].id;
  });
  $effect(() => {
    if (!draftLlmModels.some((m) => m.id === draftLlmModel)) draftLlmModel = draftLlmModels[0].id;
  });

  function baseName(p: string): string {
    return p.split(/[\\/]/).pop() || p;
  }

  function addPaths(incoming: string[]) {
    const audio = incoming.filter((p) => {
      const ext = (p.split(".").pop() || "").toLowerCase();
      return AUDIO_EXTS.includes(ext);
    });
    const merged = new Set([...paths, ...audio]);
    paths = [...merged];
  }

  async function pickFiles() {
    const picked = await openFileDialog({
      multiple: true,
      filters: [{ name: "Audio", extensions: AUDIO_EXTS }],
    });
    if (!picked) return;
    addPaths(Array.isArray(picked) ? picked : [picked]);
  }

  function removePath(p: string) {
    paths = paths.filter((x: string) => x !== p);
  }

  function close() {
    if (running) return;
    open = false;
    paths = [];
  }

  async function runOne(p: string) {
    if (kind === "conversation") {
      await api.transcribeConversation(p, {
        speakersProvider: speakerChoice.provider,
        speakersModel: speakerChoice.model,
        accuracy,
        notes,
        notesLlmProvider: notes ? draftLlmProvider : null,
        notesLlmModel: notes ? draftLlmModel : null,
      });
    } else {
      // A voice note is one speaker: no speaker labels, no meeting notes, so
      // it lands on Home, never on Conversations.
      await api.transcribeUpload(p, {
        sttProvider,
        sttModel,
        llmProvider: cleanup ? llmProvider : null,
        llmModel: cleanup ? llmModel : null,
        draftLlmProvider: draft ? draftLlmProvider : null,
        draftLlmModel: draft ? draftLlmModel : null,
        cleanup,
        draft,
        diarize: false,
        meetingNotes: false,
      });
    }
  }

  async function transcribeAll() {
    if (!canTranscribe) return;
    running = true;
    finished = false;
    finishedKind = kind;
    // Snapshot the queue so removing rows mid-run can't desync the loop.
    const queue = [...paths];
    for (const p of queue) statuses[p] = "queued";
    statuses = { ...statuses };

    for (const p of queue) {
      statuses[p] = "working";
      statuses = { ...statuses };
      stageText = "Starting…";
      try {
        await runOne(p);
        statuses[p] = "done";
      } catch (e) {
        statuses[p] = "error";
        errors[p] = String(e);
      }
      statuses = { ...statuses };
      errors = { ...errors };
      await history.refresh();
      onDone?.();
    }

    running = false;
    finished = true;
    stageText = "";
  }

  async function openDestination() {
    const dest = finishedKind === "conversation" ? "/conversations" : "/history";
    close();
    await goto(dest);
  }

  const doneCount = $derived(Object.values(statuses).filter((s) => s === "done").length);
  const errCount = $derived(Object.values(statuses).filter((s) => s === "error").length);
</script>

{#if open}
  <!-- Backdrop. Clicking it closes when idle. -->
  <div
    class="backdrop"
    role="button"
    tabindex="-1"
    aria-label="Close upload dialog"
    onclick={close}
    onkeydown={(e) => { if (e.key === "Escape") close(); }}
  ></div>

  <div class="modal" role="dialog" aria-modal="true" aria-label="Upload audio to transcribe">
    <header class="modal-head">
      <div>
        <h2>Upload audio</h2>
        <p class="sub">What kind of recording is it?</p>
      </div>
      <button class="x" onclick={close} disabled={running} aria-label="Close">×</button>
    </header>

    <!-- The one question every upload asks first. -->
    <div class="kind-row" role="radiogroup" aria-label="Recording type">
      <button
        type="button"
        role="radio"
        aria-checked={kind === "voice_note"}
        class="kind"
        class:active={kind === "voice_note"}
        disabled={running}
        onclick={() => chooseKind("voice_note")}
      >
        <svg viewBox="0 0 24 24" width="24" height="24" aria-hidden="true">
          <rect x="9" y="3" width="6" height="11" rx="3" fill="none" stroke="currentColor" stroke-width="1.7"/>
          <path d="M5.5 11a6.5 6.5 0 0 0 13 0M12 17.5V21" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/>
        </svg>
        <span class="kind-title">Voice note</span>
        <span class="kind-sub">One speaker. Lands on Home.</span>
      </button>
      <button
        type="button"
        role="radio"
        aria-checked={kind === "conversation"}
        class="kind"
        class:active={kind === "conversation"}
        disabled={running}
        onclick={() => chooseKind("conversation")}
      >
        <svg viewBox="0 0 24 24" width="24" height="24" aria-hidden="true">
          <circle cx="8" cy="8" r="3" fill="none" stroke="currentColor" stroke-width="1.7"/>
          <circle cx="16.5" cy="9.5" r="2.5" fill="none" stroke="currentColor" stroke-width="1.7"/>
          <path d="M2.5 19c.6-3 3-4.8 5.5-4.8s4.9 1.8 5.5 4.8M13.8 15.2c.8-.6 1.7-.9 2.7-.9 2.1 0 4 1.5 4.5 4.2" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/>
        </svg>
        <span class="kind-title">Conversation</span>
        <span class="kind-sub">Several people. Who said what, plus notes.</span>
      </button>
    </div>

    <!-- Drop / pick zone. Files can be dragged anywhere onto the window, or
         picked here. The list shows what's staged. -->
    <div class="dropzone" class:filled={paths.length > 0}>
      {#if paths.length === 0}
        <svg class="drop-icon" viewBox="0 0 24 24" width="34" height="34" aria-hidden="true">
          <path d="M12 15V4M8 8l4-4 4 4" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>
          <path d="M4 15v3a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-3" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
        <p class="drop-title">Drag audio files here</p>
        <p class="drop-hint">or</p>
        <button class="choose-btn" onclick={pickFiles}>Choose from computer</button>
        <p class="formats">wav · mp3 · m4a · aac · ogg · opus · flac · webm</p>
      {:else}
        <ul class="file-list">
          {#each paths as p (p)}
            <li class="file-row" class:err={statuses[p] === "error"}>
              <svg class="file-ic" viewBox="0 0 16 16" width="15" height="15" aria-hidden="true">
                <path d="M4 2h5l3 3v9a0 0 0 0 1 0 0H4a0 0 0 0 1 0 0V2z" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/>
                <path d="M9 2v3h3" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/>
              </svg>
              <span class="file-name" title={p}>{baseName(p)}</span>
              {#if statuses[p] === "working"}
                <span class="fstate working">{stageText || "Working…"}</span>
              {:else if statuses[p] === "queued"}
                <span class="fstate queued">Queued</span>
              {:else if statuses[p] === "done"}
                <span class="fstate done">✓ Done</span>
              {:else if statuses[p] === "error"}
                <span class="fstate error" title={errors[p]}>Failed</span>
              {:else if !running}
                <button class="rm" onclick={() => removePath(p)} aria-label="Remove">×</button>
              {/if}
            </li>
          {/each}
        </ul>
        {#if !running}
          <button class="add-more" onclick={pickFiles}>+ Add more</button>
        {/if}
      {/if}
    </div>

    <div class="options">
      {#if kind === "conversation"}
        <div class="checks">
          <label class="check">
            <input type="checkbox" bind:checked={accuracy} disabled={running} />
            <span>
              <strong>Also make a Whisper transcript</strong> — more accurate wording, no speakers.
              {#if accuracy && !groqHasKey}<em class="why warn">Needs a Groq key (Settings → Providers).</em>{/if}
            </span>
          </label>
          <label class="check">
            <input type="checkbox" bind:checked={notes} disabled={running} />
            <span>
              <strong>Write meeting notes</strong> — summary, decisions and action items.
              {#if notes && !notesHasKey}<em class="why warn">Needs a {draftLlmProvider} key (Settings → Providers).</em>{/if}
            </span>
          </label>
        </div>
        {#if !speakersHasKey}
          <p class="keywarn">No API key saved for {speakerChoice.label} — add one in Settings → Providers, or pick another engine under Options.</p>
        {/if}
        <Disclosure label="Options" hint={`${speakerChoice.label}${notes ? ` · notes with ${draftLlmProvider}` : ""}`} bind:open={optionsOpen}>
          <div class="opt-stack">
            <div class="opt-row">
              <label class="opt-label" for="speaker-engine">Label speakers with</label>
              <select id="speaker-engine" bind:value={speakerEngine} disabled={running}>
                {#each SPEAKER_ENGINES as e (e.provider)}
                  <option value={e.provider}>{e.label}</option>
                {/each}
              </select>
              <em class="why">{speakerChoice.note} The whole file goes in one request so speaker labels stay consistent.</em>
            </div>
            {#if accuracy}
              <div class="opt-row">
                <span class="opt-label">Whisper transcript</span>
                <em class="why">Groq Whisper Large v3.</em>
              </div>
            {/if}
            {#if notes}
              <div class="opt-row">
                <label class="opt-label" for="notes-llm-prov">Meeting notes with</label>
                <div class="selects">
                  <select id="notes-llm-prov" bind:value={draftLlmProvider} disabled={running}>
                    {#each LLM_PROVIDERS as p}<option value={p.id}>{p.label}</option>{/each}
                  </select>
                  <select bind:value={draftLlmModel} disabled={running}>
                    {#each draftLlmModels as m}<option value={m.id}>{m.label}</option>{/each}
                  </select>
                </div>
              </div>
            {/if}
          </div>
        </Disclosure>
      {:else}
        <div class="checks">
          <label class="check">
            <input type="checkbox" bind:checked={cleanup} disabled={running} />
            <span><strong>Clean up</strong> — fix punctuation & paragraphs, keep my words</span>
          </label>
        </div>
        {#if !sttHasKey}
          <p class="keywarn">No API key saved for {sttProvider} — add one in Settings → Providers first.</p>
        {/if}
        {#if !llmHasKey}
          <p class="keywarn">No API key saved for {llmProvider} — add one in Settings → Providers first.</p>
        {/if}
        {#if !draftLlmHasKey}<p class="keywarn">Add a {draftLlmProvider} key in Settings before running Draft.</p>{/if}
        <Disclosure label="Options" hint={`${sttProvider} · ${sttModel}`} bind:open={optionsOpen}>
          <div class="opt-stack">
            <div class="opt-row">
              <label class="opt-label" for="stt-prov">Transcribe with</label>
              <div class="selects">
                <select id="stt-prov" bind:value={sttProvider} disabled={running}>
                  {#each STT_PROVIDERS as p}
                    <option value={p.id}>{p.label}</option>
                  {/each}
                </select>
                <select bind:value={sttModel} disabled={running}>
                  {#each sttModels as m}
                    <option value={m.id}>{m.label}</option>
                  {/each}
                </select>
              </div>
            </div>
            {#if cleanup}
              <div class="opt-row">
                <label class="opt-label" for="llm-prov">Clean up with</label>
                <div class="selects">
                  <select id="llm-prov" bind:value={llmProvider} disabled={running}>
                    {#each LLM_PROVIDERS as p}
                      <option value={p.id}>{p.label}</option>
                    {/each}
                  </select>
                  <select bind:value={llmModel} disabled={running}>
                    {#each llmModels as m}
                      <option value={m.id}>{m.label}</option>
                    {/each}
                  </select>
                </div>
              </div>
            {/if}
            <div class="checks">
              <label class="check">
                <input type="checkbox" bind:checked={draft} disabled={running} />
                <span><strong>Draft</strong> — also write a polished, structured version</span>
              </label>
            </div>
            {#if draft}
              <div class="opt-row">
                <label class="opt-label" for="draft-llm-prov">Draft with</label>
                <div class="selects">
                  <select id="draft-llm-prov" bind:value={draftLlmProvider} disabled={running}>
                    {#each LLM_PROVIDERS as p}<option value={p.id}>{p.label}</option>{/each}
                  </select>
                  <select bind:value={draftLlmModel} disabled={running}>
                    {#each draftLlmModels as m}<option value={m.id}>{m.label}</option>{/each}
                  </select>
                </div>
              </div>
            {/if}
          </div>
        </Disclosure>
      {/if}
    </div>

    <footer class="modal-foot">
      {#if finished}
        <span class="result-msg">
          {doneCount} transcribed{#if errCount > 0}, {errCount} failed{/if}.
          {finishedKind === "conversation" ? "Added to Conversations." : "Added to Home."}
        </span>
      {:else if running}
        <span class="hint-msg">{stageText || "Working…"} ({doneCount + errCount}/{paths.length} done)</span>
      {:else}
        <span class="hint-msg">
          {paths.length === 0 ? "Add at least one audio file." : `${paths.length} file${paths.length === 1 ? "" : "s"} ready.`}
        </span>
      {/if}
      <div class="foot-btns">
        {#if finished && doneCount > 0}
          <button class="ghost" onclick={openDestination}>
            {finishedKind === "conversation" ? "Open Conversations" : "Open Home"}
          </button>
        {/if}
        <button class="ghost" onclick={close} disabled={running}>{finished ? "Close" : "Cancel"}</button>
        {#if !finished}
          <button class="primary" onclick={transcribeAll} disabled={!canTranscribe}>
            {running ? "Transcribing…" : `Transcribe ${paths.length || ""}`.trim()}
          </button>
        {/if}
      </div>
    </footer>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(30, 22, 12, 0.42);
    backdrop-filter: blur(2px);
    z-index: 200;
    border: none;
  }
  .modal {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    z-index: 201;
    width: min(560px, calc(100vw - 40px));
    max-height: calc(100vh - 48px);
    overflow-y: auto;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 16px;
    box-shadow: 0 24px 70px rgba(60, 40, 15, 0.35);
    padding: 20px 22px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .modal-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
  }
  h2 {
    font-size: 18px;
    font-weight: 700;
    color: var(--text-primary);
    margin: 0;
  }
  .sub {
    font-size: 12.5px;
    color: var(--text-secondary);
    margin: 3px 0 0;
    line-height: 1.4;
  }
  .x {
    background: transparent;
    border: none;
    color: var(--text-secondary);
    font-size: 22px;
    line-height: 1;
    cursor: pointer;
    padding: 2px 6px;
    border-radius: 6px;
  }
  .x:hover:not(:disabled) { background: var(--bg-subtle); color: var(--text-primary); }
  .x:disabled { opacity: 0.4; cursor: not-allowed; }

  .dropzone {
    border: 1.5px dashed var(--border);
    border-radius: 12px;
    background: var(--bg-subtle);
    padding: 22px 18px;
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 6px;
  }
  .dropzone.filled {
    align-items: stretch;
    text-align: left;
    background: var(--bg-card);
    border-style: solid;
    padding: 12px;
  }
  .drop-icon { color: var(--accent); margin-bottom: 2px; }
  .drop-title { font-size: 14px; font-weight: 600; color: var(--text-primary); margin: 0; }
  .drop-hint { font-size: 11px; color: var(--text-secondary); margin: 0; }
  .formats { font-size: 10.5px; color: var(--text-secondary); margin: 6px 0 0; letter-spacing: 0.02em; }
  .choose-btn {
    background: var(--accent);
    color: #fff;
    border: none;
    border-radius: 9px;
    padding: 8px 16px;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    font-family: inherit;
  }
  .choose-btn:hover { background: var(--accent-hover, var(--accent)); }

  .file-list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 4px; }
  .file-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 9px;
    border-radius: 8px;
    background: var(--bg-subtle);
    font-size: 12.5px;
    color: var(--text-primary);
  }
  .file-row.err { background: var(--danger-fade); }
  .file-ic { color: var(--text-secondary); flex-shrink: 0; }
  .file-name { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; }
  .fstate { font-size: 11px; font-weight: 600; flex-shrink: 0; }
  .fstate.working { color: var(--accent); }
  .fstate.queued { color: var(--text-secondary); }
  .fstate.done { color: #2e9e5b; }
  .fstate.error { color: var(--danger); }
  .rm {
    background: transparent; border: none; color: var(--text-secondary);
    font-size: 16px; line-height: 1; cursor: pointer; padding: 0 4px; border-radius: 4px; flex-shrink: 0;
  }
  .rm:hover { color: var(--danger); }
  .add-more {
    align-self: flex-start;
    margin-top: 8px;
    background: transparent;
    border: 1px dashed var(--border);
    color: var(--text-secondary);
    border-radius: 8px;
    padding: 5px 11px;
    font-size: 12px;
    cursor: pointer;
    font-family: inherit;
  }
  .add-more:hover { border-color: var(--accent); color: var(--accent); }

  .options { display: flex; flex-direction: column; gap: 12px; }
  .opt-row { display: flex; flex-direction: column; gap: 6px; }
  .opt-label { font-size: 11px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em; color: var(--text-secondary); }
  .selects { display: flex; gap: 8px; }
  select {
    flex: 1;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 9px;
    padding: 8px 10px;
    font-size: 13px;
    color: var(--text-primary);
    font-family: inherit;
    cursor: pointer;
  }
  select:focus { outline: none; border-color: var(--accent); box-shadow: 0 0 0 3px var(--accent-fade); }
  select:disabled { opacity: 0.55; cursor: not-allowed; }

  .checks { display: flex; flex-direction: column; gap: 8px; }
  .check {
    display: flex;
    align-items: flex-start;
    gap: 9px;
    font-size: 12.5px;
    color: var(--text-secondary);
    line-height: 1.4;
    cursor: pointer;
  }
  .check strong { color: var(--text-primary); font-weight: 600; }
  .check input { margin-top: 2px; accent-color: var(--accent); width: 15px; height: 15px; cursor: pointer; }
  .why {
    display: block;
    margin-top: 2px;
    font-style: normal;
    font-size: 11.5px;
    color: var(--text-secondary);
    opacity: 0.9;
  }

  .keywarn {
    font-size: 11.5px;
    color: var(--danger);
    margin: 0;
    background: var(--danger-fade);
    padding: 6px 9px;
    border-radius: 7px;
  }

  .modal-foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    border-top: 1px solid var(--border-subtle);
    padding-top: 14px;
  }
  .hint-msg, .result-msg { font-size: 12px; color: var(--text-secondary); }
  .result-msg { color: var(--text-primary); font-weight: 500; }
  .foot-btns { display: flex; gap: 8px; flex-shrink: 0; }
  .ghost {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--text-primary);
    border-radius: 9px;
    padding: 8px 16px;
    font-size: 13px;
    cursor: pointer;
    font-family: inherit;
  }
  .ghost:hover:not(:disabled) { background: var(--bg-subtle); }
  .ghost:disabled { opacity: 0.5; cursor: not-allowed; }
  .primary {
    background: var(--accent);
    color: #fff;
    border: none;
    border-radius: 9px;
    padding: 8px 18px;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    font-family: inherit;
  }
  .primary:hover:not(:disabled) { background: var(--accent-hover, var(--accent)); }
  .primary:disabled { opacity: 0.5; cursor: not-allowed; }

  .kind-row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--sp-3);
  }
  .kind {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--sp-1);
    padding: var(--sp-4);
    border: 1.5px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg-card);
    color: var(--text-secondary);
    cursor: pointer;
    font-family: inherit;
    text-align: left;
    transition: border-color 120ms ease, background 120ms ease;
  }
  .kind:hover:not(:disabled) { border-color: var(--accent); }
  .kind.active {
    border-color: var(--accent);
    background: var(--accent-fade);
    color: var(--accent);
  }
  .kind:disabled { cursor: not-allowed; opacity: 0.7; }
  .kind-title { font-size: var(--fs-md); font-weight: 700; color: var(--text-primary); }
  .kind-sub { font-size: var(--fs-sm); color: var(--text-secondary); line-height: 1.35; }
  .opt-stack { display: flex; flex-direction: column; gap: var(--sp-3); padding-top: var(--sp-2); }
  .why.warn { color: var(--danger); }
</style>
