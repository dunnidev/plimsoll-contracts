<p align="center">
  <img src="docs/banner.svg" alt="Plimsoll" width="100%" />
</p>

<p align="center">
  <a href="https://github.com/plimsoll-protocol/plimsoll-contracts/actions/workflows/ci.yml"><img src="https://github.com/plimsoll-protocol/plimsoll-contracts/actions/workflows/ci.yml/badge.svg" alt="CI" /></a>
  <img src="https://img.shields.io/badge/soroban--sdk-28.0.0-0f1d2b" alt="soroban-sdk 28" />
  <img src="https://img.shields.io/badge/network-testnet-c8341f" alt="testnet" />
  <a href="https://plimsoll-protocol.gitbook.io/plimsoll-protocol-docs/"><img src="https://img.shields.io/badge/docs-gitbook-0f1d2b" alt="Docs" /></a>
  <img src="https://img.shields.io/badge/license-Apache--2.0-blue" alt="Apache-2.0" />
  <img src="https://img.shields.io/badge/audit-none-lightgrey" alt="unaudited" />
</p>

# Plimsoll contracts

Soroban contracts that answer one question about a Stellar-issued asset: **how
much of what is circulating is backed, according to whom, and how recently?**

Circulating supply is computed from public ledger data and posted on-chain with
a hash of its breakdown. Reserve figures are posted by the issuer's own
account, a registered auditor, or a registered transcriber, and the signer sets
the report's tier. Any contract can then call:

```rust
is_covered(asset, min_bps, max_age, min_tier) -> bool
```

and refuse an asset that is under-backed or whose figures are out of date.

| Repo | What it is |
| --- | --- |
| **plimsoll-contracts** (this repo) | Rust/Soroban contracts |
| [plimsoll-app](https://github.com/plimsoll-protocol/plimsoll-app) | Web app and TypeScript SDK |
| [plimsoll-indexer](https://github.com/plimsoll-protocol/plimsoll-indexer) | Go service: supply poster, event indexer, read API |

**Live app:** https://plimsoll-protocol.github.io/plimsoll-app/  
**Docs:** https://plimsoll-protocol.gitbook.io/plimsoll-protocol-docs/

## Maintainers

| Maintainer | GitHub | Contact |
| --- | --- | --- |
| dunnidev | [@dunnidev](https://github.com/dunnidev) | [GitHub Discussions](https://github.com/plimsoll-protocol/plimsoll-contracts/discussions) |

Questions, ideas and contributor coordination happen in
[Discussions](https://github.com/plimsoll-protocol/plimsoll-contracts/discussions).

## Contracts

| Contract | Responsibility |
| --- | --- |
| [`reporter-registry`](contracts/reporter-registry) | Who may speak: auditors, transcribers and supply posters, managed by an admin. |
| [`coverage-ledger`](contracts/coverage-ledger) | What was said: asset listings, supply snapshots, reserve reports, and the `coverage` / `is_covered` queries. |
| [`covered-vault`](contracts/covered-vault) | Reference consumer: refuses deposits while the asset is not covered; never blocks withdrawals. |
| [`plimsoll-types`](crates/types) | Shared types, basis-point math and cross-contract client traits. |

```
reporter-registry  ◀── role_of ──  coverage-ledger  ◀── is_covered ──  covered-vault
                                         │
                                         └── name() ──▶ Stellar Asset Contract
```

The full function-by-function specification is in [docs/SPEC.md](docs/SPEC.md).

### Why it is Stellar-specific

- **Issued assets are a protocol feature.** Supply for any classic asset can be summed from trustlines, claimable balances, liquidity pools and contract balances, with no cooperation from the issuer.
- **The issuer account can sign Soroban auth.** A report signed by `asset.issuer` is unforgeable, which is what the Issuer-signed tier relies on.
- **Every asset has a built-in Stellar Asset Contract** whose `name()` is `CODE:ISSUER`. `list_asset` accepts only genuine SACs and parses that name, so a look-alike token cannot be listed under a real issuer.

### Tiers

| Tier | Value | Who signs |
| --- | --- | --- |
| Auditor-signed | 3 | Address registered with the `Auditor` role |
| Issuer-signed | 2 | The asset's issuer account |
| Transcribed | 1 | Address registered with the `Transcriber` role |

## Testnet deployment

| Contract | Address |
| --- | --- |
| reporter-registry | [`CCZCBR7MGSW5LGIEUQR5TU5Z7JQWBDEESB3RGPTRPMCBUMOISRPY7GOB`](https://stellar.expert/explorer/testnet/contract/CCZCBR7MGSW5LGIEUQR5TU5Z7JQWBDEESB3RGPTRPMCBUMOISRPY7GOB) |
| coverage-ledger | [`CC2QQ7R4FLPHZP7KA5AO4IASXXICTG6N4GQXCXITTEBNYQEYTYXKMN4D`](https://stellar.expert/explorer/testnet/contract/CC2QQ7R4FLPHZP7KA5AO4IASXXICTG6N4GQXCXITTEBNYQEYTYXKMN4D) |
| covered-vault | [`CC3SUECAJSAC4PBV4QLUII7X5Y4WNVL5YGTBAQ4CHCAH6R3QLRYJUZHQ`](https://stellar.expert/explorer/testnet/contract/CC3SUECAJSAC4PBV4QLUII7X5Y4WNVL5YGTBAQ4CHCAH6R3QLRYJUZHQ) |

Listed demo assets: `PUSD` (demo issuer, SAC `CCHPT4…HIM7S`) and Circle's testnet `USDC`
(SAC `CBIELT…DAMA`). Full record: [deployments/testnet.json](deployments/testnet.json).

## Quick start

Requirements: Rust stable with the `wasm32v1-none` target, and
[Stellar CLI](https://developers.stellar.org/docs/tools/cli) 28 or later.

```bash
git clone https://github.com/plimsoll-protocol/plimsoll-contracts
cd plimsoll-contracts

cargo test                 # 53 tests across all crates
stellar contract build     # wasm in target/wasm32v1-none/release/
```

Deploy your own copy to testnet (creates and funds the identities it needs):

```bash
./scripts/deploy-testnet.sh
```

Query the live deployment:

```bash
stellar contract invoke --network testnet --send=no \
  --id CC2QQ7R4FLPHZP7KA5AO4IASXXICTG6N4GQXCXITTEBNYQEYTYXKMN4D -- \
  coverage --sac CCHPT4TEJDPZQDSUVW3NP6A35ROGV4WEFZKT45SKWYCFZSUB7R5HIM7S --min_tier 1
```

## Contributing

Issues labelled [`good first issue`](https://github.com/plimsoll-protocol/plimsoll-contracts/labels/good%20first%20issue)
are scoped for a first pull request. Read [CONTRIBUTING.md](CONTRIBUTING.md) before you start,
and comment on an issue to have it assigned.

Security reports: see [SECURITY.md](SECURITY.md). Do not open public issues for vulnerabilities.

## Contributors

<a href="https://github.com/plimsoll-protocol/plimsoll-contracts/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=plimsoll-protocol/plimsoll-contracts" alt="Contributors" />
</a>

## License

[Apache-2.0](LICENSE)
