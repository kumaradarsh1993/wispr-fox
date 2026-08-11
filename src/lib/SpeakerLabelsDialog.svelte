<script lang="ts">
  import { api, type Recording } from "./api";
  import { history } from "./history-store.svelte";
  import { speakerLabels, speakerNames } from "./meeting-text";

  let { open = $bindable(false), rec } = $props<{ open?: boolean; rec: Recording }>();
  let labels = $derived(speakerLabels(rec));
  let names = $state<Record<string, string>>({});
  let saving = $state(false);

  $effect(() => {
    if (open) names = { ...speakerNames(rec) };
  });

  async function save() {
    saving = true;
    try {
      const clean = Object.fromEntries(Object.entries(names).filter(([, name]) => name.trim()).map(([label, name]) => [label, name.trim()]));
      await api.setSpeakerNames(rec.id, clean);
      await history.refresh();
      open = false;
    } finally {
      saving = false;
    }
  }
</script>

{#if open}
  <button class="backdrop" aria-label="Close speaker labels" onclick={() => (open = false)}></button>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="speaker-title">
    <header><div><h2 id="speaker-title">Name the speakers</h2><p>Names are applied instantly when reading or copying. The transcript is never rewritten.</p></div><button class="close" onclick={() => (open = false)} aria-label="Close">x</button></header>
    {#if labels.length}
      <div class="fields">
        {#each labels as label}
          <label><span>{label}</span><input value={names[label] || ""} oninput={(e) => (names[label] = (e.currentTarget as HTMLInputElement).value)} placeholder="Enter a name" /></label>
        {/each}
      </div>
    {:else}
      <p class="empty">No speaker turns were found. Re-run transcription with Label speakers selected.</p>
    {/if}
    <footer><button class="ghost" onclick={() => (open = false)}>Cancel</button><button class="primary" onclick={save} disabled={saving || !labels.length}>{saving ? "Saving..." : "Save names"}</button></footer>
  </div>
{/if}

<style>
  .backdrop{position:fixed;inset:0;z-index:410;background:rgba(28,20,12,.46);border:0;backdrop-filter:blur(3px)}
  .dialog{position:fixed;z-index:411;inset:50% auto auto 50%;transform:translate(-50%,-50%);width:min(500px,calc(100vw - 36px));max-height:calc(100vh - 48px);overflow:auto;background:var(--bg-card);border:1px solid var(--border);border-radius:16px;box-shadow:0 26px 80px rgba(35,22,9,.34);padding:20px}
  header{display:flex;justify-content:space-between;gap:16px}h2{margin:0;font-size:19px}p{margin:5px 0 0;color:var(--text-secondary);font-size:12px;line-height:1.5}.close{border:0;background:transparent;color:var(--text-secondary);font-size:22px;cursor:pointer}.fields{display:grid;gap:10px;margin:18px 0}.fields label{display:grid;grid-template-columns:110px 1fr;align-items:center;gap:12px;font-size:12px;font-weight:650}.fields input{border:1px solid var(--border);border-radius:9px;background:var(--bg-surface);color:var(--text-primary);padding:9px 11px;font:inherit}.fields input:focus{outline:2px solid var(--accent-fade);border-color:var(--accent)}.empty{padding:20px;background:var(--bg-subtle);border-radius:10px}footer{display:flex;justify-content:flex-end;gap:8px;border-top:1px solid var(--border-subtle);padding-top:14px}.ghost,.primary{border-radius:9px;padding:8px 14px;font:inherit;cursor:pointer}.ghost{background:transparent;border:1px solid var(--border);color:var(--text-primary)}.primary{background:var(--accent);border:1px solid var(--accent);color:#fff;font-weight:650}.primary:disabled{opacity:.5}
</style>
