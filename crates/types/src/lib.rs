//! Types shared by the Plimsoll contracts, and client traits for the
//! cross-contract calls between them.
#![no_std]

use soroban_sdk::{contractclient, contracttype, Address, BytesN, Env, String};

/// One day of ledgers at a 5-second close time.
pub const DAY_IN_LEDGERS: u32 = 17_280;
/// Extend a persistent entry once it has less than this many ledgers left.
pub const PERSISTENT_THRESHOLD: u32 = 30 * DAY_IN_LEDGERS;
/// ...back up to this many ledgers.
pub const PERSISTENT_EXTEND_TO: u32 = 90 * DAY_IN_LEDGERS;
/// Same policy for instance storage.
pub const INSTANCE_THRESHOLD: u32 = 30 * DAY_IN_LEDGERS;
pub const INSTANCE_EXTEND_TO: u32 = 90 * DAY_IN_LEDGERS;

/// Basis-point denominator: 10_000 bps = 100%.
pub const BPS_DENOMINATOR: i128 = 10_000;

/// What a registered address is allowed to say.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Role {
    Auditor = 1,
    Transcriber = 2,
    SupplyPoster = 3,
}

/// How much weight a reserve report carries. Ordered: higher is stronger.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Tier {
    /// Copied from a published document by a registered transcriber.
    Transcribed = 1,
    /// Signed by the asset's own issuer account.
    IssuerSigned = 2,
    /// Signed by a registered independent auditor.
    AuditorSigned = 3,
}

impl Tier {
    /// All tiers, strongest first.
    pub const ALL_DESC: [Tier; 3] = [Tier::AuditorSigned, Tier::IssuerSigned, Tier::Transcribed];
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReporterInfo {
    pub role: Role,
    pub name: String,
    pub added_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetRecord {
    pub sac: Address,
    pub code: String,
    pub issuer: Address,
    pub listed_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SupplySnapshot {
    pub amount: i128,
    pub ledger: u32,
    pub timestamp: u64,
    pub breakdown_hash: BytesN<32>,
    pub poster: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReserveReport {
    pub amount: i128,
    pub as_of: u64,
    pub posted_at: u64,
    pub tier: Tier,
    pub reporter: Address,
    pub doc_hash: BytesN<32>,
    pub doc_uri: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Coverage {
    pub bps: u32,
    pub supply: i128,
    pub reserves: i128,
    pub tier: Tier,
    pub supply_ledger: u32,
    pub supply_timestamp: u64,
    pub report_as_of: u64,
}

/// `reserves * 10_000 / supply`, rounded down, saturating at `u32::MAX`.
/// Zero supply means nothing is owed, so it is reported as `u32::MAX`.
pub fn coverage_bps(reserves: i128, supply: i128) -> u32 {
    if supply <= 0 {
        return u32::MAX;
    }
    match reserves.checked_mul(BPS_DENOMINATOR) {
        Some(scaled) => {
            let bps = scaled / supply;
            if bps > u32::MAX as i128 {
                u32::MAX
            } else if bps < 0 {
                0
            } else {
                bps as u32
            }
        }
        None => u32::MAX,
    }
}

/// The subset of reporter-registry that coverage-ledger calls.
#[contractclient(name = "RegistryClient")]
pub trait RegistryInterface {
    fn role_of(env: Env, reporter: Address) -> Option<Role>;
}

/// The subset of coverage-ledger that consumers call.
#[contractclient(name = "CoverageLedgerClient")]
pub trait CoverageLedgerInterface {
    fn coverage(env: Env, sac: Address, min_tier: Tier) -> Option<Coverage>;
    fn is_covered(env: Env, sac: Address, min_bps: u32, max_age: u64, min_tier: Tier) -> bool;
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn bps_rounds_down() {
        assert_eq!(coverage_bps(1, 3), 3_333);
        assert_eq!(coverage_bps(100, 100), 10_000);
        assert_eq!(coverage_bps(105, 100), 10_500);
    }

    #[test]
    fn bps_zero_supply_is_max() {
        assert_eq!(coverage_bps(0, 0), u32::MAX);
    }

    #[test]
    fn bps_saturates() {
        assert_eq!(coverage_bps(i128::MAX, 1), u32::MAX);
        assert_eq!(coverage_bps(u32::MAX as i128, 1), u32::MAX);
    }

    #[test]
    fn bps_negative_reserves_is_zero() {
        assert_eq!(coverage_bps(-5, 100), 0);
    }

    #[test]
    fn tiers_are_ordered() {
        assert!(Tier::AuditorSigned > Tier::IssuerSigned);
        assert!(Tier::IssuerSigned > Tier::Transcribed);
    }
}
