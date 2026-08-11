<script lang="ts">
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { api, type Recording } from "./api";
  import { history } from "./history-store.svelte";
  import { applySpeakerNames, namedSpeaker, speakerLabels, speakerNames, speakerTurns } from "./meeting-text";

  let { open = $bindable(false), rec, version, text } = $props<{ open?: boolean; rec: Recording; version: string; text: string }>();
  let names = $state<Record<string, string>>({});
  let saving = $state(false);
  let copied = $state(false);
  let labels = $derived(speakerLabels(rec));
  let turns = $derived(version === "raw" ? speakerTurns(rec) : []);
  let namedText = $derived(applySpeakerNames(text, names));
  let blocks = $derived(namedText.split(/\n\s*\n/).map((p) => p.trim()).filter(Boolean));

  $effect(() => { if (open) names = { ...speakerNames(rec) }; });
  $effect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => { if (event.key === "Escape") open = false; };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  async function saveNames() {
    saving = true;
    try {
      const clean = Object.fromEntries(Object.entries(names).filter(([, name]) => name.trim()).map(([label, name]) => [label, name.trim()]));
      await api.setSpeakerNames(rec.id, clean);
      await history.refresh();
    } finally { saving = false; }
  }

  async function copy() {
    await writeText(namedText);
    copied = true;
    setTimeout(() => (copied = false), 1300);
  }
</script>

{#if open}
  <div class="reader" role="dialog" aria-modal="true" aria-label="Focused reading mode">
    <header>
      <div><span class="eyebrow">Focused reading</span><h1>{rec.title || "Recording"}</h1><p>{version.replace("_", " ")} / {Math.max(1, Math.round(rec.duration_ms / 60000))} min</p></div>
      <div class="top-actions"><button onclick={copy}>{copied ? "Copied" : "Copy"}</button><button class="close" onclick={() => (open = false)} aria-label="Close reading mode">x</button></div>
    </header>
    <div class="layout" class:with-panel={rec.is_meeting}>
      {#if rec.is_meeting}
        <aside>
          <h2>Speakers</h2>
          <p>Name people as you read. Labels update in the page and copied text without re-transcribing.</p>
          {#if labels.length}
            <div class="speaker-fields">
              {#each labels as label}
                <label><span>{label}</span><input value={names[label] || ""} oninput={(e) => (names[label] = (e.currentTarget as HTMLInputElement).value)} placeholder="Name" /></label>
              {/each}
            </div>
            <button class="save" onclick={saveNames} disabled={saving}>{saving ? "Saving..." : "Save speaker names"}</button>
          {:else}<p class="no-speakers">No labelled turns in this version yet.</p>{/if}
        </aside>
      {/if}
      <main>
        {#if turns.length}
          <div class="turns">
            {#each turns as turn}
              <article class="turn"><h3>{namedSpeaker(turn.speaker, names)}</h3><p>{turn.text}</p></article>
            {/each}
          </div>
        {:else}
          <article class="prose">
            {#each blocks as block}
              {#if block.split("\n").every((line) => /^[-*] /.test(line))}
                <ul>{#each block.split("\n") as line}<li>{line.replace(/^[-*]\s+/, "")}</li>{/each}</ul>
              {:else if /^#{1,3}\s/.test(block)}
                <h2>{block.replace(/^#{1,3}\s+/, "")}</h2>
              {:else}<p>{block}</p>{/if}
            {/each}
          </article>
        {/if}
      </main>
    </div>
  </div>
{/if}

<style>
  .reader{position:fixed;inset:0;z-index:500;background:var(--bg-surface);color:var(--text-primary);display:flex;flex-direction:column;overflow:hidden}
  header{min-height:84px;padding:18px 28px;display:flex;align-items:center;justify-content:space-between;border-bottom:1px solid var(--border-subtle);background:color-mix(in srgb,var(--bg-card) 94%,transparent)}header h1{font-size:21px;margin:2px 0}header p{margin:0;color:var(--text-secondary);font-size:11px;text-transform:capitalize}.eyebrow{color:var(--accent);font-size:10px;font-weight:750;text-transform:uppercase;letter-spacing:.11em}.top-actions{display:flex;gap:8px}.top-actions button{border:1px solid var(--border);background:var(--bg-card);color:var(--text-primary);border-radius:9px;padding:8px 12px;cursor:pointer}.top-actions .close{font-size:20px;line-height:1;padding:6px 11px}.layout{flex:1;min-height:0;display:grid;grid-template-columns:1fr}.layout.with-panel{grid-template-columns:250px minmax(0,1fr)}aside{padding:24px 18px;border-right:1px solid var(--border-subtle);background:var(--bg-card);overflow:auto}aside h2{margin:0;font-size:15px}aside>p{font-size:11px;line-height:1.5;color:var(--text-secondary)}.speaker-fields{display:grid;gap:12px;margin-top:18px}.speaker-fields label{display:grid;gap:5px}.speaker-fields span{font-size:10px;font-weight:700;color:var(--text-secondary)}.speaker-fields input{border:1px solid var(--border);border-radius:8px;background:var(--bg-surface);color:var(--text-primary);padding:8px 9px;font:inherit}.save{width:100%;margin-top:14px;border:0;border-radius:8px;background:var(--accent);color:#fff;padding:8px;font:inherit;font-size:11px;font-weight:700;cursor:pointer}.no-speakers{padding:10px;background:var(--bg-subtle);border-radius:8px}main{overflow:auto;padding:42px clamp(28px,7vw,110px) 90px}.prose,.turns{max-width:820px;margin:0 auto;font-family:Inter,ui-sans-serif,system-ui;letter-spacing:.003em}.prose p,.prose li{font-size:17px;line-height:1.82}.prose p{margin:0 0 1.35em;white-space:pre-wrap}.prose h2{font-size:20px;margin:2em 0 .7em}.prose ul{margin:0 0 1.5em;padding-left:1.5em}.turns{display:grid;gap:14px}.turn{padding:18px 21px;background:var(--bg-card);border:1px solid var(--border-subtle);border-radius:13px;box-shadow:var(--shadow-xs)}.turn h3{margin:0 0 8px;color:var(--accent);font-size:12px}.turn p{margin:0;font-size:16px;line-height:1.75;white-space:pre-wrap}@media(max-width:720px){.layout.with-panel{grid-template-columns:1fr}aside{border-right:0;border-bottom:1px solid var(--border);max-height:220px}.speaker-fields{grid-template-columns:repeat(2,minmax(0,1fr))}main{padding:28px 20px 70px}}
</style>
