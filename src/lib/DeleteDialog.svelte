<script lang="ts">
  // Ownership-scoped delete confirm (v3.0.0, supersedes the What/Where matrix).
  // The press-and-hold on the delete control only ARMS the delete; this dialog
  // is the second gate. There are no What/Where axes any more: a client may
  // delete only what it originated, and a transcript + its recording die
  // together. Deleting your own row also tombstones the cloud copy, so it
  // leaves your other devices too — the copy states that plainly.
  import { api } from "./api";
  import { account } from "./account-store.svelte";

  let {
    open = $bindable(false),
    // null = all of THIS device's recordings; otherwise the specific ids.
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

  let busy = $state(false);
  let error = $state("");

  let signedIn = $derived(account.signedIn);

  // Reset transient state each time the dialog opens.
  $effect(() => {
    if (open) {
      error = "";
      busy = false;
    }
  });

  let isAll = $derived(ids === null);

  let consequence = $derived.by(() => {
    // "All" only ever touches THIS device's rows — other devices' transcripts
    // survive locally and on the server, so we say so to kill the old
    // "everything on every device" fear.
    if (isAll) {
      if (signedIn) {
        return `Permanently deletes every recording made on this device — the text and the audio. Transcripts from your other devices stay put. This can't be undone.`;
      }
      return `Permanently deletes every recording on this device — the text and the audio. This can't be undone.`;
    }
    if (signedIn) {
      return `Permanently deletes ${label} — the transcript and the audio. It's removed from your other devices too. This can't be undone.`;
    }
    return `Permanently deletes ${label} — the transcript and the audio. This can't be undone.`;
  });

  function close() {
    open = false;
  }

  async function confirm() {
    busy = true;
    error = "";
    try {
      await api.deleteRecordings(ids);
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

      <p class="consequence">{consequence}</p>

      {#if error}<p class="err">{error}</p>{/if}

      <div class="actions">
        <button class="btn ghost" onclick={close} disabled={busy}>Cancel</button>
        <button class="btn danger" onclick={confirm} disabled={busy}>
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
    margin: 0 0 12px;
    font-size: 17px;
    font-weight: 600;
  }
  .consequence {
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--danger);
    background: var(--danger-fade);
    border-radius: 10px;
    padding: 10px 12px;
    margin: 4px 0 16px;
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
