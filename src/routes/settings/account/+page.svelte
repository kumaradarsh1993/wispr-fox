<script lang="ts">
  // Settings → Account. Sign in (Google or email/password) to sync
  // transcripts + synced API keys across desktop, web and mobile. All of the
  // real UI lives in the shared AccountPanel so the onboarding step and this
  // page stay in lockstep.
  import AccountPanel from "$lib/AccountPanel.svelte";
</script>

<section>
  <h2>Account</h2>
  <p class="lede">
    Sign in to sync your transcripts and API keys across desktop, web and
    mobile. Optional — signed out, wispr-fox works exactly as before and stays
    fully local. Your voice recordings never leave this device either way.
  </p>

  <AccountPanel />
</section>

<style>
  section {
    max-width: 640px;
  }
  h2 {
    font-size: 18px;
    font-weight: 600;
    margin: 0 0 6px;
    color: var(--text-primary);
  }
  .lede {
    font-size: 13px;
    color: var(--text-secondary);
    line-height: 1.55;
    margin: 0 0 20px;
    max-width: 520px;
  }
</style>
