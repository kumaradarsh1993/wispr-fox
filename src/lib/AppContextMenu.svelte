<script lang="ts">
  // One app-wide right-click menu for the main window.
  //
  // Without this you get WebView2's own menu — Back, Reload, Save as, Print,
  // Inspect — on every surface. Those are browser commands leaking through the
  // fact that this happens to be a webview; none of them mean anything in a
  // dictation app, and "Save as…" on a transcript card is actively confusing.
  //
  // The rule: right-click always suppresses the browser menu, and shows one of
  // ours ONLY where there is something real to offer. Right-clicking empty
  // chrome shows nothing at all, which is the correct answer — a menu with one
  // greyed-out item is worse than no menu.
  //
  // The floater (`/clippy`) is deliberately excluded: it has its own
  // FloaterContextMenu with skin/scale/position controls, and two handlers on
  // one event would fight.

  import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";

  type Item = {
    label: string;
    action: () => void | Promise<void>;
    /** Shown right-aligned, e.g. "Ctrl+C". Display only — the real binding is
     *  the webview's own, we never handle these keys ourselves. */
    hint?: string;
  };

  let open = $state(false);
  let x = $state(0);
  let y = $state(0);
  let items = $state<Item[]>([]);
  let menuEl = $state<HTMLDivElement | null>(null);

  const MENU_W = 190;
  /** Rough per-item height + padding. Used ONLY for the first paint, so the
   *  menu doesn't visibly jump; the real clamp measures the element (see the
   *  effect below). An estimate alone was 6px short of the rendered height and
   *  let the menu hang 2px past the bottom of an 800px viewport. */
  const ITEM_H = 32;
  const PAD = 10;

  function isEditable(el: Element | null): el is HTMLInputElement | HTMLTextAreaElement {
    if (!el) return false;
    const tag = el.tagName;
    if (tag === "TEXTAREA") return true;
    if (tag === "INPUT") {
      const t = (el as HTMLInputElement).type;
      // Only text-ish inputs have anything to cut/paste. A checkbox or a range
      // slider does not.
      return ["text", "search", "url", "email", "password", "tel", "number", ""].includes(t);
    }
    return false;
  }

  function selectionText(): string {
    return (window.getSelection()?.toString() ?? "").trim();
  }

  /** Insert text at the caret and tell Svelte about it.
   *
   *  `setRangeText` does NOT fire an `input` event, so without the explicit
   *  dispatch a paste would visibly land in the box and then be silently
   *  discarded on the next re-render, because `bind:value` never heard about
   *  it. That failure looks exactly like "paste is broken". */
  function insertAtCaret(el: HTMLInputElement | HTMLTextAreaElement, text: string) {
    const start = el.selectionStart ?? el.value.length;
    const end = el.selectionEnd ?? el.value.length;
    el.setRangeText(text, start, end, "end");
    el.dispatchEvent(new Event("input", { bubbles: true }));
    el.focus();
  }

  function buildItems(target: Element | null): Item[] {
    const sel = selectionText();
    const editable = isEditable(target);

    if (editable) {
      const el = target as HTMLInputElement | HTMLTextAreaElement;
      const hasSel = (el.selectionEnd ?? 0) > (el.selectionStart ?? 0);
      const selected = hasSel ? el.value.slice(el.selectionStart ?? 0, el.selectionEnd ?? 0) : "";
      const out: Item[] = [];
      if (hasSel && !el.readOnly && !el.disabled) {
        out.push({
          label: "Cut",
          hint: "Ctrl+X",
          action: async () => {
            await writeText(selected);
            insertAtCaret(el, "");
          },
        });
      }
      if (hasSel) {
        out.push({ label: "Copy", hint: "Ctrl+C", action: () => writeText(selected) });
      }
      if (!el.readOnly && !el.disabled) {
        out.push({
          label: "Paste",
          hint: "Ctrl+V",
          action: async () => {
            const t = await readText();
            if (t) insertAtCaret(el, t);
          },
        });
      }
      if (el.value.length > 0) {
        out.push({ label: "Select all", hint: "Ctrl+A", action: () => el.select() });
      }
      return out;
    }

    // Read-only surface: the only honest offer is copying what is selected.
    if (sel) {
      return [{ label: "Copy", hint: "Ctrl+C", action: () => writeText(sel) }];
    }
    return [];
  }

  function onContextMenu(e: MouseEvent) {
    // Always kill the webview menu, even when we have nothing of our own to
    // show. Suppressing it is the point; our menu is the bonus.
    e.preventDefault();

    const next = buildItems(e.target as Element | null);
    if (next.length === 0) {
      open = false;
      return;
    }

    // Flip rather than overflow. A menu opened near the right or bottom edge
    // must not push the page into scrolling.
    const h = next.length * ITEM_H + PAD;
    x = Math.min(e.clientX, Math.max(4, window.innerWidth - MENU_W - 4));
    y = Math.min(e.clientY, Math.max(4, window.innerHeight - h - 4));
    items = next;
    open = true;
  }

  async function run(item: Item) {
    open = false;
    try {
      await item.action();
    } catch (err) {
      // Clipboard access can fail (permissions, empty clipboard). Never throw
      // out of a menu click — the menu is already gone by then.
      console.warn("context menu action failed", err);
    }
  }

  // Correct the placement against the MEASURED box once it has rendered.
  //
  // Writes to the DOM, never to `x`/`y`. Assigning the reactive state it reads
  // here would be the classic self-triggering `$effect` — which in this
  // codebase does not just loop, it aborts the component and silently kills
  // every click in the app. Styling the node directly has no such feedback.
  $effect(() => {
    const el = menuEl;
    if (!el || !open) return;
    // Depend on position + item count so a re-open re-clamps.
    void x;
    void y;
    void items.length;
    const r = el.getBoundingClientRect();
    const maxLeft = Math.max(4, window.innerWidth - r.width - 4);
    const maxTop = Math.max(4, window.innerHeight - r.height - 4);
    el.style.left = `${Math.min(x, maxLeft)}px`;
    el.style.top = `${Math.min(y, maxTop)}px`;
  });

  function dismiss() {
    open = false;
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") dismiss();
  }
