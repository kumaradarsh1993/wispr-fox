//! Named-secret access to the OS credential store, shared by `secrets.rs`
//! (API keys) and `sync/auth.rs` (the sync refresh token).
//!
//! Windows and Linux: one keyring entry per name, exactly as before.
//!
//! macOS: one Keychain item per APP BUILD, holding every secret as JSON.
//!
//! **Why macOS is different.** wispr-fox is signed with a self-signed
//! certificate, so it has no Apple Team ID. For such apps the Keychain's
//! partition list names the app by its cdhash, the hash of that exact binary,
//! and every release has a new one. An item created by build N therefore
//! raises a password prompt when build N+1 reads it, once per item. With one
//! item per secret, that was 4 prompts (3 API keys + the sync token), and on
//! every launch for anyone who pressed Return after typing the password, which
//! picks "Allow" rather than "Always Allow".
//!
//! An item this build created itself carries its own cdhash, so reading it
//! never prompts. So on the first launch of a new build we read the previous
//! build's single vault item (at most ONE prompt), write a fresh vault item
//! that this build owns, and delete the old one. Every later launch of the
//! same build is prompt-free.
//!
//! Safety rule: an old item is deleted only after its contents were read AND
//! the new vault was written and read back. A read the user denied is kept in
//! `pending` and retried on a later launch, so a denied prompt never loses a key.

use anyhow::Result;

#[cfg(not(target_os = "macos"))]
mod imp {
    use anyhow::{anyhow, Context, Result};

    const SERVICE: &str = "wispr-fox";

    fn entry(name: &str) -> Result<keyring::Entry> {
        keyring::Entry::new(SERVICE, name)
            .with_context(|| format!("opening keyring entry {name}"))
    }

    pub fn get(name: &str) -> Option<String> {
        entry(name).ok().and_then(|e| e.get_password().ok())
    }

    pub fn set(name: &str, value: &str) -> Result<()> {
        entry(name)?
            .set_password(value)
            .with_context(|| format!("storing {name} in keyring"))
    }

    pub fn delete(name: &str) -> Result<()> {
        match entry(name)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(anyhow!("keyring delete failed for {name}: {e:#}")),
        }
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use super::vault::{self, VaultData};
    use anyhow::{anyhow, Context, Result};
    use parking_lot::Mutex;
    use std::sync::OnceLock;

    const SERVICE: &str = "wispr-fox";

    struct State {
        account: String,
        data: VaultData,
    }

    static STATE: OnceLock<Mutex<State>> = OnceLock::new();

