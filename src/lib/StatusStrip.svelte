<script lang="ts">
  /**
   * Home status strip (docs/DESIGN_v3.5.md §4). One quiet line under the
   * header that answers "what happens when I press the key right now?" and
   * lets the advanced user change it without leaving Home. Replaces the
   * sidebar's Quick-controls card.
   *
   *   Listening with Nova-3 · Polish: On · Fox: While dictating · ⌥Space
   */
  import { onMount } from "svelte";
  import { settings } from "./settings-store.svelte";
  import { skinStore } from "./skin-store.svelte";
  import {
    avatarVisibility,
    applyVisibilityWindow,
    type AvatarVisibility,
  } from "./avatar-visibility.svelte";
  import { STT_PROVIDERS, sttModelsFor } from "./provider-options";
  import { listen } from "@tauri-apps/api/event";
  import SkinIcon from "./SkinIcon.svelte";
  import Kbd from "./ui/Kbd.svelte";

  let busy = $state(false);
  let visOpen = $state(false);

  onMount(() => {
    skinStore.subscribe();
    avatarVisibility.subscribe();
    let un: (() => void) | undefined;
    listen<string>("wispr:state", (e) => (busy = e.payload !== "idle")).then((u) => (un = u));
    return () => un?.();
  });

  const providerLabel = $derived(
    STT_PROVIDERS.find((p) => p.id === settings.s.stt_provider)?.label ?? settings.s.stt_provider,
  );
  const modelLabel = $derived(
    sttModelsFor(settings.s.stt_provider).find((m) => m.id === settings.s.stt_model)?.label ?? "",
  );

  const VIS: { id: AvatarVisibility; label: string }[] = [
    { id: "always", label: "Always" },
    { id: "auto", label: "While dictating" },
    { id: "hidden", label: "Hidden" },
  ];
  const visLabel = $derived(VIS.find((v) => v.id === avatarVisibility.current)?.label ?? "");

  async function pickVis(v: AvatarVisibility) {
    visOpen = false;
    await avatarVisibility.set(v);
    await applyVisibilityWindow(v);
  }
</script>

<div class="strip" aria-label="Current setup">
  <a class="seg link" href="/settings/providers" title="Change the transcription engine">
    <span class="k">Listening with</span>
    <strong>{modelLabel || providerLabel}</strong>
  </a>

  <span class="dot" aria-hidden="true"></span>

  <label class="seg toggle" title="Fix spelling, punctuation and filler words after Transcribe">
    <span class="k">Polish</span>
    <input
      type="checkbox"
      checked={settings.s.auto_clean_in_light}
      disabled={busy}
      onchange={(e) => settings.set("auto_clean_in_light", (e.currentTarget as HTMLInputElement).checked)}
    />
    <strong>{settings.s.auto_clean_in_light ? "On" : "Off"}</strong>
  </label>

  <span class="dot" aria-hidden="true"></span>

  <div class="seg pop">
    <button class="popbtn" type="button" onclick={() => (visOpen = !visOpen)} aria-expanded={visOpen} title="When the avatar shows">
      <span class="skin"><SkinIcon skin={skinStore.current} size={16} /></span>
      <span class="k">Fox</span>
      <strong>{visLabel}</strong>
    </button>
    {#if visOpen}
      <div class="menu" role="menu">
        {#each VIS as v (v.id)}
          <button role="menuitemradio" aria-checked={avatarVisibility.current === v.id} onclick={() => pickVis(v.id)}>{v.label}</button>
        {/each}
        <a href="/settings/appearance" onclick={() => (visOpen = false)}>Change avatar…</a>
      </div>
    {/if}
  </div>

  <span class="grow"></span>

  <a class="seg link hk" href="/settings/dictation" title="Change shortcuts">
    <span class="k">Dictate</span>
    <Kbd combo={settings.s.light_hotkey} />
  </a>
</div>

{#if visOpen}
  <button class="backdrop" type="button" aria-label="Close menu" onclick={() => (visOpen = false)}></button>
{/if}

<style>
  .strip {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    min-height: 32px;
    padding: 0 var(--sp-1);
    color: var(--text-secondary);
    font-size: var(--fs-sm);
    line-height: 1;
    position: relative;
    z-index: 3;
  }
  .seg {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 5px 8px;
    border-radius: var(--radius-sm);
    color: inherit;
    text-decoration: none;
    white-space: nowrap;
  }
  .seg strong {
    color: var(--text-primary);
    font-weight: 600;
  }
  .k {
    color: var(--text-muted);
  }
  .link:hover,
  .toggle:hover,
  .popbtn:hover {
    background: var(--bg-subtle);
    color: var(--text-primary);
  }
  .toggle {
    cursor: pointer;
  }
  .toggle input {
    appearance: none;
    width: 26px;
    height: 15px;
    margin: 0;
    border-radius: var(--radius-pill);
    background: var(--border);
    position: relative;
    transition: background var(--motion-fast) var(--ease-standard);
  }
  .toggle input::after {
    content: "";
    position: absolute;
    top: 2px;
    left: 2px;
    width: 11px;
    height: 11px;
    border-radius: 50%;
    background: #fff;
    transition: transform var(--motion-fast) var(--ease-standard);
  }
  .toggle input:checked {
    background: var(--accent);
  }
  .toggle input:checked::after {
    transform: translateX(11px);
  }
  .toggle input:disabled {
    opacity: 0.5;
  }
  .pop {
    position: relative;
    padding: 0;
  }
  .popbtn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 5px 8px;
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    color: inherit;
    font: inherit;
  }
  .popbtn strong {
    color: var(--text-primary);
    font-weight: 600;
  }
  .skin {
    display: inline-grid;
    place-items: center;
    width: 16px;
    height: 16px;
  }
  .menu {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    min-width: 170px;
    display: grid;
    padding: 4px;
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-md);
    z-index: 5;
  }
  .menu button,
  .menu a {
    display: block;
    padding: 7px 10px;
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-primary);
    font: inherit;
    font-size: var(--fs-sm);
    text-align: left;
    text-decoration: none;
  }
  .menu button:hover,
  .menu a:hover {
    background: var(--bg-subtle);
  }
  .menu button[aria-checked="true"] {
    color: var(--accent-pressed);
    font-weight: 600;
  }
  .menu a {
    margin-top: 2px;
    border-top: 1px solid var(--border-subtle);
    border-radius: 0 0 var(--radius-sm) var(--radius-sm);
    color: var(--text-secondary);
  }
  .dot {
    width: 3px;
    height: 3px;
    border-radius: 50%;
    background: var(--text-muted);
    opacity: 0.6;
  }
  .grow {
    flex: 1;
  }
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 2;
    border: 0;
    background: transparent;
    cursor: default;
  }
  @container (max-width: 640px) {
    .hk {
      display: none;
    }
  }
</style>
