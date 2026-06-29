<script lang="ts">
  import { onMount } from "svelte";
  import { api, type SecretAuditEvent, type SecretLocation, type SecretsDiagnostic } from "$lib/api";

  let diag = $state<SecretsDiagnostic | null>(null);
  let events = $state<SecretAuditEvent[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);

  function locationLabel(loc: SecretLocation): string {
    return loc === "keyring"
      ? "OS keyring"
      : loc === "encrypted_file"
      ? "Encrypted fallback"
      : loc === "legacy_file"
      ? "Legacy plaintext"
      : loc === "file"
      ? "File fallback"
      : "Not saved";
  }

  function locationClass(loc: SecretLocation): string {
    return loc === "keyring"
      ? "ok"
      : loc === "encrypted_file"
      ? "warn"
      : loc === "file" || loc === "legacy_file"
      ? "danger"
      : "neutral";
  }

  function storageLabel(storage: string): string {
    const labels: Record<string, string> = {
      all: "All storage",
      keyring: "OS keyring",
      encrypted_file: "Encrypted fallback",
      file: "File fallback",
      legacy_file: "Legacy plaintext",
      none: "None",
    };
    return labels[storage] ?? storage;
  }

  function outcomeClass(outcome: string): string {
    return outcome === "ok"
      ? "ok"
      : outcome === "failed" || outcome === "verify_failed"
      ? "danger"
      : outcome === "started"
      ? "neutral"
      : "warn";
  }

  function when(ts: string): string {
    const d = new Date(ts);
    return Number.isNaN(d.getTime()) ? ts : d.toLocaleString();
  }

  function rows(d: SecretsDiagnostic) {
    return [
      { label: "Groq STT", loc: d.stt },
      { label: "Groq cleanup", loc: d.llm },
      { label: "OpenAI STT", loc: d.openai_stt },
      { label: "OpenAI cleanup", loc: d.openai_llm },
      { label: "Deepgram STT", loc: d.deepgram_stt },
      { label: "ElevenLabs STT", loc: d.elevenlabs_stt },
      { label: "Gemini cleanup", loc: d.gemini },
    ];
  }

  async function refresh() {
    loading = true;
    error = null;
    try {
      const [nextDiag, nextEvents] = await Promise.all([
        api.secretsDiagnostic(),
        api.secretAuditLog(120),
      ]);
      diag = nextDiag;
      events = nextEvents.slice().reverse();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(refresh);
</script>

<section>
  <div class="security-title-row">
    <div>
      <h2>Security</h2>
      <p class="lede">API keys stay local. This page shows storage status and recent key-storage events without showing the keys themselves.</p>
    </div>
    <button class="btn-secondary" onclick={refresh} disabled={loading}>
      {loading ? "Refreshing..." : "Refresh"}
    </button>
  </div>

  {#if error}
    <div class="settings-card security-error">{error}</div>
  {/if}

  {#if diag}
    <div class="settings-card">
      <div class="security-head">
        <h3>Storage status</h3>
        <span class:ok={diag.keyring_works} class:danger={!diag.keyring_works} class="status-pill">
          {diag.keyring_works ? "Keyring verified" : "Keyring not verified"}
        </span>
      </div>

      <ul class="security-list">
        {#each rows(diag) as row}
          <li>
            <span>{row.label}</span>
            <span class="status-pill {locationClass(row.loc)}">{locationLabel(row.loc)}</span>
          </li>
        {/each}
      </ul>

      {#if diag.legacy_fallback_exists}
        <div class="security-warning">
          Legacy plaintext fallback detected. The app will try to migrate it the next time that saved key is used.
        </div>
      {/if}

      <div class="path-grid">
        <div>
          <span>Encrypted fallback</span>
          <code>{diag.encrypted_fallback_path}</code>
          <em>{diag.encrypted_fallback_exists ? "exists" : "empty"}</em>
        </div>
        <div>
          <span>Legacy plaintext</span>
          <code>{diag.legacy_fallback_path}</code>
          <em>{diag.legacy_fallback_exists ? "exists" : "empty"}</em>
        </div>
        <div>
          <span>Audit log</span>
          <code>{diag.audit_log_path}</code>
          <em>{events.length} shown</em>
        </div>
      </div>
    </div>
  {/if}

  <details class="settings-card audit-card">
    <summary>
      <span>Key event log</span>
      <em>{events.length === 0 ? "No events" : `${events.length} recent events`}</em>
    </summary>
    <p class="audit-intro">Diagnostic storage events only. API key values are never shown here.</p>
    {#if events.length === 0}
      <p class="empty-log">No key-storage events yet.</p>
    {:else}
      <div class="audit-table">
        <div class="audit-row audit-header">
          <span>Time</span>
          <span>Key</span>
          <span>Action</span>
          <span>Storage</span>
          <span>Result</span>
          <span>Detail</span>
        </div>
        {#each events as event}
          <div class="audit-row">
            <span class="audit-time">{when(event.ts)}</span>
            <span>{event.label}</span>
            <span>{event.action}</span>
            <span>{storageLabel(event.storage)}</span>
            <span class="status-pill {outcomeClass(event.outcome)}">{event.outcome.replace("_", " ")}</span>
            <span class="audit-detail">{event.detail}</span>
          </div>
        {/each}
      </div>
    {/if}
  </details>
</section>

<style>
  .security-title-row {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    max-width: 980px;
  }

  .security-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 10px;
  }

  .security-head h3 {
    margin-top: 0;
  }

  .security-list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: grid;
    gap: 4px;
  }

  .security-list li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 7px 0;
    border-bottom: 1px dashed var(--border-subtle);
    font-size: 13px;
  }

  .security-list li:last-child {
    border-bottom: none;
  }

  .status-pill {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 22px;
    padding: 2px 8px;
    border-radius: 999px;
    font-size: 11px;
    font-weight: 600;
    white-space: nowrap;
  }

  .status-pill.ok {
    background: rgba(76, 175, 80, 0.15);
    color: #2e7d32;
  }

  .status-pill.warn {
    background: rgba(255, 152, 0, 0.18);
    color: #a96100;
  }

  .status-pill.danger,
  .security-error {
    background: var(--danger-fade);
    color: var(--danger);
  }

  .status-pill.neutral {
    background: var(--bg-subtle);
    color: var(--text-secondary);
  }

  .security-warning {
    margin: 12px 0 0;
    padding: 10px 12px;
    background: rgba(255, 152, 0, 0.1);
    border-left: 3px solid #ff9800;
    border-radius: 4px;
    font-size: 12px;
    color: var(--text-primary);
  }

  .path-grid {
    margin-top: 16px;
    display: grid;
    gap: 8px;
  }

  .path-grid div {
    display: grid;
    grid-template-columns: 140px minmax(0, 1fr) auto;
    gap: 10px;
    align-items: center;
    font-size: 11px;
    color: var(--text-secondary);
  }

  .path-grid code {
    min-width: 0;
    overflow-wrap: anywhere;
    background: var(--bg-subtle);
    color: var(--text-primary);
    border: 1px solid var(--border-subtle);
    border-radius: 4px;
    padding: 3px 6px;
  }

  .audit-card {
    max-width: 980px;
    padding: 0;
    overflow: hidden;
  }

  .audit-card > summary {
    list-style: none;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 14px 18px;
    cursor: pointer;
    font-size: 13px;
    font-weight: 700;
    color: var(--text-primary);
  }

  .audit-card > summary::-webkit-details-marker {
    display: none;
  }

  .audit-card > summary::before {
    content: "";
    width: 0;
    height: 0;
    border-top: 4px solid transparent;
    border-bottom: 4px solid transparent;
    border-left: 6px solid var(--text-secondary);
    transform-origin: 3px 4px;
    transition: transform 120ms ease;
  }

  .audit-card[open] > summary::before {
    transform: rotate(90deg);
  }

  .audit-card > summary em {
    margin-left: auto;
    font-size: 11px;
    font-style: normal;
    font-weight: 500;
    color: var(--text-secondary);
  }

  .audit-card[open] > summary {
    border-bottom: 1px solid var(--border-subtle);
  }

  .audit-intro {
    margin: 12px 16px;
    font-size: 12px;
    color: var(--text-secondary);
  }

  .empty-log {
    margin: 12px 16px 16px;
    font-size: 12px;
    color: var(--text-secondary);
  }

  .audit-table {
    display: grid;
    gap: 0;
    overflow-x: auto;
    padding: 0 16px 16px;
  }

  .audit-row {
    display: grid;
    grid-template-columns: 150px 130px 86px 130px 96px minmax(220px, 1fr);
    gap: 10px;
    align-items: center;
    min-width: 820px;
    padding: 8px 0;
    border-bottom: 1px solid var(--border-subtle);
    font-size: 12px;
    color: var(--text-primary);
  }

  .audit-header {
    color: var(--text-secondary);
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .audit-time,
  .audit-detail {
    color: var(--text-secondary);
  }

  .audit-detail {
    overflow-wrap: anywhere;
  }
</style>