    fn state() -> &'static Mutex<State> {
        STATE.get_or_init(|| Mutex::new(load()))
    }

    enum Read {
        Found(String),
        Missing,
        Failed(String),
    }

    fn read(account: &str) -> Read {
        let entry = match keyring::Entry::new(SERVICE, account) {
            Ok(e) => e,
            Err(e) => return Read::Failed(e.to_string()),
        };
        match entry.get_password() {
            Ok(v) => Read::Found(v),
            Err(keyring::Error::NoEntry) => Read::Missing,
            Err(e) => Read::Failed(e.to_string()),
        }
    }

    fn write(account: &str, value: &str) -> Result<()> {
        let entry = keyring::Entry::new(SERVICE, account)
            .with_context(|| format!("opening keychain item {account}"))?;
        entry
            .set_password(value)
            .with_context(|| format!("writing keychain item {account}"))?;
        match entry.get_password() {
            Ok(back) if back == value => Ok(()),
            _ => Err(anyhow!("keychain item {account} did not read back")),
        }
    }

    fn remove(account: &str) {
        if let Ok(entry) = keyring::Entry::new(SERVICE, account) {
            match entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => {}
                Err(e) => tracing::warn!("keychain: could not delete old item {account}: {e}"),
            }
        }
    }

    /// Account names under our service. Reads attributes only, never secret
    /// data, so it never prompts. `None` if the search itself failed.
    fn list_accounts() -> Option<Vec<String>> {
        use security_framework::item::{ItemClass, ItemSearchOptions, Limit};
        let results = match ItemSearchOptions::new()
            .class(ItemClass::generic_password())
            .service(SERVICE)
            .load_attributes(true)
            .limit(Limit::All)
            .search()
        {
            Ok(r) => r,
            // errSecItemNotFound: nothing stored yet.
            Err(e) if e.code() == -25300 => return Some(Vec::new()),
            Err(e) => {
                tracing::warn!("keychain: listing items failed: {e}");
                return None;
            }
        };
        Some(
            results
                .iter()
                .filter_map(|r| r.simplify_dict())
                .filter_map(|d| d.get("acct").cloned())
                .collect(),
        )
    }

    fn load() -> State {
        let version = env!("CARGO_PKG_VERSION");
        let accounts = list_accounts().unwrap_or_else(|| {
            // Search failed: fall back to the names we know.
            vault::LEGACY_ACCOUNTS.iter().map(|s| s.to_string()).collect()
        });
        let plan = vault::plan(&accounts, version);

        let mut data = VaultData::default();
        let mut account = plan.own.clone();
        let mut dirty = false;

        if let Some(own) = &plan.own {
            match read(own) {
                Read::Found(json) => {
                    data = serde_json::from_str(&json).unwrap_or_default();
                }
                Read::Missing => account = None,
                Read::Failed(e) => {
                    // Our own item should never prompt. If it can't be read,
                    // leave it alone and start a fresh one beside it.
                    tracing::warn!("keychain: own vault {own} unreadable: {e}");
                    account = None;
                    data.pending.push(own.clone());
                }
            }
        }
        if account.is_none() {
            data.pending.extend(plan.sources.iter().cloned());
            dirty = true;
        }

        // Read every pending source (the old vault on an update, or the
        // per-key items the first time). Each may raise one prompt.
        let mut absorbed: Vec<String> = Vec::new();
        let mut queue: std::collections::VecDeque<String> =
            std::mem::take(&mut data.pending).into();
        let mut seen = std::collections::HashSet::new();
        while let Some(name) = queue.pop_front() {
            if !seen.insert(name.clone()) {
                continue;
            }
            match read(&name) {
                Read::Found(content) => match vault::absorb(&mut data, &name, &content) {
                    Some(more) => {
                        queue.extend(more);
                        absorbed.push(name);
                        dirty = true;
                    }
                    None => {
                        tracing::warn!("keychain: {name} is not a readable vault, kept as is");
                    }
                },
                Read::Missing => dirty = true,
                Read::Failed(e) => {
                    tracing::warn!("keychain: could not read {name} ({e}); will retry next launch");
                    data.pending.push(name);
                }
            }
        }

        let account = account.unwrap_or_else(|| vault::new_account(version));
        if dirty {
            match serde_json::to_string(&data)
                .map_err(anyhow::Error::from)
                .and_then(|json| write(&account, &json))
            {
                Ok(()) => {
                    // The new vault is verified, so the absorbed copies and
                    // stale vaults of other builds can go.
                    for name in absorbed.iter().chain(plan.stale.iter()) {
                        if name != &account && !data.pending.contains(name) {
                            remove(name);
                        }
                    }
                }
                Err(e) => tracing::warn!("keychain: could not save vault {account}: {e:#}"),
            }
        }

        State { account, data }
    }

    fn persist(st: &State) -> Result<()> {
        let json = serde_json::to_string(&st.data)?;
        write(&st.account, &json)
    }

    pub fn get(name: &str) -> Option<String> {
        state().lock().data.values.get(name).cloned()
    }

    pub fn set(name: &str, value: &str) -> Result<()> {
        let mut st = state().lock();
        st.data.values.insert(name.to_string(), value.to_string());
        // A user-written value is authoritative: stop retrying an old copy.
        st.data.pending.retain(|p| p != name);
        persist(&st)
    }

    pub fn delete(name: &str) -> Result<()> {
        let mut st = state().lock();
        let had = st.data.values.remove(name).is_some();
        let was_pending = st.data.pending.contains(&name.to_string());
        st.data.pending.retain(|p| p != name);
        if was_pending {
            remove(name);
        }
        if had || was_pending {
            persist(&st)?;
        }
        Ok(())
    }
}

/// Pure planning and merge logic for the macOS vault, kept platform-neutral
/// so it is unit-tested on every OS.
#[allow(dead_code)]
mod vault {
    use serde::{Deserialize, Serialize};
    use std::collections::BTreeMap;

    pub const PREFIX: &str = "vault:";

    /// Per-secret item names used before the vault existed.
    pub const LEGACY_ACCOUNTS: [&str; 8] = [
        "groq_stt_key",
        "groq_llm_key",
        "gemini_llm_key",
        "openai_stt_key",
        "openai_llm_key",
        "deepgram_stt_key",
        "elevenlabs_stt_key",
        "sync_refresh_token",
    ];

    #[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
    pub struct VaultData {
        #[serde(default)]
        pub values: BTreeMap<String, String>,
        /// Item names whose read failed (e.g. the user denied the prompt).
        /// Retried on a later launch; never deleted until read.
        #[serde(default)]
        pub pending: Vec<String>,
    }

    /// `vault:<created-ms>:<version>`
    pub fn new_account(version: &str) -> String {
        format!("{PREFIX}{}:{version}", chrono::Utc::now().timestamp_millis())
    }

    fn parse(account: &str) -> Option<(i64, &str)> {
        let rest = account.strip_prefix(PREFIX)?;
        let (ms, version) = rest.split_once(':')?;
        Some((ms.parse().ok()?, version))
    }

    #[derive(Debug, PartialEq)]
    pub struct Plan {
        /// This build's own vault, if one exists.
        pub own: Option<String>,
        /// What to read if there is no own vault: the newest older vault, or
        /// the legacy per-key items when no vault exists at all.
        pub sources: Vec<String>,
        /// Older vaults to delete without reading (only the newest is read,
        /// so leftovers never multiply the prompts).
        pub stale: Vec<String>,
    }

