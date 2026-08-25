<script lang="ts">
  // Shared account / sync panel — used by Settings → Account and the
  // onboarding "Sync across your devices" step. Handles Google + email/
  // password sign-in and sign-up, the signed-in view (email, editable device
  // name, Sync now + relative last-synced time, sign out), and the
  // not-configured state. All sync state comes from the account store.
  import { onMount, onDestroy } from "svelte";
  import { api } from "./api";
  import { account } from "./account-store.svelte";
  import { settings } from "./settings-store.svelte";
  import { fleet } from "./fleet-store.svelte";
  import { DEVICE_ICONS, deviceGlyph, deviceDisplayName, lastSeenLabel } from "./device-icons";

  // `compact` trims the chrome for the onboarding step (no card border/title).
  let { compact = false } = $props<{ compact?: boolean }>();

  let email = $state("");
  let password = $state("");
  let mode = $state<"signin" | "signup">("signin");
  let busy = $state(false);
  let googleBusy = $state(false);
  let error = $state("");
  let notice = $state("");

  let deviceName = $state("");
  let deviceSaved = $state(false);
  /** Which device's icon picker is open, by id. Only one at a time. */
  let iconPickerFor = $state<string | null>(null);
  /** Inline rename buffer, keyed by device id. */
  let labelDraft = $state<Record<string, string>>({});
  let now = $state(Date.now());

  let configured = $derived(account.status.configured);
  let signedIn = $derived(account.signedIn);
  let restoring = $derived(account.status.restoring && !account.status.signed_in);

  // Tick for the relative "last synced" label — set up synchronously so the
  // async onMount below can stay a plain Promise (its return value isn't a
  // valid Svelte cleanup).
  let tick: ReturnType<typeof setInterval> | undefined;
  onMount(async () => {
    await settings.init();
    await account.init();
    deviceName = settings.s.device_name || "";
    void fleet.subscribe();
    tick = setInterval(() => (now = Date.now()), 30_000);
  });
  onDestroy(() => {
    if (tick) clearInterval(tick);
  });

  async function signInEmail() {
    busy = true;
    error = "";
    notice = "";
    try {
      const s = mode === "signup"
        ? await api.signUpEmail(email.trim(), password)
        : await api.signInEmail(email.trim(), password);
      account.setStatus(s);
      password = "";
    } catch (e) {
      error = `${e}`;
    } finally {
      busy = false;
    }
  }

  async function signInGoogle() {
    googleBusy = true;
    error = "";
    notice = "A browser window opened — finish signing in there.";
    try {
      const s = await api.signInGoogle();
      account.setStatus(s);
      notice = "";
    } catch (e) {
      error = `${e}`;
      notice = "";
    } finally {
      googleBusy = false;
    }
  }

  async function cancelGoogle() {
    try {
      await api.cancelGoogleSignIn();
    } catch {
      /* ignore */
    }
  }

  async function signOut() {
    busy = true;
    try {
      const s = await api.signOut();
      account.setStatus(s);
    } catch (e) {
      error = `${e}`;
    } finally {
      busy = false;
    }
  }

  async function pickIcon(deviceId: string, icon: string | null) {
    iconPickerFor = null;
    try {
      const current = fleet.byId(deviceId);
      await fleet.setMeta(deviceId, icon, current?.label ?? null);
    } catch {
      /* the store restores the previous value and records the error */
    }
  }

  async function saveLabel(deviceId: string) {
    const current = fleet.byId(deviceId);
    const next = (labelDraft[deviceId] ?? "").trim();
    try {
      await fleet.setMeta(deviceId, current?.icon ?? null, next || null);
      delete labelDraft[deviceId];
      labelDraft = { ...labelDraft };
    } catch {
      /* ditto */
    }
  }

  async function saveDeviceName() {
    const name = deviceName.trim();
    if (!name) return;
    await settings.set("device_name", name);
    try {
      await api.setDeviceName(name);
    } catch {
      /* the settings store already persisted it; Rust copy is best-effort */
    }
    deviceSaved = true;
    setTimeout(() => (deviceSaved = false), 1800);
  }

  async function syncNow() {
    try {
      await api.syncNow();
    } catch (e) {
      error = `${e}`;
    }
  }

  function relativeTime(iso: string | null): string {
    if (!iso) return "not yet";
    const t = new Date(iso).getTime();
    if (Number.isNaN(t)) return "not yet";
    const diff = Math.max(0, now - t);
    if (diff < 60_000) return "just now";
    if (diff < 3600_000) return `${Math.floor(diff / 60_000)}m ago`;
    if (diff < 86_400_000) return `${Math.floor(diff / 3600_000)}h ago`;
    return `${Math.floor(diff / 86_400_000)}d ago`;
  }

  let syncLabel = $derived.by(() => {
    switch (account.sync.state) {
      case "syncing":
        return "Syncing…";
      case "error":
        return "Sync paused — will retry";
      case "signed_out":
        return "";
      default:
        return `Last synced ${relativeTime(account.sync.last_synced_at)}`;
    }
  });

  // ── Purge: reset the entire account history on every device ──────────────
  // The one operation that crosses device ownership (it also clears orphaned
  // rows whose originating device is gone). Gated behind a press-and-hold that
  // only ARMS it, then an explicit confirm modal that spells out the stakes.
  // Same HOLD_MS as the History "Clear all" affordance.
  const HOLD_MS = 1500;
  let purgeHoldActive = $state(false);
  let purgeHoldProgress = $state(0); // 0..1
  let purgeConfirmOpen = $state(false);
  let purgeBusy = $state(false);
  let purgeError = $state("");
  let purgeDone = $state("");
  let _purgeStart = 0;
  let _purgeRAF: number | null = null;
  let _purgeReachedFull = false;

  function purgeTick() {
    const t = Math.min(1, (Date.now() - _purgeStart) / HOLD_MS);
    purgeHoldProgress = t;
    if (t >= 1) {
      _purgeRAF = null;
      _purgeReachedFull = true;
      purgeHoldActive = false;
      purgeHoldProgress = 0;
      purgeConfirmOpen = true;
      return;
    }
    _purgeRAF = requestAnimationFrame(purgeTick);
  }
  function startPurgeHold() {
    purgeHoldActive = true;
    purgeError = "";
    purgeDone = "";
    _purgeReachedFull = false;
    _purgeStart = Date.now();
    _purgeRAF = requestAnimationFrame(purgeTick);
  }
  function cancelPurgeHold() {
    if (_purgeRAF !== null) {
      cancelAnimationFrame(_purgeRAF);
      _purgeRAF = null;
    }
    purgeHoldActive = false;
    purgeHoldProgress = 0;
  }

  async function doPurge() {
    purgeBusy = true;
    purgeError = "";
    try {
      await api.purgeAccount();
      purgeConfirmOpen = false;
      purgeDone = "Account history purged on every device.";
    } catch (e) {
      purgeError = `${e}`;
    } finally {
      purgeBusy = false;
    }
  }
