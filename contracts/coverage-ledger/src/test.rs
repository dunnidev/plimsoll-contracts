extern crate std;

use super::*;
use reporter_registry::{ReporterRegistry, ReporterRegistryClient};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, BytesN, Env, String,
};

const T0: u64 = 1_700_000_000;
const UNIT: i128 = 10_000_000;

struct Fixture {
    env: Env,
    ledger: CoverageLedgerClient<'static>,
    registry: ReporterRegistryClient<'static>,
    sac: Address,
    issuer: Address,
    poster: Address,
    auditor: Address,
    transcriber: Address,
}

fn setup() -> Fixture {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(T0);
    env.ledger().set_sequence_number(1_000);

    let admin = Address::generate(&env);
    let registry_id = env.register(ReporterRegistry, (admin.clone(),));
    let registry = ReporterRegistryClient::new(&env, &registry_id);
    let ledger_id = env.register(CoverageLedger, (admin.clone(), registry_id.clone()));
    let ledger = CoverageLedgerClient::new(&env, &ledger_id);

    // The SAC admin is a contract address; the asset's issuer is the G-account
    // the test host creates, which is what `name()` reports.
    let sac_admin = Address::generate(&env);
    let handle = env.register_stellar_asset_contract_v2(sac_admin);
    let sac = handle.address();
    let issuer = handle.issuer().address();

    let poster = Address::generate(&env);
    let auditor = Address::generate(&env);
    let transcriber = Address::generate(&env);
    registry.set_reporter(
        &poster,
        &Role::SupplyPoster,
        &String::from_str(&env, "indexer"),
    );
    registry.set_reporter(
        &auditor,
        &Role::Auditor,
        &String::from_str(&env, "Acme Audit"),
    );
    registry.set_reporter(
        &transcriber,
        &Role::Transcriber,
        &String::from_str(&env, "volunteer"),
    );

    Fixture {
        env,
        ledger,
        registry,
        sac,
        issuer,
        poster,
        auditor,
        transcriber,
    }
}

fn hash(env: &Env, byte: u8) -> BytesN<32> {
    BytesN::from_array(env, &[byte; 32])
}

fn uri(env: &Env) -> String {
    String::from_str(env, "https://example.org/attestation-2026-09.pdf")
}

fn listed() -> Fixture {
    let f = setup();
    f.ledger.list_asset(&f.sac);
    f
}

// ---- list_asset -----------------------------------------------------------

#[test]
fn list_asset_reads_code_and_issuer_from_sac() {
    let f = setup();
    let record = f.ledger.list_asset(&f.sac);
    assert_eq!(record.issuer, f.issuer);
    assert_eq!(record.sac, f.sac);
    assert_eq!(record.listed_at, T0);
    assert!(!record.code.is_empty());
    assert_eq!(f.ledger.get_asset(&f.sac), Some(record));
}

#[test]
fn list_asset_twice_fails() {
    let f = listed();
    let res = f.ledger.try_list_asset(&f.sac);
    assert_eq!(res, Err(Ok(LedgerError::AlreadyListed.into())));
}

#[test]
fn list_asset_rejects_wasm_contract() {
    let f = setup();
    // The registry is a Wasm-style contract, not a Stellar Asset Contract.
    let res = f.ledger.try_list_asset(&f.registry.address);
    assert_eq!(res, Err(Ok(LedgerError::NotAStellarAsset.into())));
}

#[test]
fn list_asset_rejects_account_address() {
    let f = setup();
    let res = f.ledger.try_list_asset(&Address::generate(&f.env));
    assert!(res.is_err());
}

#[test]
fn parse_rejects_native_and_malformed() {
    let env = Env::default();
    assert!(parse_sac_name(&env, &String::from_str(&env, "native")).is_none());
    assert!(parse_sac_name(&env, &String::from_str(&env, "USDC")).is_none());
    let no_code = std::format!(":{}", "G".repeat(56));
    assert!(parse_sac_name(&env, &String::from_str(&env, &no_code)).is_none());
    let long_code = std::format!("ABCDEFGHIJKLM:{}", "G".repeat(56));
    assert!(parse_sac_name(&env, &String::from_str(&env, &long_code)).is_none());
}

