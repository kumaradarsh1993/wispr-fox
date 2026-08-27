<script lang="ts">
  // General — startup behaviour + audio cues.
  // Two unrelated-but-small section pairs that were each too small to
  // deserve their own sub-route.
  import { onMount } from "svelte";
  import { api } from "$lib/api";
  import { settings } from "$lib/settings-store.svelte";
  import { flash } from "$lib/settings-toast.svelte";
  import { showMessage } from "$lib/dialogs";

  async function setNumber<K extends keyof typeof settings.s>(key: K, value: number) {
    await settings.set(key, value as (typeof settings.s)[K]);
  }

  // ── Autostart sync ─────────────────────────────────────────────────────
  // Keep the OS-level "launch on login" registration in step with our
  // settings.autostart flag. tauri-plugin-autostart's enable() / disable()
  // write the platform-specific entry (Win registry / launchd / systemd)
  // and isEnabled() reads it back. We don't trust our setting in isolation
  // because the user might have flipped the OS entry manually.
  async function syncAutostart(target: boolean) {
    try {
      const { enable, disable } = await import("@tauri-apps/plugin-autostart");
      if (target) await enable();
      else await disable();
      await settings.set("autostart", target);
    } catch (e) {
      console.warn("autostart sync failed", e);
    }
  }

  // ── Notification sounds ────────────────────────────────────────────────
  // Start + stop sounds are always synced now — the v1.0.0 UX feedback
  // was that two separate pickers + an opt-in toggle was clutter. Picking
  // a cue sets both start and stop to the same file.
  let availableSounds = $state<string[]>([]);
  let soundsDir = $state<string>("");
  let previewAudio: HTMLAudioElement | null = null;

  async function refreshSounds() {
    availableSounds = await api.listNotificationSounds();
    try {
      const paths = await api.appPaths();
      soundsDir = paths.sounds_dir;
    } catch {
      /* ignore */
    }
  }

  async function setCueSound(name: string) {
    await settings.set("start_sound", name);
    await settings.set("stop_sound", name);
    await api.configureCues(name, name, settings.s.cues_enabled);
    previewSound(name);
    flash(name ? "Cue sound updated" : "Built-in tone selected");
  }

  async function setCuesEnabled(on: boolean) {
    await settings.set("cues_enabled", on);
    await api.configureCues(settings.s.start_sound, settings.s.stop_sound, on);
  }

  // Preview a sound by name. Empty string = built-in tone; we synthesize
  // a brief WebAudio chirp so the user gets audible feedback for that
  // choice too (used to be silent which read as "nothing happened").
  async function previewSound(name: string) {
    if (previewAudio) {
      previewAudio.pause();
      previewAudio = null;
    }
    if (!name) {
      try {
        const ctx = new (window.AudioContext || (window as any).webkitAudioContext)();
        const o = ctx.createOscillator();
        const g = ctx.createGain();
        o.type = "sine";
        o.frequency.setValueAtTime(880, ctx.currentTime);
        o.frequency.exponentialRampToValueAtTime(440, ctx.currentTime + 0.12);
        g.gain.setValueAtTime(0.0001, ctx.currentTime);
        g.gain.exponentialRampToValueAtTime(0.25, ctx.currentTime + 0.02);
        g.gain.exponentialRampToValueAtTime(0.0001, ctx.currentTime + 0.18);
        o.connect(g).connect(ctx.destination);
        o.start();
        o.stop(ctx.currentTime + 0.2);
        setTimeout(() => ctx.close(), 300);
      } catch (e) {
        console.warn("built-in tone preview failed", e);
      }
      return;
    }
    const paths = await api.appPaths();
    const fileUrl = `file://${paths.sounds_dir.replace(/\\/g, "/")}/${encodeURIComponent(name)}`;
    try {
      previewAudio = new Audio(fileUrl);
      previewAudio.volume = 0.6;
      await previewAudio.play();
    } catch (e) {
      console.warn("preview failed", e);
    }
  }

  async function uploadSound() {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const picked = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "Audio", extensions: ["wav", "mp3", "ogg"] }],
      });
      if (typeof picked === "string") {
        await api.addNotificationSound(picked);
        await refreshSounds();
        flash("Sound added");
      }
    } catch (e) {
      await showMessage(`Upload failed: ${e}`);
    }
  }

  // Updates moved to Settings -> About, where version, both release channels
  // and the installer live together. Keeping a second, weaker copy here would
  // just be two places to look.
  onMount(() => {
    refreshSounds();
  });