</script>

<svelte:window
  oncontextmenu={onContextMenu}
  onkeydown={onKeydown}
  onresize={dismiss}
  onblur={dismiss}
/>

{#if open}
  <!-- Backdrop swallows the dismissing click so it doesn't also activate
       whatever is underneath the menu. -->
  <div
    class="ctx-backdrop"
    role="presentation"
    onpointerdown={(e) => { e.preventDefault(); dismiss(); }}
    oncontextmenu={(e) => { e.preventDefault(); dismiss(); }}
    onwheel={dismiss}
  ></div>
  <div
    class="ctx-menu"
    role="menu"
    tabindex="-1"
    bind:this={menuEl}
    style="left: {x}px; top: {y}px;"
  >
    {#each items as item (item.label)}
      <button class="ctx-item" role="menuitem" onclick={() => run(item)}>
        <span>{item.label}</span>
        {#if item.hint}<span class="ctx-hint">{item.hint}</span>{/if}
      </button>
    {/each}
  </div>
{/if}

<style>
  .ctx-backdrop {
    position: fixed;
    inset: 0;
    z-index: 9998;
  }

  .ctx-menu {
    position: fixed;
    z-index: 9999;
    min-width: 190px;
    padding: 4px;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 9px;
    box-shadow: 0 10px 28px rgba(60, 38, 12, 0.18);
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .ctx-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 18px;
    width: 100%;
    padding: 6px 10px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--text-primary);
    font-family: inherit;
    font-size: 12.5px;
    text-align: left;
    cursor: pointer;
  }
  .ctx-item:hover {
    background: var(--accent-fade);
    color: var(--accent);
  }

  .ctx-hint {
    font-size: 10.5px;
    color: var(--text-secondary);
    font-variant-numeric: tabular-nums;
  }
</style>
