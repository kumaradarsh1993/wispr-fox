<script lang="ts">
  // Appearance - avatar skin + app theme.
  import { onMount } from "svelte";
  import { settings } from "$lib/settings-store.svelte";
  import { skinStore, setClippyWindowVisible, type Skin } from "$lib/skin-store.svelte";
  import { floaterScale, floaterDebug, floaterFixedBox, SCALE_MIN, SCALE_MAX, SCALE_PRESETS } from "$lib/floater-scale.svelte";
  import SkinIcon from "$lib/SkinIcon.svelte";

  type SkinOption = { id: Skin; label: string; desc: string };
  const SKIN_OPTIONS: SkinOption[] = [
    { id: "off",         label: "Off",       desc: "Hide the floating character entirely" },
    { id: "codex-fox",   label: "Codex Fox", desc: "High-fidelity 2.5D raster fox companion with Codex-blue glow and state-specific poses" },
    { id: "fox",         label: "Fox",       desc: "Watercolor fox mascot — the Foxy identity, default since v1.0" },
    { id: "stylized",    label: "Paperclip", desc: "Minimal stylised paperclip — dark outline, elephant ear, big eyes" },
    { id: "real-clippy", label: "Clippy",    desc: "The actual Microsoft Clippy with original animations" },
    { id: "cat",         label: "Desk Cat",  desc: "Sleepy charcoal cat with green slit-pupil eyes — curls up idle, perks up to help, typing paws" },
    { id: "duo",         label: "Khaumani & Indy", desc: "The two-cat team — a serene white cat loafing on the console supervises while an orange tabby kitten does the actual typing. Paw bump on every successful paste" },
    { id: "oru-gujia",   label: "Oru & Gujia", desc: "High-fidelity personal duo based on Oru the orange tabby and Gujia the white supervisor" },
    { id: "spark-buddy", label: "Spark Buddy", desc: "Original electric companion with polished raster poses, teal glow, and celebratory sparks" },
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
    floaterDebug.subscribe();
    floaterFixedBox.subscribe();
  });

  const BOX_OPTIONS = [
    {
      fixed: false,
      label: "Compact",
      desc: "Window hugs the character and expands upward only while it's speaking. Frees the screen area above the head — with a brief fade on each expand/contract.",
    },
    {
      fixed: true,
      label: "Full box (classic)",
      desc: "Window always reserves room for the speech bubble, like v1.3. Never resizes during dictation, so there is zero transition — at the cost of an invisible band above the character that covers whatever is behind it.",
    },
  ] as const;
</script>

<section>
  <h2>Appearance</h2>
  <p class="lede">Avatar, window behaviour, and app-wide theme.</p>

  <h3>Avatar</h3>
  <p class="lede">The floating character that reacts to your dictation.</p>
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

  <h3>Avatar size</h3>
  <p class="lede">Scale the character and its window together.</p>
  <div class="scale-control">
    <input
      class="scale-slider"
      type="range"
      min={SCALE_MIN}
      max={SCALE_MAX}
      step="0.05"
      value={floaterScale.current}
      oninput={onScaleInput}
      aria-label="Avatar size"
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

  <h3>Avatar window</h3>
  <p class="lede">How much screen the avatar's invisible window claims around the character.</p>
  <div class="radio-grid">
    {#each BOX_OPTIONS as opt (opt.fixed)}
      <button
        class="radio-card"
        class:active={floaterFixedBox.current === opt.fixed}
        onclick={() => floaterFixedBox.set(opt.fixed)}
      >
        <div class="radio-card-head">
          <span class="radio-dot">{floaterFixedBox.current === opt.fixed ? "●" : "○"}</span>
          <span class="radio-label">{opt.label}</span>
        </div>
        <div class="radio-desc">{opt.desc}</div>
      </button>
    {/each}
  </div>

  <div class="debug-row">
    <label class="debug-toggle">
      <input
        type="checkbox"
        checked={floaterDebug.current}
        onchange={(e) => floaterDebug.set((e.currentTarget as HTMLInputElement).checked)}
      />
      <span>Show avatar debug overlay</span>
    </label>
    <p class="debug-hint">Draws the window bounds and a live size readout on the character. For tuning only.</p>
  </div>

  <h3>Theme</h3>
  <p class="lede">App-wide colour scheme.</p>
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
  .debug-row {
    margin-top: 14px;
  }
  .debug-toggle {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: var(--text-primary);
    cursor: pointer;
  }
  .debug-toggle input {
    width: 15px;
    height: 15px;
    accent-color: var(--accent);
    cursor: pointer;
  }
  .debug-hint {
    margin: 4px 0 0 23px;
    font-size: 11px;
    color: var(--text-secondary);
    max-width: 460px;
    line-height: 1.4;
  }
</style>
