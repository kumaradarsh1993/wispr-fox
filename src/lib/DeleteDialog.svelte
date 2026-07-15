<script lang="ts">
  // Reworked delete dialog (v3.0.0). Released by a press-and-hold on a delete
  // control (History "Clear" or a per-row delete). Offers, clearly layered:
  //   What:  ☐ Voice files   ☐ Transcripts
  //   Where: ◉ This device only  ◯ Everywhere  (Everywhere only when signed in)
  //   + a plain-words consequence line, then Confirm.
  import { api, type DeleteWhat } from "./api";
  import { account } from "./account-store.svelte";

  let {
    open = $bindable(false),
    // null = all recordings; otherwise the specific ids this delete targets.
    ids = null as string[] | null,
    // Human label for the consequence line ("all recordings" vs "this recording").
    label = "all recordings",
    onDone = () => {},
  } = $props<{
    open?: boolean;
    ids?: string[] | null;
    label?: string;
    onDone?: () => void;
  }>();

  let audio = $state(true);
  let transcripts = $state(true);
  let scope = $state<"device" | "everywhere">("device");
  let busy = $state(false);
  let error = $state("");

  let signedIn = $derived(account.signedIn);

  // Reset the form each time the dialog opens.
  $effect(() => {
    if (open) {
      audio = true;
      transcripts = true;
      scope = "device";
      error = "";
      busy = false;
    }
  });

  // Everywhere is only meaningful when signed in — force back to device-only
  // if the user signs out while the dialog is somehow open.
  $effect(() => {
    if (!signedIn && scope === "everywhere") scope = "device";
  });

  let nothingChosen = $derived(!audio && !transcripts);

  let consequence = $derived.by(() => {
    if (nothingChosen) return "Pick at least one thing to delete.";
    const parts: string[] = [];
    if (audio) parts.push("voice files");
    if (transcripts) parts.push("transcripts");
    const what = parts.join(" and ");
    if (scope === "everywhere") {
      return `Deletes the ${what} for ${label} on every signed-in device. This can't be undone.`;
    }
    if (transcripts && signedIn) {
      return `Deletes the ${what} for ${label} on this device only. The cloud copy stays for your other devices (they won't reappear here).`;
    }
    return `Deletes the ${what} for ${label} on this device only.`;
  });

  function close() {
    open = false;
  }

  async function confirm() {
    if (nothingChosen) return;
    busy = true;
    error = "";
    try {
      const what: DeleteWhat = { audio, transcripts };
      await api.deleteRecordings(scope, what, ids);
      open = false;
      onDone();
    } catch (e) {
      error = `${e}`;
    } finally {
      busy = false;
    }
  }
</script>

{#if open}
  <div
    class="overlay"
    role="button"
    tabindex="-1"
    onclick={close}
    onkeydown={(e) => { if (e.key === "Escape") close(); }}
  >
    <!-- Stop propagation so clicking inside the card doesn't dismiss it. -->
    <div class="card" role="dialog" aria-modal="true" aria-label="Delete recordings" onclick={(e) => e.stopPropagation()} onkeydown={() => {}} tabindex="-1">
      <h2>Delete {label}</h2>

      <div class="group">
        <div class="group-label">What to delete</div>
        <label class="check">
          <input type="checkbox" bind:checked={audio} />
          <span>Voice files <em>(the audio recordings on disk)</em></span>
        </label>
        <label class="check">
          <input type="checkbox" bind:checked={transcripts} />
          <span>Transcripts <em>(the text — removes the whole entry)</em></span>
        </label>
      </div>

      <div class="group">
        <div class="group-label">Where</div>
        <label class="radio">
          <input type="radio" value="device" bind:group={scope} />
          <span>This device only</span>
        </label>
        <label class="radio" class:disabled={!signedIn}>
          <input type="radio" value="everywhere" bind:group={scope} disabled={!signedIn} />
          <span>
            Everywhere
            {#if !signedIn}<em>(sign in to sync-delete across devices)</em>{/if}
          </span>
        </label>
      </div>

      <p class="consequence" class:danger={scope === "everywhere"}>{consequence}</p>

      {#if error}<p class="err">{error}</p>{/if}

      <div class="actions">
        <button class="btn ghost" onclick={close} disabled={busy}>Cancel</button>
        <button class="btn danger" onclick={confirm} disabled={busy || nothingChosen}>
          {busy ? "Deleting…" : "Delete"}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 400;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(30, 24, 16, 0.42);
    backdrop-filter: blur(2px);
    padding: 24px;
  }
  .card {
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 16px;
    padding: 22px 24px;
    width: 100%;
    max-width: 420px;
    box-shadow: 0 18px 60px rgba(40, 26, 10, 0.35);
    color: var(--text-primary);
    text-align: left;
  }
  h2 {
    margin: 0 0 16px;
    font-size: 17px;
    font-weight: 600;
  }
  .group {
    margin-bottom: 16px;
  }
  .group-label {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-secondary);
    margin-bottom: 8px;
  }
  .check,
  .radio {
    display: flex;
    align-items: flex-start;
    gap: 9px;
    padding: 6px 0;
    font-size: 13px;
    cursor: pointer;
  }
  .check input,
  .radio input {
    margin-top: 2px;
    accent-color: var(--accent);
    flex-shrink: 0;
  }
  .check em,
  .radio em {
    color: var(--text-secondary);
    font-style: normal;
    font-size: 12px;
  }
  .radio.disabled {
    cursor: default;
    opacity: 0.6;
  }
  .consequence {
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--text-secondary);
    background: var(--bg-subtle);
    border-radius: 10px;
    padding: 10px 12px;
    margin: 4px 0 16px;
  }
  .consequence.danger {
    color: var(--danger);
    background: var(--danger-fade);
  }
  .err {
    color: var(--danger);
    font-size: 12px;
    margin: 0 0 12px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .btn {
    border-radius: 8px;
    padding: 8px 16px;
    font-size: 13px;
    font-weight: 600;
    font-family: inherit;
    cursor: pointer;
    border: 1px solid transparent;
  }
  .btn.ghost {
    background: transparent;
    border-color: var(--border);
    color: var(--text-primary);
  }
  .btn.ghost:hover {
    background: var(--bg-subtle);
  }
  .btn.danger {
    background: var(--danger);
    color: #fff;
  }
  .btn.danger:hover {
    filter: brightness(1.05);
  }
  .btn:disabled {
    opacity: 0.55;
    cursor: default;
  }
</style>
