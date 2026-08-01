<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { api } from "$lib/api";

  onMount(async () => {
    // Home is gone — always route to History (the working pane) unless
    // first-launch needs onboarding for the API key.
    try {
      const secrets = await api.checkSecrets();
      const onboardingHandled = localStorage.getItem("wispr.onboarding.field-v1") !== null;
      if (!secrets.any_stt && !onboardingHandled) {
        goto("/onboarding");
        return;
      }
    } catch {}
    goto("/history");
  });
</script>

<p class="redirecting">Redirecting…</p>

<style>
  .redirecting {
    padding: 20px;
    color: #86868b;
    font-size: 13px;
    font-family: ui-sans-serif, system-ui, sans-serif;
  }
</style>
