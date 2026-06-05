<script lang="ts">
  // Appearance — floater skin + app theme.
  import { onMount } from "svelte";
  import { settings } from "$lib/settings-store.svelte";
  import { skinStore, setClippyWindowVisible, type Skin } from "$lib/skin-store.svelte";
  import { floaterScale, SCALE_MIN, SCALE_MAX, SCALE_PRESETS } from "$lib/floater-scale.svelte";
  import SkinIcon from "$lib/SkinIcon.svelte";

  type SkinOption = { id: Skin; label: string; desc: string };
  const SKIN_OPTIONS: SkinOption[] = [
    { id: "off",         label: "Off",       desc: "Hide the floating character entirely" },
    { id: "fox",         label: "Fox",       desc: "Watercolor fox mascot — the Foxy identity, default since v1.0" },
    { id: "stylized",    label: "Paperclip", desc: "Minimal stylised paperclip — dark outline, elephant ear, big eyes" },
    { id: "real-clippy", label: "Clippy",    desc: "The actual Microsoft Clippy with original animations" },
    { id: "cat",         label: "Desk Cat",  desc: "Sleepy charcoal cat with green slit-pupil eyes — curls up idle, perks up to help, typing paws" },
    { id: "cat-lab",     label: "Cat (lab)", desc: "Experimental charcoal cat — thin white edge highlights, lighter belly + paws, defined neck, mouth and tail for legibility over dark wallpapers" },
  ];

  const THEME_OPTIONS = [
    { id: "auto",  label: "Auto",  desc: "Follow your system theme" },
    { id: "light", label: "Light", desc: "Always light" },
    { id: "dark",  label: "Dark",  desc: "Always dark" },
    { id: "retro", label: "Retro", desc: "Warm cream tones, vintage feel" },
  ] as const;

  async function pickSkin(s: Skin) {
    await skinStore.set(s);
    await setClippyWindowVisible(s !== "off");
  }

  function onScaleInput(e: Event) {
    const v = Number.parseFloat((e.currentTarget as HTMLInputElement).value);
    floaterScale.set(v);
  }

  onMount(() => {
    skinStore.subscribe();
    floaterScale.subscribe();
  });
</script>

<section>
  <h2>Appearance</h2>
  <p class="lede">Floating character and app-wide theme.</p>

  <h3>Floater character</h3>
  <p class="lede">The floating animated character that reacts to your dictation. Off = hidden window.</p>
  <div class="skin-tiles">
    {#each SKIN_OPTIONS as opt (opt.id)}
      <button
        class="skin-tile"
        class:active={skinStore.current === opt.id}
        onclick={() => pickSkin(opt.id)}
        title={opt.desc}
      >
        <div class="skin-tile-preview">
          <SkinIcon skin={opt.id} size={60} />
        </div>
        <div class="skin-tile-label">{opt.label}</div>
        {#if skinStore.current === opt.id}
          <div class="skin-tile-check">✓</div>
        {/if}
      </button>
    {/each}
  </div>

  <h3>Floater size</h3>
  <p class="lede">Scale the floating character and its window together. Smaller frees up screen space on 13″ laptops; larger is easier to see. Applies live and sticks across restarts.</p>
  <div class="scale-control">
    <input
      class="scale-slider"
      type="range"
      min={SCALE_MIN}
      max={SCALE_MAX}
      step="0.05"
      value={floaterScale.current}
      oninput={onScaleInput}
      aria-label="Floater size"
    />
    <span class="scale-value">{Math.round(floaterScale.current * 100)}%</span>
  </div>
  <div class="scale-presets">
    {#each SCALE_PRESETS as p (p.id)}
      <button
        class="preset-chip"
        class:active={Math.abs(floaterScale.current - p.value) < 0.001}
        onclick={() => floaterScale.set(p.value)}
      >
        {p.id === "s" ? "Small" : p.id === "m" ? "Medium" : "Large"}
      </button>
    {/each}
  </div>

  <h3>Theme</h3>
  <p class="lede">App-wide colour scheme. (Retro warm theme is in progress — placeholder for now.)</p>
  <div class="radio-grid">
    {#each THEME_OPTIONS as opt (opt.id)}
      <button
        class="radio-card"
        class:active={settings.s.theme === opt.id}
        onclick={() => settings.set("theme", opt.id)}
      >
        <div class="radio-card-head">
          <span class="radio-dot">{settings.s.theme === opt.id ? "●" : "○"}</span>
          <span class="radio-label">{opt.label}</span>
        </div>
        <div class="radio-desc">{opt.desc}</div>
      </button>
    {/each}
  </div>
</section>

<style>
  .scale-control {
    display: flex;
    align-items: center;
    gap: 14px;
    max-width: 420px;
    margin-bottom: 10px;
  }
  .scale-slider {
    flex: 1 1 auto;
    accent-color: var(--accent);
    cursor: pointer;
  }
  .scale-value {
    flex: 0 0 auto;
    min-width: 44px;
    text-align: right;
    font-variant-numeric: tabular-nums;
    font-weight: 600;
    color: var(--text-primary);
  }
  .scale-presets {
    display: flex;
    gap: 8px;
  }
  .preset-chip {
    padding: 5px 14px;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 8px;
    color: var(--text-secondary);
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: all 120ms ease;
  }
  .preset-chip:hover {
    border-color: var(--accent);
    color: var(--accent);
  }
  .preset-chip.active {
    border-color: var(--accent);
    background: var(--accent-fade);
    color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent) inset;
  }
</style>
