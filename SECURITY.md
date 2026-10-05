# Security policy

## Audit status

**These contracts have not been audited.** They are deployed on Stellar
testnet only. Do not use them to protect real funds.

## Reporting a vulnerability

Report privately through GitHub:
[Security → Report a vulnerability](https://github.com/dunnidev/plimsoll-contracts/security/advisories/new).

Please include:

- the contract and function affected,
- a description of the impact,
- steps or a test case that reproduces it.

Do not open a public issue or pull request for a vulnerability. We aim to
acknowledge reports within 72 hours and to agree a disclosure date with you.

## Scope

In scope:

- `reporter-registry`, `coverage-ledger`, `covered-vault` and `plimsoll-types`
- Authorisation bypasses (posting as a tier you do not hold, listing a fake asset)
- Logic errors that make `coverage` or `is_covered` return a wrong answer
- Storage/TTL issues that can lose or freeze state

Out of scope:

- The accuracy of reserve figures themselves. Plimsoll records who signed a
  figure; it does not audit the figure.
- Testnet resets and RPC availability.
- Issues in third-party dependencies with no exploitable path here (report
  those upstream).

## Known trust assumptions

- The registry admin decides who is an auditor, transcriber or supply poster.
- Supply snapshots are computed off-chain by a registered poster. They are
  verifiable (the breakdown hash is on-chain) but not proven on-chain.
