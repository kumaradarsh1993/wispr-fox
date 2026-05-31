<script lang="ts">
  // Appearance — floater skin + app theme.
  import { onMount } from "svelte";
  import { settings } from "$lib/settings-store.svelte";
  import { skinStore, setClippyWindowVisible, type Skin } from "$lib/skin-store.svelte";
  import SkinIcon from "$lib/SkinIcon.svelte";

  type SkinOption = { id: Skin; label: string; desc: string };
  const SKIN_OPTIONS: SkinOption[] = [
    { id: "off",         label: "Off",       desc: "Hide the floating character entirely" },
    { id: "fox",         label: "Fox",       desc: "Watercolor fox mascot — the Foxy identity, default since v1.0" },
    { id: "stylized",    label: "Paperclip", desc: "Minimal stylised paperclip — dark outline, elephant ear, big eyes" },
    { id: "real-clippy", label: "Clippy",    desc: "The actual Microsoft Clippy with original animations" },
    { id: "duck",        label: "Duck",      desc: "Rubber duck debugging buddy — bobbing on water, squeaky and helpful" },
    { id: "cat",         label: "Cat",       desc: "Sleepy desk cat — curls up idle, perks up to help, typing paws" },
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

  onMount(() => {
    skinStore.subscribe();
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
