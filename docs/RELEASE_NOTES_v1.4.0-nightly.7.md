# wispr-fox v1.4.0-nightly.7 - Codex key-storage hardening

This is a Codex-authored nightly focused on Windows API-key reliability and
secret-storage visibility.

## What changed

- Replaced the Windows plaintext fallback with a DPAPI-encrypted local fallback
  that only the same Windows user profile can decrypt.
- Kept OS keyring storage as the preferred path and continued verifying writes
  by reading the key back immediately after save.
- Added safe migration for older `.keys.json` fallback files. Legacy plaintext
  entries are moved to the OS keyring when possible, or to the encrypted local
  fallback when the keyring cannot be verified.
- Added Settings -> Security with storage status, fallback paths, keyring
  health, and a recent no-secret key event log.
- Added Git ignore guardrails for local key fallback and secret audit files.
- Documented the Codex handoff checkpoint for future Claude Code sessions.

## Notes

The key event log never shows API key values. It records the key label, action,
storage location, result, and a short diagnostic detail so key-save failures are
visible without leaking secrets.
