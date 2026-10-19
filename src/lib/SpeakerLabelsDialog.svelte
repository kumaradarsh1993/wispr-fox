<script lang="ts">
  import { tick } from "svelte";
  import { api, type Recording } from "./api";
  import { history } from "./history-store.svelte";
  import { speakerNames, speakerTurns } from "./meeting-text";

  // `focus` names the speaker whose field should take focus on open (the
  // Conversations reader opens this from a click on that speaker's name).
  let {
    open = $bindable(false),
    rec,
    focus = null,
    onSaved,
  } = $props<{ open?: boolean; rec: Recording; focus?: string | null; onSaved?: () => void }>();

  // Each speaker with the first thing they said, so the user can tell who is
  // who before typing a name.
  let speakers = $derived.by(() => {
    const seen = new Map<string, string>();
    for (const t of speakerTurns(rec)) {
      if (!seen.has(t.speaker)) seen.set(t.speaker, t.text);
    }
    return [...seen.entries()].map(([label, first]) => ({ label, first }));
  });
  let names = $state<Record<string, string>>({});
  let saving = $state(false);
  let error = $state("");
  let fieldsEl = $state<HTMLDivElement | null>(null);

  let wasOpen = false;
  $effect(() => {
    if (open && !wasOpen) {
      names = { ...speakerNames(rec) };
      error = "";
      const target = focus;
      void tick().then(() => {
        const input = fieldsEl?.querySelector<HTMLInputElement>(
          target ? `input[data-label="${CSS.escape(target)}"]` : "input",
        );
        input?.focus();
        input?.select();
      });
    }
    wasOpen = open;
  });

  function firstLine(text: string): string {
    const t = text.trim().replace(/\s+/g, " ");
    return t.length > 110 ? `${t.slice(0, 110)}…` : t;
  }

  async function save() {
    saving = true;
    error = "";
    try {
      const clean = Object.fromEntries(Object.entries(names).filter(([, name]) => name.trim()).map(([label, name]) => [label, name.trim()]));
      await api.setSpeakerNames(rec.id, clean);
      await history.refresh();
      onSaved?.();
      open = false;
    } catch (e) {
      error = String(e);
    } finally {
      saving = false;
    }
  }
</script>

{#if open}
  <button class="backdrop" aria-label="Close speaker labels" onclick={() => (open = false)}></button>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="speaker-title">
    <header><div><h2 id="speaker-title">Name the speakers</h2><p>Names apply everywhere you read, copy or export this conversation. The transcript itself is never rewritten.</p></div><button class="close" onclick={() => (open = false)} aria-label="Close">×</button></header>
    {#if speakers.length}
      <div class="fields" bind:this={fieldsEl}>
        {#each speakers as s (s.label)}
          <label>
            <span class="who">{s.label}</span>
            <span class="said">“{firstLine(s.first)}”</span>
            <input
              data-label={s.label}
              value={names[s.label] || ""}
              oninput={(e) => (names[s.label] = (e.currentTarget as HTMLInputElement).value)}
              onkeydown={(e) => { if (e.key === "Enter") void save(); }}
              placeholder="Enter a name"
            />
          </label>
        {/each}
      </div>
    {:else}
      <p class="empty">No speaker turns were found. Re-run the speakers pass to label who spoke.</p>
    {/if}
    {#if error}<p class="err">{error}</p>{/if}
    <footer><button class="ghost" onclick={() => (open = false)}>Cancel</button><button class="primary" onclick={save} disabled={saving || !speakers.length}>{saving ? "Saving..." : "Save names"}</button></footer>
  </div>
{/if}

<style>
  .backdrop{position:fixed;inset:0;z-index:410;background:rgba(28,20,12,.46);border:0;backdrop-filter:blur(3px)}
  .dialog{position:fixed;z-index:411;inset:50% auto auto 50%;transform:translate(-50%,-50%);width:min(520px,calc(100vw - 36px));max-height:calc(100vh - 48px);overflow:auto;background:var(--bg-card);border:1px solid var(--border);border-radius:16px;box-shadow:0 26px 80px rgba(35,22,9,.34);padding:20px}
  header{display:flex;justify-content:space-between;gap:16px}h2{margin:0;font-size:19px}p{margin:5px 0 0;color:var(--text-secondary);font-size:12px;line-height:1.5}.close{border:0;background:transparent;color:var(--text-secondary);font-size:22px;cursor:pointer}
  .fields{display:grid;gap:14px;margin:18px 0}
  .fields label{display:grid;grid-template-columns:1fr;gap:4px;font-size:12px}
  .who{font-weight:650;color:var(--accent)}
  .said{color:var(--text-secondary);font-style:italic;line-height:1.4}
  .fields input{border:1px solid var(--border);border-radius:9px;background:var(--bg-surface);color:var(--text-primary);padding:9px 11px;font:inherit}.fields input:focus{outline:2px solid var(--accent-fade);border-color:var(--accent)}
  .empty{padding:20px;background:var(--bg-subtle);border-radius:10px}
  .err{color:var(--danger);margin-bottom:10px}
  footer{display:flex;justify-content:flex-end;gap:8px;border-top:1px solid var(--border-subtle);padding-top:14px}.ghost,.primary{border-radius:9px;padding:8px 14px;font:inherit;cursor:pointer}.ghost{background:transparent;border:1px solid var(--border);color:var(--text-primary)}.primary{background:var(--accent);border:1px solid var(--accent);color:#fff;font-weight:650}.primary:disabled{opacity:.5}
</style>
