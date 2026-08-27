<script lang="ts">
  import { api, type Recording, type SecretCheck } from "./api";
  import { history } from "./history-store.svelte";
  import { settings } from "./settings-store.svelte";
  import { LLM_PROVIDERS, STT_PROVIDERS, llmModelsFor, llmReady, sttModelsFor, sttReady } from "./provider-options";
  import { askConfirm } from "./dialogs";

  let { open = $bindable(false), rec } = $props<{ open?: boolean; rec: Recording }>();
  let transcribe = $state(false), diarize = $state(false), cleanup = $state(false), draft = $state(false), meeting = $state(false);
  let sttProvider = $state("groq"), sttModel = $state("whisper-large-v3-turbo");
  let cleanupProvider = $state("groq"), cleanupModel = $state("openai/gpt-oss-20b");
  let draftProvider = $state("groq"), draftModel = $state("openai/gpt-oss-120b");
  let switchNote = $state(""), running = $state(false), stage = $state(""), error = $state("");
  let secrets = $state<SecretCheck | null>(null);
  let sttModels = $derived(sttModelsFor(sttProvider));
  let cleanupModels = $derived(llmModelsFor(cleanupProvider));
  let draftModels = $derived(llmModelsFor(draftProvider));
  let canDiarize = $derived(sttProvider === "deepgram" || sttProvider === "elevenlabs" || (sttProvider === "openai" && sttModel === "gpt-4o-transcribe-diarize"));
  let ready = $derived((!transcribe || sttReady(secrets, sttProvider)) && (!cleanup || llmReady(secrets, cleanupProvider)) && (!(draft || meeting) || llmReady(secrets, draftProvider)));

  $effect(() => {
    if (!open) return;
    transcribe = rec.status === "error" && !rec.remote;
    diarize = rec.diarization_enabled;
    cleanup = false; draft = false; meeting = false; error = ""; stage = ""; switchNote = "";
    sttProvider = settings.s.stt_provider; sttModel = settings.s.stt_model;
    cleanupProvider = settings.s.llm_provider; cleanupModel = settings.s.llm_model;
    draftProvider = settings.s.draft_llm_provider; draftModel = settings.s.draft_llm_model;
    api.checkSecrets().then((value) => (secrets = value)).catch(() => {});
    if (diarize) enableDiarize(true);
  });
  $effect(() => { if (!sttModels.some((m) => m.id === sttModel)) sttModel = sttModels[0].id; });
  $effect(() => { if (!cleanupModels.some((m) => m.id === cleanupModel)) cleanupModel = cleanupModels[0].id; });
  $effect(() => { if (!draftModels.some((m) => m.id === draftModel)) draftModel = draftModels[0].id; });
  $effect(() => { if (diarize && !canDiarize) enableDiarize(true); });

  function enableDiarize(on: boolean) {
    diarize = on; switchNote = "";
    if (!on) return;
    transcribe = true;
    if (sttProvider === "openai") {
      if (sttModel !== "gpt-4o-transcribe-diarize") {
        sttModel = "gpt-4o-transcribe-diarize";
        switchNote = "Changed to GPT-4o Diarize because the selected OpenAI model has no speaker labels.";
      }
    } else if (sttProvider !== "deepgram" && sttProvider !== "elevenlabs") {
      sttProvider = "deepgram"; sttModel = "nova-3";
      switchNote = "Changed to Deepgram Nova-3 because Whisper does not support speaker labels.";
    }
  }

  async function run() {
    if (!ready || !(transcribe || cleanup || draft || meeting)) return;
    if (transcribe && rec.transcript && !(await askConfirm("Replace the current transcript and run the selected follow-up steps?"))) return;
    running = true; error = "";
    try {
      if (transcribe) { stage = "Transcribing..."; await api.rerunTranscription(rec.id, sttProvider, sttModel, diarize); }
      if (cleanup) { stage = "Cleaning up..."; await api.generateAltVersion(rec.id, "cleaned", { provider: cleanupProvider, model: cleanupModel }); }
      if (draft) { stage = "Drafting..."; await api.generateAltVersion(rec.id, "drafted", { provider: draftProvider, model: draftModel }); }
      if (meeting) { stage = "Creating meeting notes..."; await api.generateAltVersion(rec.id, "meeting_notes", { provider: draftProvider, model: draftModel }); }
      await history.refresh(); open = false;
    } catch (e) { error = String(e); }
    finally { running = false; stage = ""; }
  }
</script>

