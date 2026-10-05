# Contributing to Plimsoll contracts

Thanks for helping. This file covers how to pick up work, what a pull request
must contain, and the standards the code is held to.

## Picking up an issue

1. Find an open issue. `good first issue` ones are small and self-contained.
2. Comment that you want it. Wait to be assigned before starting, so two people
   do not build the same thing.
3. If the issue is unclear, ask in the issue. Do not guess at contract behaviour.

During a Drips Wave, assignment works the same way; points are attached to the
issue, not to the pull request.

## Local setup

```bash
rustup target add wasm32v1-none
cargo test
stellar contract build
```

## Pull request checklist

- [ ] One logical change per pull request. Link the issue (`Closes #12`).
- [ ] `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.
- [ ] New public functions have tests for the success path **and** every error they can return.
- [ ] If behaviour changed, [docs/SPEC.md](docs/SPEC.md) is updated in the same pull request.
- [ ] Commit messages use [Conventional Commits](https://www.conventionalcommits.org/): `feat(ledger): …`, `fix(registry): …`, `test(vault): …`.

## Code standards

- No `unwrap()` or `expect()` on user-controlled data outside tests. `expect` is
  allowed only for invariants set in the constructor (e.g. the admin key).
- Errors are `#[contracterror]` enums raised with `panic_with_error!`. Never
  renumber an existing error code.
- No floats. Ratios are basis points (`10_000 = 100%`) computed in `i128`.
- Every persistent write extends TTL with the constants in `plimsoll-types`.
- Every state change emits a `#[contractevent]`. The indexer depends on them;
  changing an event's fields is a breaking change and needs an issue first.
- `require_auth()` on the address whose authority is used, before any write.

## AI-assisted contributions

Allowed, but you must understand and test every line you submit. Untested or
unexplained generated code will be closed, in line with the Drips Wave rules.

## Code of conduct

Be direct and kind. Review the code, not the person.
