<script lang="ts">
  // /settings — bare-route redirect into the first sub-page.
  // Settings was refactored from a 2164-line single-file god-component
  // into 5 sub-routes (providers, dictation, general, appearance, data)
  // each with its own URL. Hitting /settings without a sub-path lands
  // here and bounces to Voice, the daily setup and shortcut surface.
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";

  onMount(() => {
    goto("/settings/dictation", { replaceState: true });
  });
</script>