{#if open}
  <button class="backdrop" aria-label="Close rerun dialog" onclick={() => !running && (open = false)}></button>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="rerun-title">
    <header><div><h2 id="rerun-title">Rerun recording</h2><p>Select any combination. Transcription always finishes before AI versions start.</p></div><button class="close" aria-label="Close" onclick={() => (open = false)} disabled={running}>x</button></header>
    <div class="block first">
      <label class="choice"><input type="checkbox" bind:checked={transcribe} disabled={running || rec.remote}/><span><strong>Transcription</strong><em>Re-read the original audio</em></span></label>
      {#if transcribe}
        <div class="selects"><select bind:value={sttProvider} disabled={running}>{#each STT_PROVIDERS as p}<option value={p.id}>{p.label}</option>{/each}</select><select bind:value={sttModel} disabled={running}>{#each sttModels as m}<option value={m.id}>{m.label}</option>{/each}</select></div>
        <label class="choice sub"><input type="checkbox" checked={diarize} onchange={(e) => enableDiarize((e.currentTarget as HTMLInputElement).checked)} disabled={running}/><span><strong>Label speakers</strong><em>Makes this a meeting and preserves editable speaker placeholders</em></span></label>
        {#if switchNote}<p class="switch-note">{switchNote}</p>{/if}
      {/if}
    </div>
    <div class="connector">then</div>
    <div class="block">
      <h3>AI versions</h3>
      <label class="choice"><input type="checkbox" bind:checked={cleanup} disabled={running}/><span><strong>Cleanup</strong><em>Readable punctuation and paragraphs, same meaning</em></span></label>
      {#if cleanup}<div class="selects"><select bind:value={cleanupProvider} disabled={running}>{#each LLM_PROVIDERS as p}<option value={p.id}>{p.label}</option>{/each}</select><select bind:value={cleanupModel} disabled={running}>{#each cleanupModels as m}<option value={m.id}>{m.label}</option>{/each}</select></div>{/if}
      <label class="choice"><input type="checkbox" bind:checked={draft} disabled={running}/><span><strong>Draft</strong><em>A polished output for sending</em></span></label>
      <label class="choice"><input type="checkbox" bind:checked={meeting} disabled={running}/><span><strong>Meeting notes</strong><em>Succinct summary, decisions, risks, and actions</em></span></label>
      {#if draft || meeting}<div class="selects"><select bind:value={draftProvider} disabled={running}>{#each LLM_PROVIDERS as p}<option value={p.id}>{p.label}</option>{/each}</select><select bind:value={draftModel} disabled={running}>{#each draftModels as m}<option value={m.id}>{m.label}</option>{/each}</select></div>{/if}
    </div>
    {#if !ready}<p class="warn">Add the selected provider key in Settings first.</p>{/if}{#if error}<p class="warn">{error}</p>{/if}
    <footer><span>{stage}</span><div><button class="ghost" onclick={() => (open = false)} disabled={running}>Cancel</button><button class="primary" onclick={run} disabled={running || !ready || !(transcribe || cleanup || draft || meeting)}>{running ? "Working..." : "Run selected"}</button></div></footer>
  </div>
{/if}

<style>
  .backdrop{position:fixed;inset:0;z-index:420;background:rgba(28,20,12,.46);border:0;backdrop-filter:blur(3px)}.dialog{position:fixed;z-index:421;left:50%;top:50%;transform:translate(-50%,-50%);width:min(600px,calc(100vw - 36px));max-height:calc(100vh - 40px);overflow:auto;background:var(--bg-card);border:1px solid var(--border);border-radius:17px;padding:20px;box-shadow:0 28px 84px rgba(35,22,9,.36)}header{display:flex;justify-content:space-between;gap:16px}h2{margin:0;font-size:19px}header p{margin:4px 0 0;color:var(--text-secondary);font-size:12px}.close{border:0;background:transparent;color:var(--text-secondary);font-size:22px;cursor:pointer}.block{display:grid;gap:10px;padding:15px;border:1px solid var(--border-subtle);border-radius:12px;background:var(--bg-subtle)}.block.first{margin-top:17px}.block h3{margin:0 0 2px;font-size:12px;text-transform:uppercase;letter-spacing:.07em;color:var(--text-secondary)}.choice{display:flex;align-items:flex-start;gap:9px;font-size:12px}.choice input{margin-top:3px;accent-color:var(--accent)}.choice span{display:grid;gap:2px}.choice em{font-style:normal;color:var(--text-secondary);font-size:11px}.choice.sub{margin-left:5px}.selects{display:grid;grid-template-columns:1fr 1.25fr;gap:8px;margin-left:23px}.selects select{min-width:0;border:1px solid var(--border);border-radius:8px;background:var(--bg-card);color:var(--text-primary);padding:8px;font:inherit;font-size:12px}.connector{text-align:center;color:var(--text-secondary);font-size:10px;text-transform:uppercase;letter-spacing:.12em;padding:5px}.switch-note{margin:0 0 0 23px;padding:7px 9px;border-radius:8px;background:var(--accent-fade);color:var(--accent);font-size:11px}.warn{font-size:11px;color:var(--danger);background:var(--danger-fade);padding:8px;border-radius:8px}footer{display:flex;align-items:center;justify-content:space-between;gap:12px;margin-top:15px;border-top:1px solid var(--border-subtle);padding-top:14px}footer>span{font-size:11px;color:var(--accent)}footer>div{display:flex;gap:8px}.ghost,.primary{border-radius:9px;padding:8px 14px;font:inherit;cursor:pointer}.ghost{background:transparent;border:1px solid var(--border);color:var(--text-primary)}.primary{background:var(--accent);border:1px solid var(--accent);color:#fff;font-weight:650}.primary:disabled{opacity:.45}
</style>
