<script lang="ts">
  import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
  import { api, type SecretCheck } from "$lib/api";
  import { history } from "$lib/history-store.svelte";
  import { settings } from "$lib/settings-store.svelte";
  import {
    STT_PROVIDERS,
    LLM_PROVIDERS,
    sttModelsFor,
    llmModelsFor,
    sttReady,
    llmReady,
  } from "$lib/provider-options";

  // `open` toggles the modal; `paths` is the staged file list, shared with the
  // parent so a window-wide drag-drop can push files straight in. Both bindable.
  let {
    open = $bindable(false),
    paths = $bindable<string[]>([]),
  } = $props<{ open?: boolean; paths?: string[] }>();

  const AUDIO_EXTS = ["wav", "mp3", "m4a", "aac", "ogg", "oga", "opus", "flac", "webm", "mp4"];

  // Engine + options. Seed from the user's current global picks; the dialog can
  // override them just for this batch without touching the saved settings.
  let sttProvider = $state(settings.s.stt_provider || "groq");
  let sttModel = $state(settings.s.stt_model || sttModelsFor(settings.s.stt_provider || "groq")[0].id);
  let llmProvider = $state(settings.s.llm_provider || "groq");
  let llmModel = $state(settings.s.llm_model || llmModelsFor(settings.s.llm_provider || "groq")[0].id);
  let draftLlmProvider = $state(settings.s.draft_llm_provider || "groq");
  let draftLlmModel = $state(settings.s.draft_llm_model || llmModelsFor(settings.s.draft_llm_provider || "groq")[0].id);
  let cleanup = $state(false);
  let draft = $state(false);
  let diarize = $state(false);
  let meetingNotes = $state(false);

  let secrets = $state<SecretCheck | null>(null);

  // Only Deepgram and ElevenLabs have a speaker model. Whisper (Groq/OpenAI)
  // has none at all, so the checkbox is disabled with the reason rather than
  // accepted and silently ignored — which would hand back an unlabelled wall
  // of text and no clue why. Rust enforces the same rule server-side.
  const canDiarize = $derived(
    sttProvider === "deepgram" || sttProvider === "elevenlabs" ||
      (sttProvider === "openai" && sttModel === "gpt-4o-transcribe-diarize"),
  );
  let diarizeSwitchNote = $state("");

  // Diarization is priced differently by provider and this is a real cost
  // decision for long meeting audio, so say it plainly at the point of choice.
  const diarizeCostNote = $derived(
    sttProvider === "elevenlabs"
      ? "Included free with Scribe — no extra charge."
      : sttProvider === "deepgram"
        ? "Deepgram bills speaker labels as an add-on (~$0.002/min on top of the model rate)."
        : "",
  );

  // Per-file progress: keyed by path → "queued" | "working" | "done" | "error".
  type FileState = "queued" | "working" | "done" | "error";
  let statuses = $state<Record<string, FileState>>({});
  let errors = $state<Record<string, string>>({});
  let running = $state(false);
  let finished = $state(false);

  // Re-seed engine + reset progress every time the dialog opens.
  $effect(() => {
    if (open) {
      api.checkSecrets().then((s) => (secrets = s)).catch(() => {});
      if (!running) {
        statuses = {};
        errors = {};
        finished = false;
      }
    }
  });

  const sttModels = $derived(sttModelsFor(sttProvider));
  const llmModels = $derived(llmModelsFor(llmProvider));
  const draftLlmModels = $derived(llmModelsFor(draftLlmProvider));
  const needsLlm = $derived(cleanup || draft || meetingNotes);
  const sttHasKey = $derived(sttReady(secrets, sttProvider));
  const llmHasKey = $derived(!cleanup || llmReady(secrets, llmProvider));
  const draftLlmHasKey = $derived(!(draft || meetingNotes) || llmReady(secrets, draftLlmProvider));
  const canTranscribe = $derived(paths.length > 0 && sttHasKey && llmHasKey && draftLlmHasKey && !running);

  // Keep the selected model valid when the provider changes.
  $effect(() => {
    if (!sttModels.some((m) => m.id === sttModel)) sttModel = sttModels[0].id;
  });
  function toggleDiarize(on: boolean) {
    diarize = on;
    diarizeSwitchNote = "";
    if (!on) return;
    if (sttProvider === "openai") {
      if (sttModel !== "gpt-4o-transcribe-diarize") {
        sttModel = "gpt-4o-transcribe-diarize";
        diarizeSwitchNote = "Changed to OpenAI GPT-4o Diarize because the selected model has no speaker labels.";
      }
      return;
    }
    if (sttProvider !== "deepgram" && sttProvider !== "elevenlabs") {
      sttProvider = "deepgram";
      sttModel = "nova-3";
      diarizeSwitchNote = "Changed to Deepgram Nova-3 because Whisper does not support speaker labels.";
    }
  }
  $effect(() => {
    if (!llmModels.some((m) => m.id === llmModel)) llmModel = llmModels[0].id;
  });
  $effect(() => {
    if (!draftLlmModels.some((m) => m.id === draftLlmModel)) draftLlmModel = draftLlmModels[0].id;
  });
  $effect(() => {
    if (diarize && !canDiarize) toggleDiarize(true);
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

  async function transcribeAll() {
    if (!canTranscribe) return;
    running = true;
    finished = false;
    // Snapshot the queue so removing rows mid-run can't desync the loop.
    const queue = [...paths];
    for (const p of queue) statuses[p] = "queued";
    statuses = { ...statuses };

    for (const p of queue) {
      statuses[p] = "working";
      statuses = { ...statuses };
      try {
        await api.transcribeUpload(p, {
          sttProvider,
          sttModel,
          llmProvider: needsLlm ? llmProvider : null,
          llmModel: needsLlm ? llmModel : null,
          draftLlmProvider: draft || meetingNotes ? draftLlmProvider : null,
          draftLlmModel: draft || meetingNotes ? draftLlmModel : null,
          cleanup,
          draft,
          diarize,
          meetingNotes,
        });
        statuses[p] = "done";
      } catch (e) {
        statuses[p] = "error";
        errors[p] = String(e);
      }
      statuses = { ...statuses };
      errors = { ...errors };
      await history.refresh();
    }

    running = false;
    finished = true;
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
        <p class="sub">Transcribe voice notes or recordings from your computer or phone.</p>
      </div>
      <button class="x" onclick={close} disabled={running} aria-label="Close">×</button>
    </header>

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
                <span class="fstate working">Transcribing…</span>
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

    <!-- Engine + processing options. -->
    <div class="options">
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
      {#if !sttHasKey}
        <p class="keywarn">No API key saved for {sttProvider} — add one in Settings → Providers first.</p>
      {/if}

      <!-- Speaker labels. Sits directly under the engine picker because it's a
           property of the transcription request, not a post-processing step. -->
      <div class="checks">
        <label class="check">
          <input
            type="checkbox"
            checked={diarize}
            onchange={(e) => toggleDiarize((e.currentTarget as HTMLInputElement).checked)}
            disabled={running}
          />
          <span>
            <strong>Label speakers</strong> — split the transcript into "Speaker 1 / Speaker 2"
            turns. For meetings and interviews.
            {#if diarizeSwitchNote}
              <em class="why switch-note">{diarizeSwitchNote}</em>
            {:else if !canDiarize}
              <em class="why">Not available on {sttProvider === "groq" ? "Groq" : "OpenAI"} — Whisper has no speaker model. Switch to Deepgram or ElevenLabs above.</em>
            {:else if diarizeCostNote}
              <em class="why">{diarizeCostNote}</em>
            {/if}
          </span>
        </label>
      </div>

      <div class="checks">
        <label class="check">
          <input type="checkbox" bind:checked={cleanup} disabled={running} />
          <span><strong>Clean up</strong> — fix punctuation & paragraphs, keep my words</span>
        </label>
        <label class="check">
          <input type="checkbox" bind:checked={draft} disabled={running} />
          <span><strong>Draft</strong> — rewrite into a polished, structured version</span>
        </label>
        <label class="check">
          <input type="checkbox" bind:checked={meetingNotes} disabled={running} />
          <span>
            <strong>Meeting notes</strong> — summary, decisions, and action items with owners.
            {#if meetingNotes && !diarize && canDiarize}
              <em class="why">Tip: turn on speaker labels too, so action items can be attributed.</em>
            {/if}
          </span>
        </label>
      </div>

      {#if cleanup}
        <div class="opt-row sub-opt">
          <label class="opt-label" for="llm-prov">
            Clean up with
          </label>
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
        {#if !llmHasKey}
          <p class="keywarn">No API key saved for {llmProvider} — add one in Settings → Providers first.</p>
        {/if}
      {/if}
      {#if draft || meetingNotes}
        <div class="opt-row sub-opt">
          <label class="opt-label" for="draft-llm-prov">Draft / meeting notes with</label>
          <div class="selects">
            <select id="draft-llm-prov" bind:value={draftLlmProvider} disabled={running}>
              {#each LLM_PROVIDERS as p}<option value={p.id}>{p.label}</option>{/each}
            </select>
            <select bind:value={draftLlmModel} disabled={running}>
              {#each draftLlmModels as m}<option value={m.id}>{m.label}</option>{/each}
            </select>
          </div>
        </div>
        {#if !draftLlmHasKey}<p class="keywarn">Add a {draftLlmProvider} key in Settings before running Draft / Meeting notes.</p>{/if}
      {/if}
    </div>

    <footer class="modal-foot">
      {#if finished}
        <span class="result-msg">
          {doneCount} transcribed{#if errCount > 0}, {errCount} failed{/if}. Added to your history.
        </span>
      {:else}
        <span class="hint-msg">
          {paths.length === 0 ? "Add at least one audio file." : `${paths.length} file${paths.length === 1 ? "" : "s"} ready.`}
        </span>
      {/if}
      <div class="foot-btns">
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
  .sub-opt {
    padding-left: 10px;
    border-left: 2px solid var(--accent-fade);
  }
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
</style>