#[test]
fn parse_accepts_real_name() {
    let env = Env::default();
    let name = "USDC:GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN";
    let (code, issuer) = parse_sac_name(&env, &String::from_str(&env, name)).unwrap();
    assert_eq!(code, String::from_str(&env, "USDC"));
    assert_eq!(
        issuer.to_string(),
        String::from_str(
            &env,
            "GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN"
        )
    );
}

// ---- post_supply ----------------------------------------------------------

#[test]
fn post_supply_stores_snapshot() {
    let f = listed();
    f.ledger
        .post_supply(&f.poster, &f.sac, &(1_000 * UNIT), &990, &hash(&f.env, 1));
    let s = f.ledger.get_supply(&f.sac).unwrap();
    assert_eq!(s.amount, 1_000 * UNIT);
    assert_eq!(s.ledger, 990);
    assert_eq!(s.timestamp, T0);
    assert_eq!(s.poster, f.poster);
}

#[test]
fn post_supply_requires_poster_role() {
    let f = listed();
    let res = f
        .ledger
        .try_post_supply(&f.auditor, &f.sac, &UNIT, &990, &hash(&f.env, 1));
    assert_eq!(res, Err(Ok(LedgerError::NotSupplyPoster.into())));
}

#[test]
fn post_supply_requires_listed_asset() {
    let f = setup();
    let res = f
        .ledger
        .try_post_supply(&f.poster, &f.sac, &UNIT, &990, &hash(&f.env, 1));
    assert_eq!(res, Err(Ok(LedgerError::AssetNotListed.into())));
}

#[test]
fn post_supply_rejects_negative() {
    let f = listed();
    let res = f
        .ledger
        .try_post_supply(&f.poster, &f.sac, &-1, &990, &hash(&f.env, 1));
    assert_eq!(res, Err(Ok(LedgerError::InvalidAmount.into())));
}

#[test]
fn post_supply_rejects_future_ledger() {
    let f = listed();
    let res = f
        .ledger
        .try_post_supply(&f.poster, &f.sac, &UNIT, &1_001, &hash(&f.env, 1));
    assert_eq!(res, Err(Ok(LedgerError::FutureLedger.into())));
}

#[test]
fn post_supply_must_move_forward() {
    let f = listed();
    f.ledger
        .post_supply(&f.poster, &f.sac, &UNIT, &990, &hash(&f.env, 1));
    let res = f
        .ledger
        .try_post_supply(&f.poster, &f.sac, &UNIT, &990, &hash(&f.env, 2));
    assert_eq!(res, Err(Ok(LedgerError::StaleLedger.into())));
}

#[test]
fn revoked_poster_cannot_post() {
    let f = listed();
    f.registry.revoke(&f.poster);
    let res = f
        .ledger
        .try_post_supply(&f.poster, &f.sac, &UNIT, &990, &hash(&f.env, 1));
    assert_eq!(res, Err(Ok(LedgerError::NotSupplyPoster.into())));
}

// ---- post_reserve ---------------------------------------------------------

#[test]
fn issuer_report_is_issuer_signed() {
    let f = listed();
    let tier = f.ledger.post_reserve(
        &f.issuer,
        &f.sac,
        &(1_000 * UNIT),
        &(T0 - 60),
        &hash(&f.env, 9),
        &uri(&f.env),
    );
    assert_eq!(tier, Tier::IssuerSigned);
    let r = f.ledger.get_report(&f.sac, &Tier::IssuerSigned).unwrap();
    assert_eq!(r.amount, 1_000 * UNIT);
    assert_eq!(r.reporter, f.issuer);
    assert_eq!(r.posted_at, T0);
}

#[test]
fn auditor_and_transcriber_tiers() {
    let f = listed();
    let a = f.ledger.post_reserve(
        &f.auditor,
        &f.sac,
        &UNIT,
        &T0,
        &hash(&f.env, 1),
        &uri(&f.env),
    );
    let t = f.ledger.post_reserve(
        &f.transcriber,
        &f.sac,
        &UNIT,
        &T0,
        &hash(&f.env, 2),
        &uri(&f.env),
    );
    assert_eq!(a, Tier::AuditorSigned);
    assert_eq!(t, Tier::Transcribed);
}

