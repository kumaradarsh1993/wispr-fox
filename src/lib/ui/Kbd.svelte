<script lang="ts">
  /**
   * Renders a hotkey as keycaps: <Kbd combo="Alt+Space" /> → ⌥ Space on
   * macOS, Alt Space elsewhere. Accepts the app's stored "Mod+Key" strings.
   */
  let { combo, size = "sm" }: { combo: string; size?: "sm" | "md" } = $props();

  const isMac =
    typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform ?? "");

  const glyph: Record<string, string> = isMac
    ? { Cmd: "⌘", Meta: "⌘", Super: "⌘", Ctrl: "⌃", Control: "⌃", Alt: "⌥", Option: "⌥", Shift: "⇧", Enter: "↩", Return: "↩", Esc: "⎋", Escape: "⎋", Space: "Space", Tab: "⇥", Backspace: "⌫" }
    : { Cmd: "Win", Meta: "Win", Super: "Win", Ctrl: "Ctrl", Control: "Ctrl", Alt: "Alt", Option: "Alt", Shift: "Shift", Enter: "Enter", Return: "Enter", Esc: "Esc", Escape: "Esc", Space: "Space", Tab: "Tab", Backspace: "Backspace" };

  const keys = $derived(
    combo
      .split("+")
      .map((k) => k.trim())
      .filter(Boolean)
      .map((k) => glyph[k] ?? k),
  );
</script>

<span class="kbd" data-size={size} aria-label={combo}>
  {#each keys as k, i (i)}
    <kbd>{k}</kbd>
  {/each}
</span>

<style>
  .kbd {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    vertical-align: middle;
  }
  kbd {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 1.6em;
    padding: 0 6px;
    border: 1px solid var(--border);
    border-bottom-width: 2px;
    border-radius: 6px;
    background: var(--bg-card);
    color: var(--text-primary);
    font-family: inherit;
    font-weight: 600;
    line-height: 1.6;
  }
  .kbd[data-size="sm"] kbd {
    font-size: var(--fs-xs);
  }
  .kbd[data-size="md"] kbd {
    font-size: var(--fs-sm);
    padding: 1px 8px;
  }
</style>
