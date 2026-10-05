# Instructions for coding agents — plimsoll-contracts

You are working on the Soroban contracts of Plimsoll as a senior Soroban
engineer. Write complete, tested code. No placeholders, no stubs, no TODOs.

## Source of truth

`docs/SPEC.md` defines every type, function, auth rule, error code and event.
If a task changes behaviour, update SPEC.md first, in its own commit, then the
code. Never renumber an existing error code or change an event's fields
without an issue that says so: the indexer decodes them.

## Stack

| Item | Version |
| --- | --- |
| Rust | stable, edition 2021 |
| soroban-sdk | 28.0.0 (workspace dependency) |
| Target | `wasm32v1-none` |
| Stellar CLI | 28.1.0 (`stellar contract build` is required by soroban-sdk 28) |

## Layout

```
crates/types/src/lib.rs              shared types, bps math, TTL constants, client traits
contracts/reporter-registry/src/     lib.rs + test.rs
contracts/coverage-ledger/src/       lib.rs + test.rs
contracts/covered-vault/src/         lib.rs + test.rs
scripts/deploy-testnet.sh            dependency-ordered deployment
deployments/testnet.json             current ids
```

## Patterns to follow

- **Storage:** admin/config in `instance()`; everything else `persistent()`.
  Extend TTL with `PERSISTENT_THRESHOLD` / `PERSISTENT_EXTEND_TO` on every
  write and on reads of existing entries. Bump instance TTL on every write.
- **Auth:** `addr.require_auth()` on the address whose authority is used,
  before any state change.
- **Errors:** one `#[contracterror] #[repr(u32)]` enum per contract, raised with
  `panic_with_error!`. New codes are appended.
- **Events:** `#[contractevent]` structs with `#[topic]` fields, published
  after the state change.
- **Cross-contract calls:** through the `#[contractclient]` traits in
  `plimsoll-types`, not `contractimport!`.
- **Math:** `i128` amounts in smallest units, basis points as `u32`,
  `checked_*` arithmetic. No floats.
- **Upgrades:** `env.deployer().update_current_contract(ContractExecutable::Wasm(hash))`.

## Tests

Every public function: one success test and one test per error it can return,
using `client.try_*` and `Err(Ok(Error::X.into()))`. Use
`env.register_stellar_asset_contract_v2(admin)` for assets and take the issuer
from `handle.issuer().address()` (a G-account), not `Address::generate`.

## Before every commit

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
stellar contract build
```

## Git rules

- Stage named files only. Never `git add .`.
- One commit per logical unit (one function, one type file, one test block).
- Conventional Commits: `type(scope): description`; scopes `types`, `registry`, `ledger`, `vault`, `spec`, `deploy`, `ci`.
- Push after each commit.

## Do not

- [ ] use `unwrap()` / `expect()` on caller-controlled data outside tests
- [ ] add functions that no user flow in SPEC.md needs
- [ ] store history on-chain (events carry history)
- [ ] change the canonical meaning of `bps`, tiers or roles
- [ ] commit secrets or `.stellar/` identities
