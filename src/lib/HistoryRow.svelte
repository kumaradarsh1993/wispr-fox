<script lang="ts">
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { history } from "./history-store.svelte";
  import type { Recording } from "./api";

  let { rec } = $props<{ rec: Recording }>();

  function timeShort(iso: string): string {
    try {
      const d = new Date(iso);
      const today = new Date();
      const sameDay = d.toDateString() === today.toDateString();
      return sameDay
        ? d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
        : d.toLocaleString([], {
            month: "short",
            day: "numeric",
            hour: "2-digit",
            minute: "2-digit",
          });
    } catch {
      return iso;
    }
  }

  function durationShort(ms: number): string {
    if (!ms) return "—";
    const s = Math.round(ms / 1000);
    if (s < 60) return `${s}s`;
    const m = Math.floor(s / 60);
    return `${m}m ${s % 60}s`;
  }

  function preview(): string {
    return rec.cleaned_text || rec.transcript || "(no transcript)";
  }

  async function copyText() {
    await writeText(preview());
  }

  async function remove() {
    if (!confirm("Delete this recording?")) return;
    await history.remove(rec.id);
  }
</script>

<tr class="border-b last:border-0 hover:bg-gray-50">
  <td class="px-2 py-1.5 text-xs text-gray-500 whitespace-nowrap">
    {timeShort(rec.created_at)}
  </td>
  <td class="px-2 py-1.5 text-xs text-gray-500 whitespace-nowrap">
    {durationShort(rec.duration_ms)}
  </td>
  <td class="px-2 py-1.5 text-xs">
    <span
      class="px-1.5 py-0.5 rounded {rec.mode === 'advanced'
        ? 'bg-purple-100 text-purple-700'
        : 'bg-blue-100 text-blue-700'}"
    >
      {rec.mode}
    </span>
  </td>
  <td class="px-2 py-1.5 text-sm text-gray-800 max-w-md truncate" title={preview()}>
    {preview()}
    {#if rec.status === "error"}
      <span class="ml-1 text-red-600 text-xs">✗ {rec.error}</span>
    {:else if rec.clippy_note}
      <span class="ml-1 text-amber-600 text-xs">({rec.clippy_note})</span>
    {/if}
  </td>
  <td class="px-2 py-1.5 text-xs whitespace-nowrap">
    <button class="px-1.5 py-0.5 hover:underline" onclick={copyText}>📋 copy</button>
    <button class="px-1.5 py-0.5 hover:underline text-red-600" onclick={remove}>🗑</button>
  </td>
</tr>
