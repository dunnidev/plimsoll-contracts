//! Coverage ledger: records circulating-supply snapshots and reserve reports
//! for Stellar-issued assets, and answers "is this asset covered?" for any
//! contract that asks.
#![no_std]

use plimsoll_types::{
    coverage_bps, AssetRecord, Coverage, RegistryClient, ReserveReport, Role, SupplySnapshot, Tier,
    INSTANCE_EXTEND_TO, INSTANCE_THRESHOLD, PERSISTENT_EXTEND_TO, PERSISTENT_THRESHOLD,
};
use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, panic_with_error, token,
    Address, BytesN, ContractExecutable, Env, Executable, String,
};

const MAX_URI_LEN: u32 = 256;
/// Longest SAC name: 12-char code + ':' + 56-char G-address.
const MAX_SAC_NAME_LEN: usize = 69;
const STRKEY_LEN: usize = 56;
const MAX_CODE_LEN: usize = 12;

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Admin,
    Registry,
    Asset(Address),
    Supply(Address),
    Report(Address, Tier),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum LedgerError {
    NotAStellarAsset = 1,
    AlreadyListed = 2,
    AssetNotListed = 3,
    NotSupplyPoster = 4,
    NotReporter = 5,
    InvalidAmount = 6,
    FutureLedger = 7,
    StaleLedger = 8,
    FutureTimestamp = 9,
    StaleReport = 10,
    InvalidUri = 11,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetListed {
    #[topic]
    pub sac: Address,
    pub code: String,
    pub issuer: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SupplyPosted {
    #[topic]
    pub sac: Address,
    pub amount: i128,
    pub ledger: u32,
    pub breakdown_hash: BytesN<32>,
    pub poster: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReservePosted {
    #[topic]
    pub sac: Address,
    #[topic]
    pub tier: Tier,
    pub amount: i128,
    pub as_of: u64,
    pub reporter: Address,
    pub doc_hash: BytesN<32>,
    pub doc_uri: String,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminTransferred {
    #[topic]
    pub new_admin: Address,
}

#[contract]
pub struct CoverageLedger;

fn admin(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .expect("admin is set in the constructor")
}

fn registry(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&DataKey::Registry)
        .expect("registry is set in the constructor")
}

fn bump_instance(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(INSTANCE_THRESHOLD, INSTANCE_EXTEND_TO);
}

fn read_persistent<V: soroban_sdk::TryFromVal<Env, soroban_sdk::Val>>(
    env: &Env,
    key: &DataKey,
) -> Option<V> {
    let value = env.storage().persistent().get::<_, V>(key);
    if value.is_some() {
        env.storage()
            .persistent()
            .extend_ttl(key, PERSISTENT_THRESHOLD, PERSISTENT_EXTEND_TO);
    }
    value
}

fn write_persistent<V: soroban_sdk::IntoVal<Env, soroban_sdk::Val>>(
    env: &Env,
    key: &DataKey,
    value: &V,
) {
    env.storage().persistent().set(key, value);
    env.storage()
        .persistent()
        .extend_ttl(key, PERSISTENT_THRESHOLD, PERSISTENT_EXTEND_TO);
}

fn require_listed(env: &Env, sac: &Address) -> AssetRecord {
    match read_persistent::<AssetRecord>(env, &DataKey::Asset(sac.clone())) {
        Some(asset) => asset,
        None => panic_with_error!(env, LedgerError::AssetNotListed),
    }
}

/// Split a Stellar Asset Contract name (`CODE:GISSUER...`) into code and
/// issuer. Returns `None` for `native` or anything malformed.
fn parse_sac_name(env: &Env, name: &String) -> Option<(String, Address)> {
    let len = name.len() as usize;
    if !(STRKEY_LEN + 2..=MAX_SAC_NAME_LEN).contains(&len) {
        return None;
    }
    let mut buf = [0u8; MAX_SAC_NAME_LEN];
    name.copy_into_slice(&mut buf[..len]);
    let colon = buf[..len].iter().position(|b| *b == b':')?;
    if colon == 0 || colon > MAX_CODE_LEN || len - colon - 1 != STRKEY_LEN {
        return None;
    }
    let code = String::from_bytes(env, &buf[..colon]);
    let issuer = String::from_bytes(env, &buf[colon + 1..len]);
    Some((code, Address::from_string(&issuer)))
}

fn best_report(env: &Env, sac: &Address, min_tier: Tier) -> Option<ReserveReport> {
    let mut best: Option<ReserveReport> = None;
    // Strongest tier first, so a tie on `as_of` keeps the stronger report.
    for tier in Tier::ALL_DESC {
        if tier < min_tier {
            continue;
        }
        if let Some(report) =
            read_persistent::<ReserveReport>(env, &DataKey::Report(sac.clone(), tier))
        {
            let newer = match &best {
                Some(current) => report.as_of > current.as_of,
                None => true,
            };
            if newer {
                best = Some(report);
            }
        }
    }
    best
}

#[contractimpl]
impl CoverageLedger {
    pub fn __constructor(env: Env, admin: Address, registry: Address) {
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Registry, &registry);
        bump_instance(&env);
    }

    pub fn admin(env: Env) -> Address {
        admin(&env)
    }

    pub fn registry(env: Env) -> Address {
        registry(&env)
    }

    /// List a Stellar-issued asset by its Stellar Asset Contract address.
    /// Permissionless: the code and issuer are read from the SAC itself.
    pub fn list_asset(env: Env, sac: Address) -> AssetRecord {
        if sac.executable() != Some(Executable::StellarAsset) {
            panic_with_error!(&env, LedgerError::NotAStellarAsset);
        }
        let key = DataKey::Asset(sac.clone());
        if env.storage().persistent().has(&key) {
            panic_with_error!(&env, LedgerError::AlreadyListed);
        }
        let name = token::TokenClient::new(&env, &sac).name();
        let (code, issuer) = match parse_sac_name(&env, &name) {
            Some(parts) => parts,
            None => panic_with_error!(&env, LedgerError::NotAStellarAsset),
        };
        let record = AssetRecord {
            sac: sac.clone(),
            code: code.clone(),
            issuer: issuer.clone(),
            listed_at: env.ledger().timestamp(),
        };
        write_persistent(&env, &key, &record);
        bump_instance(&env);
        AssetListed { sac, code, issuer }.publish(&env);
        record
    }

    /// Record the circulating supply of `sac` as computed at `ledger`.
    pub fn post_supply(
        env: Env,
        poster: Address,
        sac: Address,
        amount: i128,
        ledger: u32,
        breakdown_hash: BytesN<32>,
    ) {
        poster.require_auth();
        let role = RegistryClient::new(&env, &registry(&env)).role_of(&poster);
        if role != Some(Role::SupplyPoster) {
            panic_with_error!(&env, LedgerError::NotSupplyPoster);
        }
        require_listed(&env, &sac);
        if amount < 0 {
            panic_with_error!(&env, LedgerError::InvalidAmount);
        }
        if ledger > env.ledger().sequence() {
            panic_with_error!(&env, LedgerError::FutureLedger);
        }
        let key = DataKey::Supply(sac.clone());
        if let Some(previous) = read_persistent::<SupplySnapshot>(&env, &key) {
            if ledger <= previous.ledger {
                panic_with_error!(&env, LedgerError::StaleLedger);
            }
        }
        let snapshot = SupplySnapshot {
            amount,
            ledger,
            timestamp: env.ledger().timestamp(),
            breakdown_hash: breakdown_hash.clone(),
            poster: poster.clone(),
        };
        write_persistent(&env, &key, &snapshot);
        bump_instance(&env);
        SupplyPosted {
            sac,
            amount,
            ledger,
            breakdown_hash,
            poster,
        }
        .publish(&env);
    }

    /// Record a reserve figure for `sac`. The reporter's identity sets the
    /// tier: the asset's issuer account, a registered auditor, or a
    /// registered transcriber.
    pub fn post_reserve(
        env: Env,
        reporter: Address,
        sac: Address,
        amount: i128,
        as_of: u64,
        doc_hash: BytesN<32>,
        doc_uri: String,
    ) -> Tier {
        reporter.require_auth();
        let asset = require_listed(&env, &sac);
        let tier = if reporter == asset.issuer {
            Tier::IssuerSigned
        } else {
            match RegistryClient::new(&env, &registry(&env)).role_of(&reporter) {
                Some(Role::Auditor) => Tier::AuditorSigned,
                Some(Role::Transcriber) => Tier::Transcribed,
                _ => panic_with_error!(&env, LedgerError::NotReporter),
            }
        };
        if amount < 0 {
            panic_with_error!(&env, LedgerError::InvalidAmount);
        }
        if doc_uri.is_empty() || doc_uri.len() > MAX_URI_LEN {
            panic_with_error!(&env, LedgerError::InvalidUri);
        }
        let now = env.ledger().timestamp();
        if as_of > now {
            panic_with_error!(&env, LedgerError::FutureTimestamp);
        }
        let key = DataKey::Report(sac.clone(), tier);
        if let Some(previous) = read_persistent::<ReserveReport>(&env, &key) {
            if as_of <= previous.as_of {
                panic_with_error!(&env, LedgerError::StaleReport);
            }
        }
        let report = ReserveReport {
            amount,
            as_of,
            posted_at: now,
            tier,
            reporter: reporter.clone(),
            doc_hash: doc_hash.clone(),
            doc_uri: doc_uri.clone(),
        };
        write_persistent(&env, &key, &report);
        bump_instance(&env);
        ReservePosted {
            sac,
            tier,
            amount,
            as_of,
            reporter,
            doc_hash,
            doc_uri,
        }
        .publish(&env);
        tier
    }

    pub fn get_asset(env: Env, sac: Address) -> Option<AssetRecord> {
        read_persistent(&env, &DataKey::Asset(sac))
    }

    pub fn get_supply(env: Env, sac: Address) -> Option<SupplySnapshot> {
        read_persistent(&env, &DataKey::Supply(sac))
    }

    pub fn get_report(env: Env, sac: Address, tier: Tier) -> Option<ReserveReport> {
        read_persistent(&env, &DataKey::Report(sac, tier))
    }

    /// Latest supply against the freshest report at `min_tier` or stronger.
    pub fn coverage(env: Env, sac: Address, min_tier: Tier) -> Option<Coverage> {
        let supply = read_persistent::<SupplySnapshot>(&env, &DataKey::Supply(sac.clone()))?;
        let report = best_report(&env, &sac, min_tier)?;
        Some(Coverage {
            bps: coverage_bps(report.amount, supply.amount),
            supply: supply.amount,
            reserves: report.amount,
            tier: report.tier,
            supply_ledger: supply.ledger,
            supply_timestamp: supply.timestamp,
            report_as_of: report.as_of,
        })
    }

    /// True when both the supply snapshot and the chosen report are at most
    /// `max_age` seconds old and coverage is at least `min_bps`.
    pub fn is_covered(env: Env, sac: Address, min_bps: u32, max_age: u64, min_tier: Tier) -> bool {
        let coverage = match Self::coverage(env.clone(), sac, min_tier) {
            Some(c) => c,
            None => return false,
        };
        let now = env.ledger().timestamp();
        let fresh = |t: u64| now.saturating_sub(t) <= max_age;
        fresh(coverage.supply_timestamp) && fresh(coverage.report_as_of) && coverage.bps >= min_bps
    }

    pub fn transfer_admin(env: Env, new_admin: Address) {
        admin(&env).require_auth();
        new_admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &new_admin);
        bump_instance(&env);
        AdminTransferred { new_admin }.publish(&env);
    }

    pub fn upgrade(env: Env, wasm_hash: BytesN<32>) {
        admin(&env).require_auth();
        env.deployer()
            .update_current_contract(ContractExecutable::Wasm(wasm_hash));
    }
}

#[cfg(test)]
mod test;
