//! Supabase project constants for this client. The anon key is publishable
//! (safe to ship in the binary) — RLS on the backend is what actually
//! protects user data, not secrecy of this key.
//!
//! Both constants are placeholders until the orchestrator bakes in the
//! user's real Supabase project values before tagging a release. While
//! they're placeholders, [`is_configured`] returns `false` and every piece
//! of account/sync UI must show "Sync not configured in this build" and
//! behave exactly as if the user were signed out — never crash, never
//! block dictation.

pub const SUPABASE_URL: &str = "https://hvaljemiwuhnohrndyyh.supabase.co";
pub const SUPABASE_ANON_KEY: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6Imh2YWxqZW1pd3Vobm9ocm5keXloIiwicm9sZSI6ImFub24iLCJpYXQiOjE3ODQxMjI1MDAsImV4cCI6MjA5OTY5ODUwMH0.kOu8wVU1XqGOooAShVCQtkr6IIxcdeBujuyUiajMOBc";

/// `false` while the constants above are still the baked-in placeholders.
pub fn is_configured() -> bool {
    !SUPABASE_URL.is_empty()
        && !SUPABASE_ANON_KEY.is_empty()
        && !SUPABASE_URL.contains("PLACEHOLDER")
        && !SUPABASE_ANON_KEY.contains("PLACEHOLDER")
}