    pub fn plan(accounts: &[String], version: &str) -> Plan {
        let mut vaults: Vec<(i64, &str, &String)> = accounts
            .iter()
            .filter_map(|a| parse(a).map(|(ms, v)| (ms, v, a)))
            .collect();
        vaults.sort_by(|a, b| b.0.cmp(&a.0));

        let own_idx = vaults.iter().position(|(_, v, _)| *v == version);
        let own = own_idx.map(|i| vaults[i].2.clone());
        let others: Vec<String> = vaults
            .iter()
            .enumerate()
            .filter(|(i, _)| Some(*i) != own_idx)
            .map(|(_, (_, _, a))| (*a).clone())
            .collect();

        let legacy: Vec<String> = LEGACY_ACCOUNTS
            .iter()
            .filter(|n| accounts.iter().any(|a| a == *n))
            .map(|s| s.to_string())
            .collect();

        let (sources, mut stale) = if own.is_some() {
            (Vec::new(), others.clone())
        } else if let Some((newest, rest)) = others.split_first() {
            (vec![newest.clone()], rest.to_vec())
        } else {
            (legacy.clone(), Vec::new())
        };
        // Once any vault exists, legacy items are leftovers whose values the
        // vault already holds (or lists as pending).
        if !others.is_empty() || own.is_some() {
            stale.extend(legacy);
        }
        Plan { own, sources, stale }
    }

    /// Fold one item's content into `data`, without overwriting values that
    /// are already there. Returns the item names the content still listed as
    /// pending (to read next), or `None` if a vault item did not parse.
    pub fn absorb(data: &mut VaultData, account: &str, content: &str) -> Option<Vec<String>> {
        if account.starts_with(PREFIX) {
            let old: VaultData = serde_json::from_str(content).ok()?;
            for (k, v) in old.values {
                data.values.entry(k).or_insert(v);
            }
            Some(old.pending)
        } else {
            if !content.trim().is_empty() {
                data.values
                    .entry(account.to_string())
                    .or_insert_with(|| content.to_string());
            }
            Some(Vec::new())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn s(v: &[&str]) -> Vec<String> {
            v.iter().map(|x| x.to_string()).collect()
        }

        #[test]
        fn first_launch_reads_the_legacy_items_that_exist() {
            let p = plan(&s(&["groq_stt_key", "sync_refresh_token", "__probe__"]), "3.6.0");
            assert_eq!(p.own, None);
            assert_eq!(p.sources, s(&["groq_stt_key", "sync_refresh_token"]));
            assert!(p.stale.is_empty());
        }

        #[test]
        fn same_build_uses_its_own_vault_and_reads_nothing_else() {
            let p = plan(&s(&["vault:200:3.6.0", "vault:100:3.5.0"]), "3.6.0");
            assert_eq!(p.own.as_deref(), Some("vault:200:3.6.0"));
            assert_eq!(p.stale, s(&["vault:100:3.5.0"]));
        }

        #[test]
        fn an_update_reads_only_the_newest_old_vault() {
            let p = plan(
                &s(&["vault:100:3.4.0", "vault:300:3.5.0", "groq_stt_key"]),
                "3.6.0",
            );
            assert_eq!(p.own, None);
            assert_eq!(p.sources, s(&["vault:300:3.5.0"]));
            assert_eq!(p.stale, s(&["vault:100:3.4.0", "groq_stt_key"]));
        }

        #[test]
        fn absorb_keeps_existing_values_and_returns_old_pending() {
            let mut d = VaultData::default();
            d.values.insert("groq_stt_key".into(), "new".into());
            let old = r#"{"values":{"groq_stt_key":"old","gemini_llm_key":"g"},"pending":["deepgram_stt_key"]}"#;
            let more = absorb(&mut d, "vault:1:3.5.0", old).unwrap();
            assert_eq!(d.values["groq_stt_key"], "new");
            assert_eq!(d.values["gemini_llm_key"], "g");
            assert_eq!(more, s(&["deepgram_stt_key"]));
        }

        #[test]
        fn absorb_takes_a_legacy_item_as_one_value() {
            let mut d = VaultData::default();
            assert_eq!(absorb(&mut d, "deepgram_stt_key", "dg"), Some(vec![]));
            assert_eq!(d.values["deepgram_stt_key"], "dg");
        }

        #[test]
        fn a_corrupt_vault_is_not_absorbed() {
            let mut d = VaultData::default();
            assert_eq!(absorb(&mut d, "vault:1:3.5.0", "not json"), None);
        }
    }
}

pub fn get(name: &str) -> Option<String> {
    imp::get(name)
}

pub fn set(name: &str, value: &str) -> Result<()> {
    imp::set(name, value)
}

pub fn delete(name: &str) -> Result<()> {
    imp::delete(name)
}
