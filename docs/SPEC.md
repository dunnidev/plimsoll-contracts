# Plimsoll contract specification (v0.1)

This document is the source of truth for the contract layer. Code that
disagrees with it is a bug in one or the other; fix whichever is wrong and
update both in the same pull request.

## What the contracts do

Plimsoll answers one question about a Stellar-issued asset: *how much of what
is circulating is backed, according to whom, and how recently?*

- The **liability side** (circulating supply) is computed off-chain from public
  ledger data by a supply poster and recorded on-chain with a hash of the
  per-bucket breakdown, so anyone can recompute and compare.
- The **reserve side** is a signed report. Who signed it decides its tier:
  the asset's own issuer account, a registered auditor, or a registered
  transcriber who copied a figure from a published report.
- Any contract can ask `is_covered(asset, min_bps, max_age, min_tier)` and get
  a yes or no, then refuse the asset when the answer is no.

## Contracts and dependency order

```
plimsoll-types   (library crate: shared types, cross-contract client traits)
      │
      ├── reporter-registry   (no contract dependencies)
      │
      ├── coverage-ledger     ──calls──▶ reporter-registry.role_of
      │                       ──calls──▶ <SAC>.name
      │
      └── covered-vault       ──calls──▶ coverage-ledger.is_covered
                              ──calls──▶ <SAC>.transfer
```

Build and deploy order: `reporter-registry` → `coverage-ledger` →
`covered-vault`. The vault is a reference consumer, not part of the protocol;
it exists to show the integration in about 100 lines.

## Shared types (`crates/types`)

| Type | Kind | Fields / variants |
| --- | --- | --- |
| `Role` | `u32` enum | `Auditor = 1`, `Transcriber = 2`, `SupplyPoster = 3` |
| `Tier` | `u32` enum | `Transcribed = 1`, `IssuerSigned = 2`, `AuditorSigned = 3` (ordered: higher is stronger) |
| `ReporterInfo` | struct | `role: Role`, `name: String`, `added_at: u64` |
| `AssetRecord` | struct | `sac: Address`, `code: String`, `issuer: Address`, `listed_at: u64` |
| `SupplySnapshot` | struct | `amount: i128`, `ledger: u32`, `timestamp: u64`, `breakdown_hash: BytesN<32>`, `poster: Address` |
| `ReserveReport` | struct | `amount: i128`, `as_of: u64`, `posted_at: u64`, `tier: Tier`, `reporter: Address`, `doc_hash: BytesN<32>`, `doc_uri: String` |
| `Coverage` | struct | `bps: u32`, `supply: i128`, `reserves: i128`, `tier: Tier`, `supply_ledger: u32`, `supply_timestamp: u64`, `report_as_of: u64` |

Amounts are in the asset's smallest unit (Stellar classic assets: 7 decimals,
so `1_0000000` is one unit). No floats anywhere. Coverage is basis points:
`bps = reserves * 10_000 / supply`, rounded down, saturating at `u32::MAX`.
When supply is zero, `bps = u32::MAX` (nothing is owed, so nothing is
uncovered).

## reporter-registry

Decides who may speak. Holds no money.

**Storage**

| Key | Storage | Value |
| --- | --- | --- |
| `Admin` | instance | `Address` |
| `Reporter(Address)` | persistent, TTL extended on write and read | `ReporterInfo` |

**Functions**

| Function | Auth | Returns | Errors | Event |
| --- | --- | --- | --- | --- |
| `__constructor(admin: Address)` | — | — | — | — |
| `admin()` | — | `Address` | — | — |
| `set_reporter(reporter: Address, role: Role, name: String)` | admin | `()` | `InvalidName` (len 0 or > 64) | `reporter_set {reporter} role, name` |
| `revoke(reporter: Address)` | admin | `()` | `ReporterNotFound` | `reporter_revoked {reporter}` |
| `role_of(reporter: Address)` | — | `Option<Role>` | — | — |
| `get_reporter(reporter: Address)` | — | `Option<ReporterInfo>` | — | — |
| `transfer_admin(new_admin: Address)` | admin and new_admin | `()` | — | `admin_transferred {new_admin}` |
| `upgrade(wasm_hash: BytesN<32>)` | admin | `()` | — | — |

Errors (`RegistryError`): `ReporterNotFound = 1`, `InvalidName = 2`.

## coverage-ledger

Records what was said, and answers coverage questions.

**Storage**

| Key | Storage | Value |
| --- | --- | --- |
| `Admin` | instance | `Address` |
| `Registry` | instance | `Address` |
| `Asset(Address)` | persistent | `AssetRecord` |
| `Supply(Address)` | persistent | `SupplySnapshot` (latest only) |
| `Report(Address, Tier)` | persistent | `ReserveReport` (latest per tier) |

History lives in events. The indexer keeps the full series.

**Functions**

