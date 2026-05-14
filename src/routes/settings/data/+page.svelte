<script lang="ts">
  // Data — history retention + storage caps.
  import { settings } from "$lib/settings-store.svelte";

  async function setNumber<K extends keyof typeof settings.s>(key: K, value: number) {
    await settings.set(key, value as (typeof settings.s)[K]);
  }
</script>

<section>
  <h2>Data</h2>
  <p class="lede">How long recordings (audio + transcript) are kept before automatic cleanup.</p>

  <div class="settings-card">
    <div class="field-block">
      <label>Retention: {settings.s.retention_days} day{settings.s.retention_days === 1 ? "" : "s"}</label>
      <input
        type="range"
        min="1"
        max="90"
        value={settings.s.retention_days}
        oninput={(e) => setNumber("retention_days", Number((e.currentTarget as HTMLInputElement).value))}
      />
      <p class="hint">Recordings older than this are auto-deleted hourly. Affects both audio files and text.</p>
    </div>

    <div class="field-block">
      <label>Storage cap (MB)</label>
      <input
        type="number"
        min="50"
        step="50"
        value={settings.s.retention_max_mb}
        onchange={(e) => setNumber("retention_max_mb", Number((e.currentTarget as HTMLInputElement).value))}
      />
      <p class="hint">When the audio folder exceeds this, the oldest files are removed first.</p>
    </div>
  </div>
</section>