#[test]
fn stranger_cannot_report() {
    let f = listed();
    let res = f.ledger.try_post_reserve(
        &Address::generate(&f.env),
        &f.sac,
        &UNIT,
        &T0,
        &hash(&f.env, 1),
        &uri(&f.env),
    );
    assert_eq!(res, Err(Ok(LedgerError::NotReporter.into())));
}

#[test]
fn supply_poster_cannot_report_reserves() {
    let f = listed();
    let res = f.ledger.try_post_reserve(
        &f.poster,
        &f.sac,
        &UNIT,
        &T0,
        &hash(&f.env, 1),
        &uri(&f.env),
    );
    assert_eq!(res, Err(Ok(LedgerError::NotReporter.into())));
}

#[test]
fn report_rejects_future_as_of() {
    let f = listed();
    let res = f.ledger.try_post_reserve(
        &f.auditor,
        &f.sac,
        &UNIT,
        &(T0 + 1),
        &hash(&f.env, 1),
        &uri(&f.env),
    );
    assert_eq!(res, Err(Ok(LedgerError::FutureTimestamp.into())));
}

#[test]
fn report_must_move_forward_per_tier() {
    let f = listed();
    f.ledger.post_reserve(
        &f.auditor,
        &f.sac,
        &UNIT,
        &T0,
        &hash(&f.env, 1),
        &uri(&f.env),
    );
    let res = f.ledger.try_post_reserve(
        &f.auditor,
        &f.sac,
        &UNIT,
        &T0,
        &hash(&f.env, 2),
        &uri(&f.env),
    );
    assert_eq!(res, Err(Ok(LedgerError::StaleReport.into())));
    // A different tier at the same timestamp is fine.
    f.ledger.post_reserve(
        &f.issuer,
        &f.sac,
        &UNIT,
        &T0,
        &hash(&f.env, 3),
        &uri(&f.env),
    );
}

#[test]
fn report_rejects_empty_uri() {
    let f = listed();
    let res = f.ledger.try_post_reserve(
        &f.auditor,
        &f.sac,
        &UNIT,
        &T0,
        &hash(&f.env, 1),
        &String::from_str(&f.env, ""),
    );
    assert_eq!(res, Err(Ok(LedgerError::InvalidUri.into())));
}

#[test]
fn report_rejects_negative_amount() {
    let f = listed();
    let res =
        f.ledger
            .try_post_reserve(&f.auditor, &f.sac, &-5, &T0, &hash(&f.env, 1), &uri(&f.env));
    assert_eq!(res, Err(Ok(LedgerError::InvalidAmount.into())));
}

// ---- coverage / is_covered ------------------------------------------------

#[test]
fn coverage_none_without_data() {
    let f = listed();
    assert_eq!(f.ledger.coverage(&f.sac, &Tier::Transcribed), None);
    f.ledger
        .post_supply(&f.poster, &f.sac, &UNIT, &990, &hash(&f.env, 1));
    assert_eq!(f.ledger.coverage(&f.sac, &Tier::Transcribed), None);
}

#[test]
fn coverage_computes_bps() {
    let f = listed();
    f.ledger
        .post_supply(&f.poster, &f.sac, &(1_000 * UNIT), &990, &hash(&f.env, 1));
    f.ledger.post_reserve(
        &f.auditor,
        &f.sac,
        &(1_020 * UNIT),
        &(T0 - 100),
        &hash(&f.env, 2),
        &uri(&f.env),
    );
    let c = f.ledger.coverage(&f.sac, &Tier::Transcribed).unwrap();
    assert_eq!(c.bps, 10_200);
    assert_eq!(c.tier, Tier::AuditorSigned);
    assert_eq!(c.supply, 1_000 * UNIT);
    assert_eq!(c.reserves, 1_020 * UNIT);
    assert_eq!(c.supply_ledger, 990);
    assert_eq!(c.report_as_of, T0 - 100);
}