| Function | Auth | Returns | Errors | Event |
| --- | --- | --- | --- | --- |
| `__constructor(admin: Address, registry: Address)` | — | — | — | — |
| `list_asset(sac: Address)` | none (permissionless) | `AssetRecord` | `NotAStellarAsset`, `AlreadyListed` | `asset_listed {sac} code, issuer` |
| `post_supply(poster: Address, sac: Address, amount: i128, ledger: u32, breakdown_hash: BytesN<32>)` | poster | `()` | `NotSupplyPoster`, `AssetNotListed`, `InvalidAmount`, `FutureLedger`, `StaleLedger` | `supply_posted {sac} amount, ledger, breakdown_hash, poster` |
| `post_reserve(reporter: Address, sac: Address, amount: i128, as_of: u64, doc_hash: BytesN<32>, doc_uri: String)` | reporter | `Tier` | `NotReporter`, `AssetNotListed`, `InvalidAmount`, `FutureTimestamp`, `StaleReport`, `InvalidUri` | `reserve_posted {sac, tier} amount, as_of, reporter, doc_hash, doc_uri` |
| `get_asset(sac)` | — | `Option<AssetRecord>` | — | — |
| `get_supply(sac)` | — | `Option<SupplySnapshot>` | — | — |
| `get_report(sac, tier: Tier)` | — | `Option<ReserveReport>` | — | — |
| `coverage(sac, min_tier: Tier)` | — | `Option<Coverage>` | — | — |
| `is_covered(sac, min_bps: u32, max_age: u64, min_tier: Tier)` | — | `bool` | — | — |
| `registry()` / `admin()` | — | `Address` | — | — |
| `transfer_admin(new_admin)` | admin and new_admin | `()` | — | `admin_transferred` |
| `upgrade(wasm_hash)` | admin | `()` | — | — |

Rules:

- `list_asset` only accepts a genuine Stellar Asset Contract (the address's
  executable must be the built-in asset contract). The SAC's `name()` is
  `CODE:ISSUER`; the code and issuer are parsed from it, so nobody can list a
  look-alike token under a real issuer's name. Native XLM (`name() ==
  "native"`) is rejected: it has no issuer and no reserves.
- `post_reserve` tier resolution: if `reporter == asset.issuer` the tier is
  `IssuerSigned` (the classic issuer account signs the Soroban auth entry).
  Otherwise the registry role decides: `Auditor → AuditorSigned`,
  `Transcriber → Transcribed`. A `SupplyPoster` or unknown address gets
  `NotReporter`.
- Supply snapshots must move forward: `ledger` strictly greater than the stored
  one and not greater than the current ledger. Reports per tier must move
  forward: `as_of` strictly greater than the stored one and not in the future.
- `coverage(sac, min_tier)` picks, among tiers `>= min_tier` that have a
  report, the one with the latest `as_of` (ties go to the higher tier). It
  returns `None` if there is no supply snapshot or no qualifying report.
- `is_covered` is true only if `coverage` is `Some`, both the supply snapshot
  and the report are no older than `max_age` seconds, and `bps >= min_bps`.

Errors (`LedgerError`): `NotAStellarAsset = 1`, `AlreadyListed = 2`,
`AssetNotListed = 3`, `NotSupplyPoster = 4`, `NotReporter = 5`,
`InvalidAmount = 6`, `FutureLedger = 7`, `StaleLedger = 8`,
`FutureTimestamp = 9`, `StaleReport = 10`, `InvalidUri = 11`.

## covered-vault (reference consumer)

A deposit vault for one asset that refuses deposits while the asset is not
covered. Withdrawals are never blocked.

| Function | Auth | Returns | Errors | Event |
| --- | --- | --- | --- | --- |
| `__constructor(ledger: Address, asset: Address, min_bps: u32, max_age: u64, min_tier: Tier)` | — | — | — | — |
| `deposit(from: Address, amount: i128)` | from | `i128` new balance | `InvalidAmount`, `NotCovered` | `deposited {from} amount` |
| `withdraw(to: Address, amount: i128)` | to | `i128` new balance | `InvalidAmount`, `InsufficientBalance` | `withdrawn {to} amount` |
| `balance(of: Address)` | — | `i128` | — | — |
| `config()` | — | `VaultConfig` | — | — |

Errors (`VaultError`): `InvalidAmount = 1`, `NotCovered = 2`,
`InsufficientBalance = 3`.

## User flow → function map

| Step in the product | Function |
| --- | --- |
| Anyone adds an asset to the dashboard | `coverage-ledger.list_asset` |
| Indexer posts circulating supply every N minutes | `coverage-ledger.post_supply` |
| Issuer publishes monthly reserve figure | `coverage-ledger.post_reserve` (issuer account) |
| Auditor publishes attestation | `coverage-ledger.post_reserve` (auditor) |
| Volunteer copies a figure from a PDF report | `coverage-ledger.post_reserve` (transcriber) |
| Maintainer onboards an auditor / transcriber / poster | `reporter-registry.set_reporter` |
| Maintainer removes a bad actor | `reporter-registry.revoke` |
| Dashboard shows ratio and freshness | `coverage-ledger.coverage`, `get_supply`, `get_report` |
| Lending pool refuses an under-backed asset | `coverage-ledger.is_covered` (see `covered-vault`) |

## Out of scope for v0.1

- Soroban-native SEP-41 tokens (no standard total-supply function).
- On-chain dispute of supply figures. Watchers recompute off-chain; disputes
  are tracked as an open issue.
- Zero-knowledge proof of individual holder inclusion.
