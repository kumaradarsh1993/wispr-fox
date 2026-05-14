<script lang="ts">
  // General — startup behaviour + audio cues.
  // Two unrelated-but-small section pairs that were each too small to
  // deserve their own sub-route.
  import { onMount } from "svelte";
  import { api } from "$lib/api";
  import { settings } from "$lib/settings-store.svelte";
  import { flash } from "$lib/settings-toast.svelte";

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
  let availableSounds = $state<string[]>([]);
  let soundsDir = $state<string>("");
  let soundSync = $state(false);
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

  async function setStartSound(name: string) {
    await settings.set("start_sound", name);
    if (soundSync) {
      await settings.set("stop_sound", name);
    }
    await api.configureCues(name, soundSync ? name : settings.s.stop_sound, settings.s.cues_enabled);
    flash("Start sound updated");
  }
  async function setStopSound(name: string) {
    if (soundSync) return; // ignored when synced
    await settings.set("stop_sound", name);
    await api.configureCues(settings.s.start_sound, name, settings.s.cues_enabled);
    flash("Stop sound updated");
  }
  async function setCuesEnabled(on: boolean) {
    await settings.set("cues_enabled", on);
    await api.configureCues(settings.s.start_sound, settings.s.stop_sound, on);
  }
  function toggleSync() {
    soundSync = !soundSync;
    if (soundSync && settings.s.start_sound) {
      setStartSound(settings.s.start_sound); // mirror to stop
    }
  }

  // Preview a sound by name (or null = built-in beep — falls back to system).
  async function previewSound(name: string) {
    if (previewAudio) {
      previewAudio.pause();
      previewAudio = null;
    }
    if (!name) return;
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
      alert(`Upload failed: ${e}`);
    }
  }

  onMount(() => {
    refreshSounds();
  });
</script>

<section>
  <h2>General</h2>
  <p class="lede">Startup behaviour and audio cues.</p>

  <h3>Startup</h3>
  <p class="lede">What happens when wispr-fox launches — automatic or manual, loud or silent.</p>

  <div class="behavior-block">
    <label class="check-row">
      <input
        type="checkbox"
        checked={settings.s.autostart}
        onchange={(e) => syncAutostart((e.currentTarget as HTMLInputElement).checked)}
      />
      <span><strong>Launch wispr-fox at login</strong> — start automatically when you sign in to Windows. The tray icon + Clippy floater appear; no main window unless you ask.</span>
    </label>
    <p class="hint">Registers a Windows startup entry under the current user (no admin needed). Toggle off any time to remove it.</p>
  </div>

  <div class="behavior-block">
    <label class="check-row">
      <input
        type="checkbox"
        checked={settings.s.open_silently}
        onchange={(e) => settings.set("open_silently", (e.currentTarget as HTMLInputElement).checked)}
      />
      <span><strong>Open silently</strong> — on launch, only Clippy + the tray icon show. Open this Settings/History window via tray (left-click) or double-click on Clippy.</span>
    </label>
    <p class="hint">On by default. Turn off if you want the main window to pop open every time the app starts.</p>
  </div>

  <h3 style="margin-top: 32px;">Audio cues</h3>
  <p class="lede">Short sounds that play when recording starts and stops. Click any tile to preview it.</p>

  <label class="check-row">
    <input
      type="checkbox"
      checked={settings.s.cues_enabled}
      onchange={(e) => setCuesEnabled((e.currentTarget as HTMLInputElement).checked)}
    />
    <span>Play audio cues on record start / stop</span>
  </label>

  {#if settings.s.cues_enabled}
    <label class="check-row" style="margin-top: 12px;">
      <input type="checkbox" checked={soundSync} onchange={toggleSync} />
      <span>Sync start + stop sounds — use the same file for both</span>
    </label>

    <div class="sound-section">
      <div class="sound-section-head">
        <span class="sound-section-title">Start sound</span>
        <button class="btn-secondary small" onclick={uploadSound}>+ Upload file</button>
      </div>
      <div class="sound-tiles">
        <button
          class="sound-tile"
          class:active={settings.s.start_sound === ""}
          onclick={() => setStartSound("")}
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
            onclick={() => { setStartSound(f); previewSound(f); }}
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

    <div class="sound-section" class:disabled={soundSync}>
      <div class="sound-section-head">
        <span class="sound-section-title">
          Stop sound
          {#if soundSync}<span class="sound-locked">— synced with start</span>{/if}
        </span>
      </div>
      <div class="sound-tiles">
        <button
          class="sound-tile"
          class:active={(soundSync ? settings.s.start_sound : settings.s.stop_sound) === ""}
          onclick={() => setStopSound("")}
          disabled={soundSync}
        >
          <svg viewBox="0 0 24 24" width="22" height="22" aria-hidden="true">
            <path d="M 5 9 L 5 15 L 9 15 L 14 19 L 14 5 L 9 9 Z M 18 8 L 18 16" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round" stroke-linecap="round"/>
          </svg>
          <span class="sound-tile-label">Built-in tone</span>
        </button>
        {#each availableSounds as f (f)}
          <button
            class="sound-tile"
            class:active={(soundSync ? settings.s.start_sound : settings.s.stop_sound) === f}
            onclick={() => { setStopSound(f); previewSound(f); }}
            disabled={soundSync}
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
        const { openPath } = await import("@tauri-apps/plugin-opener");
        await openPath(soundsDir);
      }}>Open folder</button>
    </div>
  </div>
</section>
