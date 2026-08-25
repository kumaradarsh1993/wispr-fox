//! Accounts + cross-device sync (v3.0.0). Optional: sign in (Google or
//! email/password) to sync transcripts + a few API keys across desktop,
//! web, and mobile. Signed-out behaviour is unchanged from every prior
//! release — every code path in this module (and everything that calls
//! into it) is a no-op unless the user has signed in AND this build has a
//! real Supabase project baked into `config.rs`.
//!
//! Canonical spec: `wispr-fox-web/docs/SYNC_DESIGN.md`. Read that before
//! touching any of these three files — schema, protocol, delete semantics,
//! and the hard rules all live there.

pub mod auth;
pub mod config;
pub mod engine;
pub mod fleet;