</script>

<div class="account" class:compact>
  {#if !configured}
    <div class="not-configured">
      <strong>Sync not configured in this build.</strong>
      <p>Accounts and cross-device sync aren't available in this build. Everything else works exactly as before — your transcripts stay on this device.</p>
    </div>
  {:else if restoring}
    <!-- A stored session exists and its token refresh is in flight. Rendering
         the sign-in form here is what made a restart look like a logout: the
         restore is a network round-trip and the webview always wins the race
         to `auth_status`. -->
    <div class="restoring">
      <span class="spinner" aria-hidden="true"></span>
      <span>Restoring your session…</span>
    </div>
  {:else if signedIn}
    <div class="signed-in">
      <div class="who">
        <div class="avatar" aria-hidden="true">
          {(account.status.email ?? "?").slice(0, 1).toUpperCase()}
        </div>
        <div class="who-text">
          <div class="who-email">{account.status.email}</div>
          <div class="who-sub">{syncLabel}</div>
        </div>
      </div>

      {#if compact}
        <p class="compact-note">You're signed in. Transcripts and API keys can now follow you across desktop, web, and mobile.</p>
      {:else}
      <label class="field">
        <span class="field-label">Device name</span>
        <div class="field-row">
          <input type="text" bind:value={deviceName} placeholder="My Computer" />
          <button class="btn secondary" onclick={saveDeviceName} disabled={!deviceName.trim()}>
            {deviceSaved ? "Saved ✓" : "Save"}
          </button>
        </div>
        <span class="field-hint">Shown on your transcripts so you can tell which device made each one.</span>
      </label>

      <!-- My devices. The registry has existed since v3.0.0 but was never
           surfaced; without it there was no way to see what was signed in,
           and no way to tell two machines apart on a history card. -->
      <div class="devices-block">
        <div class="devices-head">
          <span class="field-label">My devices</span>
          {#if fleet.devices.length > 0}
            <span class="devices-count">{fleet.devices.length}</span>
          {/if}
        </div>
        <span class="field-hint">
          Give each machine an icon and a name you'd actually recognise. The icon shows
          on every transcript from that device.
        </span>

        {#if fleet.devices.length === 0}
          <p class="devices-empty">
            {fleet.loading ? "Looking for your devices…" : "No devices registered yet — they appear after the first sync."}
          </p>
        {:else}
          <ul class="devices-list">
            {#each fleet.devices as dev (dev.id)}
              <li class="device" class:mine={dev.this_device}>
                <button
                  class="device-icon"
                  onclick={() => (iconPickerFor = iconPickerFor === dev.id ? null : dev.id)}
                  aria-haspopup="true"
                  aria-expanded={iconPickerFor === dev.id}
                  title="Choose an icon for this device"
                >{deviceGlyph(dev.icon, dev.platform)}</button>

                <div class="device-main">
                  <div class="device-name">
                    {deviceDisplayName(dev)}
                    {#if dev.this_device}<span class="device-you">this device</span>{/if}
                  </div>
                  <div class="device-sub">
                    {dev.platform ?? "unknown"}
                    {#if lastSeenLabel(dev.last_seen_at)}
                      · last seen {lastSeenLabel(dev.last_seen_at)}
                    {/if}
                    <!-- Only ever said of ANOTHER device. This machine's
                         numbers are read locally and are always available,
                         whether or not it has pushed a rollup yet — saying
                         "not reporting" about the computer you are sitting
                         at reads as a fault when nothing is wrong. -->
                    {#if !dev.stats && !dev.this_device}
                      · not reporting yet
                    {/if}
                  </div>
                </div>

                <div class="device-rename">
                  <input
                    type="text"
                    placeholder={dev.name ?? "Name this device"}
                    value={labelDraft[dev.id] ?? dev.label ?? ""}
                    oninput={(e) => (labelDraft = { ...labelDraft, [dev.id]: e.currentTarget.value })}
                    onkeydown={(e) => { if (e.key === "Enter") saveLabel(dev.id); }}
                  />
                  {#if labelDraft[dev.id] !== undefined && labelDraft[dev.id] !== (dev.label ?? "")}
                    <button class="btn secondary tiny" onclick={() => saveLabel(dev.id)}>Save</button>
                  {/if}
                </div>

                {#if iconPickerFor === dev.id}
                  <div class="icon-picker" role="group" aria-label="Device icons">
                    {#each DEVICE_ICONS as ico (ico.id)}
                      <button
                        class="icon-opt"
                        class:chosen={dev.icon === ico.id}
                        title={ico.label}
                        aria-label={ico.label}
                        onclick={() => pickIcon(dev.id, ico.id)}
                      >{ico.glyph}</button>
                    {/each}
                    <button class="icon-opt clear" title="No icon" onclick={() => pickIcon(dev.id, null)}>✕</button>
                  </div>
                {/if}
              </li>
            {/each}
          </ul>
        {/if}
      </div>

      <div class="actions-row">
        <button class="btn secondary" onclick={syncNow} disabled={account.sync.state === "syncing"}>
          {account.sync.state === "syncing" ? "Syncing…" : "Sync now"}
        </button>
        <button class="btn ghost" onclick={signOut} disabled={busy}>Sign out</button>
      </div>
      {#if error}<p class="err">{error}</p>{/if}

      <!-- Purge — the account-wide reset, kept at the very bottom of Account.
           This is the only delete that crosses device ownership, so it's the
           most dangerous control in the app: press-and-hold to arm, then an
           explicit confirm. -->
      <div class="danger-zone">
        <div class="dz-head">Reset account history</div>
        <p class="dz-body">
          Deletes every transcript on your account across <strong>all</strong> devices —
          including devices you no longer have — and clears any leftover entries
          whose device is gone. Your API keys and sign-in stay. This can't be undone.
        </p>
        <button
          class="hold-purge"
          class:armed={purgeHoldActive}
          style="--hold:{purgeHoldProgress}"
          onmousedown={startPurgeHold}
          onmouseup={cancelPurgeHold}
          onmouseleave={cancelPurgeHold}
          title="Press and hold to reset your entire account history"
        >
          <span class="hold-fill"></span>
          <span class="hold-text">
            {#if purgeHoldActive}Keep holding…{:else}Purge account history{/if}
          </span>
        </button>
        {#if purgeDone}<p class="dz-done">{purgeDone}</p>{/if}
        {#if purgeError && !purgeConfirmOpen}<p class="err">{purgeError}</p>{/if}
      </div>
      {/if}
    </div>
  {:else}
    <div class="signed-out">
      <p class="explainer">Sync your transcripts across desktop, web and mobile.</p>

      <button class="btn google" onclick={signInGoogle} disabled={googleBusy}>
        <svg viewBox="0 0 18 18" width="16" height="16" aria-hidden="true">
          <path fill="#4285F4" d="M17.6 9.2c0-.6-.05-1.2-.16-1.7H9v3.4h4.8a4.1 4.1 0 0 1-1.8 2.7v2.2h2.9c1.7-1.6 2.7-3.9 2.7-6.6z"/>
          <path fill="#34A853" d="M9 18c2.4 0 4.5-.8 6-2.2l-2.9-2.2c-.8.5-1.8.9-3.1.9-2.4 0-4.4-1.6-5.1-3.8H.9v2.3A9 9 0 0 0 9 18z"/>
          <path fill="#FBBC05" d="M3.9 10.7a5.4 5.4 0 0 1 0-3.4V5H.9a9 9 0 0 0 0 8l3-2.3z"/>
          <path fill="#EA4335" d="M9 3.6c1.3 0 2.5.45 3.4 1.3l2.6-2.6A9 9 0 0 0 .9 5l3 2.3C4.6 5.2 6.6 3.6 9 3.6z"/>
        </svg>
        {googleBusy ? "Waiting for the browser…" : "Continue with Google"}
      </button>
      {#if googleBusy}
        <button class="link-btn" onclick={cancelGoogle}>Cancel</button>
      {/if}

      <div class="divider"><span>or</span></div>

      <div class="email-form">
        <input type="email" placeholder="you@example.com" bind:value={email} autocomplete="username" />
        <input
          type="password"
          placeholder="Password"
          bind:value={password}
          autocomplete={mode === "signup" ? "new-password" : "current-password"}
          onkeydown={(e) => { if (e.key === "Enter") signInEmail(); }}
        />
        <button class="btn primary" onclick={signInEmail} disabled={busy || !email.trim() || !password}>
          {busy ? "Please wait…" : mode === "signup" ? "Create account" : "Sign in"}
        </button>
      </div>

      <button
        class="link-btn"
        onclick={() => { mode = mode === "signup" ? "signin" : "signup"; error = ""; }}
      >
        {mode === "signup" ? "Already have an account? Sign in" : "Create an account"}
      </button>

      {#if notice}<p class="notice">{notice}</p>{/if}
      {#if error}<p class="err">{error}</p>{/if}
    </div>
  {/if}
</div>

<!-- Purge confirm — the second gate after the press-and-hold. Reachable only
     while signed in. Spells out the irreversible, cross-device consequence in
     plain words before the final button. -->
{#if purgeConfirmOpen}
  <div
    class="overlay"
    role="button"
    tabindex="-1"
    onclick={() => (purgeConfirmOpen = false)}
    onkeydown={(e) => { if (e.key === "Escape") purgeConfirmOpen = false; }}
  >
    <div class="confirm-card" role="dialog" aria-modal="true" aria-label="Reset account history" onclick={(e) => e.stopPropagation()} onkeydown={() => {}} tabindex="-1">
      <h3>Reset your entire account history?</h3>
      <p>
        This permanently deletes every transcript on your account, on
        <strong>every device</strong> — including devices you no longer have —
        and clears any leftover entries whose device is gone.
      </p>
      <p>Your API keys and sign-in are kept. <strong>This cannot be undone.</strong></p>
      {#if purgeError}<p class="err">{purgeError}</p>{/if}
      <div class="confirm-actions">
        <button class="btn ghost" onclick={() => (purgeConfirmOpen = false)} disabled={purgeBusy}>Cancel</button>
        <button class="btn danger" onclick={doPurge} disabled={purgeBusy}>
          {purgeBusy ? "Purging…" : "Purge everything"}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .account {
    max-width: 460px;
  }
  .account.compact {
    width: min(100%, 440px);
    padding: 18px;
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-lg);
    background: var(--bg-card);
    box-shadow: var(--shadow-sm);
  }
  .compact-note {
    margin: 12px 0 0;
    color: var(--text-secondary);
    font-size: 12px;
    line-height: 1.55;
  }
  .not-configured {
    background: var(--bg-subtle);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 16px 18px;
    color: var(--text-secondary);
    font-size: 13px;
  }
  .not-configured strong {
    color: var(--text-primary);
    display: block;
    margin-bottom: 6px;
  }
  .not-configured p {
    margin: 0;
    line-height: 1.5;
  }

  .restoring {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 18px 2px;
    color: var(--text-secondary);
    font-size: 13px;
  }
  .restoring .spinner {
    width: 14px;
    height: 14px;
    border: 2px solid var(--border);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: account-spin 0.7s linear infinite;
  }
  @keyframes account-spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .restoring .spinner {
      animation: none;
    }
  }

  .explainer {
    font-size: 13px;
    color: var(--text-secondary);
    margin: 0 0 14px;
  }

  .btn {
    border-radius: 8px;
    padding: 9px 16px;
    font-size: 13px;
    font-weight: 600;
    font-family: inherit;
    cursor: pointer;
    border: 1px solid transparent;
  }
  .btn:disabled {
    opacity: 0.55;
    cursor: default;
  }
  .btn.primary {
    background: var(--accent);
    color: #fff;
  }
  .btn.secondary {
    background: var(--bg-card);
    border-color: var(--border);
    color: var(--text-primary);
  }
  .btn.secondary:hover {
    background: var(--bg-subtle);
  }
  .btn.ghost {
    background: transparent;
    border-color: var(--border);
    color: var(--text-secondary);
  }
  .btn.google {
    width: 100%;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 9px;
    background: var(--bg-card);
    border-color: var(--border);
    color: var(--text-primary);
  }
  .btn.google:hover {
    background: var(--bg-subtle);
  }

  .divider {
    display: flex;
    align-items: center;
    text-align: center;
    color: var(--text-secondary);
    font-size: 11px;
    margin: 14px 0;
  }
  .divider::before,
  .divider::after {
    content: "";
    flex: 1;
    height: 1px;
    background: var(--border);
  }
  .divider span {
    padding: 0 10px;
  }

  .email-form {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .email-form input,
  .field-row input {
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 9px 11px;
    font-size: 13px;
    background: var(--bg-card);
    color: var(--text-primary);
    outline: none;
    font-family: inherit;
  }
  .email-form input:focus,
  .field-row input:focus {
    border-color: var(--accent);
  }

  .link-btn {
    background: none;
    border: none;
    color: var(--accent);
    font-size: 12px;
    cursor: pointer;
    padding: 8px 0 0;
    font-family: inherit;
  }

  .notice {
    font-size: 12px;
    color: var(--text-secondary);
    margin: 10px 0 0;
  }
  .err {
    font-size: 12px;
    color: var(--danger);
    margin: 10px 0 0;
  }

  /* Signed-in view */
  .who {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 18px;
  }
  .avatar {
    width: 40px;
    height: 40px;
    border-radius: 50%;
    background: var(--accent);
    color: #fff;
    display: flex;
    align-items: center;
    justify-content: center;
    font-weight: 700;
    font-size: 17px;
    flex-shrink: 0;
  }
  .who-email {
    font-size: 14px;
    font-weight: 600;
    color: var(--text-primary);
  }
  .who-sub {
    font-size: 12px;
    color: var(--text-secondary);
  }

  .field {
    display: block;
    margin-bottom: 18px;
  }
  .field-label {
    display: block;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-primary);
    margin-bottom: 6px;
  }
  .field-row {
    display: flex;
    gap: 8px;
  }
  .field-row input {
    flex: 1;
  }
  .field-hint {
    display: block;
    font-size: 11.5px;
    color: var(--text-secondary);
    margin-top: 6px;
    line-height: 1.4;
  }

  /* ── My devices ─────────────────────────────────────────────────────── */
  .devices-block {
    margin-top: 22px;
  }
  .devices-head {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .devices-count {
    font-size: 10px;
    color: var(--text-secondary);
    background: var(--bg-subtle);
    padding: 1px 7px;
    border-radius: 9999px;
  }
  .devices-empty {
    margin: 12px 0 0;
    font-size: 12px;
    color: var(--text-secondary);
  }
  .devices-list {
    list-style: none;
    margin: 12px 0 0;
    padding: 0;
    display: grid;
    gap: 8px;
  }
  .device {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    border: 1px solid var(--border-subtle);
    border-radius: 10px;
    background: var(--bg-card);
  }
  .device.mine {
    border-color: color-mix(in srgb, var(--accent) 32%, var(--border-subtle));
    background: color-mix(in srgb, var(--accent-fade) 26%, var(--bg-card));
  }
  .device-icon {
    width: 34px;
    height: 34px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-size: 16px;
    line-height: 1;
    border: 1px solid var(--border);
    border-radius: 9px;
    background: var(--bg-subtle);
    cursor: pointer;
    transition: border-color 120ms ease, background 120ms ease;
  }
  .device-icon:hover {
    border-color: var(--accent);
    background: var(--accent-fade);
  }
  .device-main {
    min-width: 0;
  }
  .device-name {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .device-you {
    margin-left: 7px;
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--accent);
  }
  .device-sub {
    font-size: 11px;
    color: var(--text-secondary);
    margin-top: 2px;
  }
  .device-rename {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .device-rename input {
    width: 150px;
    font-size: 12px;
    padding: 5px 8px;
    border: 1px solid var(--border);
    border-radius: 7px;
    background: var(--bg-surface);
    color: var(--text-primary);
    font-family: inherit;
  }
  .btn.tiny {
    padding: 5px 9px;
    font-size: 11px;
  }
  /* The picker spans the full row underneath, so a 10-icon grid never
     squeezes the name column. */
  .icon-picker {
    grid-column: 1 / -1;
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding-top: 10px;
    margin-top: 2px;
    border-top: 1px dashed var(--border-subtle);
  }
  .icon-opt {
    width: 30px;
    height: 30px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-size: 14px;
    line-height: 1;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--bg-surface);
    cursor: pointer;
    transition: border-color 120ms ease, background 120ms ease;
  }
  .icon-opt:hover {
    border-color: var(--accent);
    background: var(--accent-fade);
  }
  .icon-opt.chosen {
    border-color: var(--accent);
    background: var(--accent-fade);
    box-shadow: 0 0 0 2px var(--accent-fade);
  }
  .icon-opt.clear {
    color: var(--text-secondary);
    font-size: 11px;
  }

  @media (max-width: 620px) {
    .device {
      grid-template-columns: auto minmax(0, 1fr);
    }
    .device-rename {
      grid-column: 1 / -1;
    }
    .device-rename input {
      width: 100%;
    }
  }

  .actions-row {
    display: flex;
    gap: 8px;
  }

  /* Danger zone — the account-wide purge. Set apart with a red-tinted border
     and kept at the bottom so it's never a stray click near the everyday
     controls. */
  .danger-zone {
    margin-top: 24px;
    padding: 14px 16px;
    border: 1px solid var(--danger);
    border-radius: 12px;
    background: var(--danger-fade);
  }
  .dz-head {
    font-size: 12px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--danger);
    margin-bottom: 6px;
  }
  .dz-body {
    font-size: 12px;
    line-height: 1.5;
    color: var(--text-secondary);
    margin: 0 0 12px;
  }
  .dz-done {
    font-size: 12px;
    color: var(--text-secondary);
    margin: 10px 0 0;
  }

  /* Press-and-hold purge button — a danger fill sweeps left→right while held;
     releasing before it completes cancels. Mirrors the History "Clear all"
     hold affordance. */
  .hold-purge {
    position: relative;
    overflow: hidden;
    isolation: isolate;
    background: var(--bg-card);
    border: 1px solid var(--danger);
    border-radius: 8px;
    padding: 9px 16px;
    cursor: pointer;
    font-size: 13px;
    font-weight: 600;
    color: var(--danger);
    user-select: none;
    font-family: inherit;
    transition: color 120ms ease;
  }
  .hold-purge.armed {
    color: #fff;
  }
  .hold-fill {
    position: absolute;
    inset: 0;
    z-index: -1;
    transform-origin: left center;
    transform: scaleX(var(--hold, 0));
    background: var(--danger);
    transition: transform 60ms linear;
  }
  .hold-text {
    position: relative;
    white-space: nowrap;
  }

  /* Purge confirm modal. */
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 400;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(30, 24, 16, 0.42);
    backdrop-filter: blur(2px);
    padding: 24px;
  }
  .confirm-card {
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 16px;
    padding: 22px 24px;
    width: 100%;
    max-width: 440px;
    box-shadow: 0 18px 60px rgba(40, 26, 10, 0.35);
    color: var(--text-primary);
    text-align: left;
  }
  .confirm-card h3 {
    margin: 0 0 12px;
    font-size: 17px;
    font-weight: 600;
  }
  .confirm-card p {
    font-size: 13px;
    line-height: 1.55;
    color: var(--text-secondary);
    margin: 0 0 10px;
  }
  .confirm-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
  }
  .btn.danger {
    background: var(--danger);
    color: #fff;
  }
  .btn.danger:hover {
    filter: brightness(1.05);
  }
</style>
