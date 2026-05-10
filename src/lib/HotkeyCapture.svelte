<script lang="ts">
  // Capture a keybind by clicking "Record" → pressing the desired combo. Falls
  // back to a plain text input for power users who want to type accelerator
  // strings directly. Output format matches Tauri's Shortcut::from_str:
  // e.g. "CommandOrControl+Super+Space".

  let { value = $bindable(""), label = "Hotkey" } = $props<{
    value: string;
    label?: string;
  }>();

  let capturing = $state(false);
  let captured = $state("");

  function startCapture() {
    capturing = true;
    captured = "";
    window.addEventListener("keydown", onKey, true);
  }

  function stopCapture() {
    capturing = false;
    window.removeEventListener("keydown", onKey, true);
  }

  function onKey(e: KeyboardEvent) {
    e.preventDefault();
    e.stopPropagation();

    if (e.key === "Escape") {
      stopCapture();
      return;
    }

    const mods: string[] = [];
    if (e.ctrlKey) mods.push("CommandOrControl");
    if (e.altKey) mods.push("Alt");
    if (e.shiftKey) mods.push("Shift");
    if (e.metaKey) mods.push("Super");

    const k = e.key;
    // Skip while only modifiers are down — wait for the main key.
    if (k === "Control" || k === "Alt" || k === "Shift" || k === "Meta") {
      captured = mods.join("+") + (mods.length ? "+…" : "…");
      return;
    }

    // Normalize key names to Tauri/Electron accelerator format.
    const keyName = normalizeKey(k);
    if (!keyName) return;

    const combo = [...mods, keyName].join("+");
    captured = combo;
    value = combo;
    stopCapture();
  }

  function normalizeKey(k: string): string | null {
    if (k.length === 1) {
      // a..z, 0..9, etc.
      return k.toUpperCase();
    }
    const map: Record<string, string> = {
      " ": "Space",
      Enter: "Enter",
      Tab: "Tab",
      Backspace: "Backspace",
      ArrowUp: "Up",
      ArrowDown: "Down",
      ArrowLeft: "Left",
      ArrowRight: "Right",
    };
    if (map[k]) return map[k];
    if (/^F\d{1,2}$/.test(k)) return k;
    return null;
  }

  function display(combo: string) {
    if (!combo) return "(none)";
    return combo
      .replace(/CommandOrControl/g, "Ctrl")
      .replace(/Super/g, "Win")
      .replace(/\+/g, " + ");
  }
</script>

<label class="block">
  <span class="text-sm font-medium text-gray-700">{label}</span>
  <div class="mt-1 flex gap-2">
    <input
      type="text"
      class="flex-1 px-2 py-1 rounded border text-sm font-mono"
      bind:value
      placeholder="CommandOrControl+Super+Space"
    />
    {#if capturing}
      <button
        type="button"
        onclick={stopCapture}
        class="px-3 py-1 rounded border text-sm bg-amber-50 text-amber-800"
      >
        {captured || "Press keys… (Esc to cancel)"}
      </button>
    {:else}
      <button
        type="button"
        onclick={startCapture}
        class="px-3 py-1 rounded border text-sm hover:bg-gray-50"
      >
        Record
      </button>
    {/if}
  </div>
  <span class="text-xs text-gray-500 mt-1 block">
    Current: <span class="font-mono">{display(value)}</span>
  </span>
</label>