</script>

<section>
  <h2>App &amp; data</h2>
  <p class="lede">Startup, sounds, and data retention.</p>

  <h3>Startup</h3>

  <div class="behavior-block">
    <label class="check-row" title="Registers a per-user Windows startup entry (no admin needed). Tray icon and avatar appear; no main window unless you ask.">
      <input
        type="checkbox"
        checked={settings.s.autostart}
        onchange={(e) => syncAutostart((e.currentTarget as HTMLInputElement).checked)}
      />
      <span><strong>Launch wispr-fox at login</strong></span>
    </label>
  </div>

  <div class="behavior-block">
    <label class="check-row" title="Only the avatar and tray icon show on launch. Open this window via tray left-click or by double-clicking the avatar.">
      <input
        type="checkbox"
        checked={settings.s.open_silently}
        onchange={(e) => settings.set("open_silently", (e.currentTarget as HTMLInputElement).checked)}
      />
      <span><strong>Open silently</strong> — start in the tray, no main window</span>
    </label>
  </div>

  <h3>Audio cues</h3>
  <p class="lede">A short sound plays when recording starts and stops.</p>

  <label class="check-row">
    <input
      type="checkbox"
      checked={settings.s.cues_enabled}
      onchange={(e) => setCuesEnabled((e.currentTarget as HTMLInputElement).checked)}
    />
    <span>Play audio cues on record start / stop</span>
  </label>

  {#if settings.s.cues_enabled}
    <div class="sound-section">
      <div class="sound-section-head">
        <span class="sound-section-title">Cue sound</span>
        <button class="btn-secondary small" onclick={uploadSound}>+ Upload file</button>
      </div>
      <div class="sound-tiles">
        <button
          class="sound-tile"
          class:active={settings.s.start_sound === ""}
          onclick={() => setCueSound("")}
          title="System-default soft chirp"
        >
          <svg viewBox="0 0 24 24" width="22" height="22" aria-hidden="true">
            <path d="M 5 9 L 5 15 L 9 15 L 14 19 L 14 5 L 9 9 Z M 18 8 L 18 16" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round" stroke-linecap="round"/>
          </svg>
          <span class="sound-tile-label">Built-in tone</span>
        </button>
        {#each availableSounds as f (f)}
          <button
            class="sound-tile"
            class:active={settings.s.start_sound === f}
            onclick={() => setCueSound(f)}
            title="Click to select + preview"
          >
            <svg viewBox="0 0 24 24" width="22" height="22" aria-hidden="true">
              <path d="M 4 12 L 7 12 M 9 8 L 9 16 M 12 6 L 12 18 M 15 9 L 15 15 M 18 11 L 18 13" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
            </svg>
            <span class="sound-tile-label">{f}</span>
          </button>
        {/each}
      </div>
    </div>
  {/if}

  <div class="folder-hint">
    <div>Sound files live at:</div>
    <code>{soundsDir || "%APPDATA%\\com.wispr-fox.app\\sounds\\"}</code>
    <div class="folder-hint-actions">
      <button class="btn-secondary small" onclick={refreshSounds}>Refresh</button>
      <button class="btn-secondary small" onclick={async () => {
        try {
          await api.revealFolder("sounds");
        } catch (e) {
          flash(`Couldn't open: ${e}`);
        }
      }}>Open folder</button>
    </div>
  </div>

  <h3>Data retention</h3>
  <p class="lede">How long recordings stay on disk before automatic cleanup.</p>

  <div class="settings-card">
    <div class="field-block">
      <label for="retention-days">Retention: {settings.s.retention_days} day{settings.s.retention_days === 1 ? "" : "s"}</label>
      <input
        id="retention-days"
        type="range"
        min="1"
        max="90"
        value={settings.s.retention_days}
        oninput={(e) => setNumber("retention_days", Number((e.currentTarget as HTMLInputElement).value))}
      />
      <p class="hint">Recordings older than this are auto-deleted hourly. Lifetime stats are kept separately.</p>
    </div>

    <div class="field-block">
      <label for="retention-max-mb">Storage cap (MB)</label>
      <input
        id="retention-max-mb"
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