#[test]
fn coverage_prefers_freshest_report_at_or_above_min_tier() {
    let f = listed();
    f.ledger
        .post_supply(&f.poster, &f.sac, &(100 * UNIT), &990, &hash(&f.env, 1));
    // Older audit says 90%, newer transcription says 101%.
    f.ledger.post_reserve(
        &f.auditor,
        &f.sac,
        &(90 * UNIT),
        &(T0 - 1_000),
        &hash(&f.env, 2),
        &uri(&f.env),
    );
    f.ledger.post_reserve(
        &f.transcriber,
        &f.sac,
        &(101 * UNIT),
        &(T0 - 10),
        &hash(&f.env, 3),
        &uri(&f.env),
    );
    let any = f.ledger.coverage(&f.sac, &Tier::Transcribed).unwrap();
    assert_eq!(any.tier, Tier::Transcribed);
    assert_eq!(any.bps, 10_100);
    let audited = f.ledger.coverage(&f.sac, &Tier::AuditorSigned).unwrap();
    assert_eq!(audited.tier, Tier::AuditorSigned);
    assert_eq!(audited.bps, 9_000);
}

#[test]
fn coverage_tie_goes_to_stronger_tier() {
    let f = listed();
    f.ledger
        .post_supply(&f.poster, &f.sac, &(100 * UNIT), &990, &hash(&f.env, 1));
    f.ledger.post_reserve(
        &f.transcriber,
        &f.sac,
        &(50 * UNIT),
        &T0,
        &hash(&f.env, 2),
        &uri(&f.env),
    );
    f.ledger.post_reserve(
        &f.issuer,
        &f.sac,
        &(100 * UNIT),
        &T0,
        &hash(&f.env, 3),
        &uri(&f.env),
    );
    let c = f.ledger.coverage(&f.sac, &Tier::Transcribed).unwrap();
    assert_eq!(c.tier, Tier::IssuerSigned);
}

#[test]
fn is_covered_checks_ratio_age_and_tier() {
    let f = listed();
    f.ledger
        .post_supply(&f.poster, &f.sac, &(100 * UNIT), &990, &hash(&f.env, 1));
    f.ledger.post_reserve(
        &f.issuer,
        &f.sac,
        &(100 * UNIT),
        &T0,
        &hash(&f.env, 2),
        &uri(&f.env),
    );
    let day = 86_400;
    assert!(f
        .ledger
        .is_covered(&f.sac, &10_000, &day, &Tier::Transcribed));
    assert!(!f
        .ledger
        .is_covered(&f.sac, &10_001, &day, &Tier::Transcribed));
    assert!(!f
        .ledger
        .is_covered(&f.sac, &10_000, &day, &Tier::AuditorSigned));

    // Two days later both inputs are stale.
    f.env.ledger().set_timestamp(T0 + 2 * day);
    assert!(!f
        .ledger
        .is_covered(&f.sac, &10_000, &day, &Tier::Transcribed));
    assert!(f
        .ledger
        .is_covered(&f.sac, &10_000, &(3 * day), &Tier::Transcribed));
}

#[test]
fn is_covered_false_for_unlisted() {
    let f = setup();
    assert!(!f
        .ledger
        .is_covered(&f.sac, &0, &u64::MAX, &Tier::Transcribed));
}

#[test]
fn zero_supply_counts_as_covered() {
    let f = listed();
    f.ledger
        .post_supply(&f.poster, &f.sac, &0, &990, &hash(&f.env, 1));
    f.ledger
        .post_reserve(&f.issuer, &f.sac, &0, &T0, &hash(&f.env, 2), &uri(&f.env));
    let c = f.ledger.coverage(&f.sac, &Tier::Transcribed).unwrap();
    assert_eq!(c.bps, u32::MAX);
}

// ---- admin ----------------------------------------------------------------

#[test]
fn transfer_admin_changes_admin() {
    let f = setup();
    let next = Address::generate(&f.env);
    f.ledger.transfer_admin(&next);
    assert_eq!(f.ledger.admin(), next);
    assert_eq!(f.ledger.registry(), f.registry.address);
}
