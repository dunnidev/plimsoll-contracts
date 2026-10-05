#!/usr/bin/env bash
# Deploy Plimsoll to Stellar testnet in dependency order and seed demo data.
#
#   ./scripts/deploy-testnet.sh
#
# Requires: stellar CLI >= 28, built wasm (stellar contract build).
# Creates (or reuses) these identities in the global keystore:
#   plimsoll-admin    contract admin
#   plimsoll-poster   supply poster used by the indexer
#   plimsoll-auditor  demo auditor
#   plimsoll-issuer   issuer of the demo asset PUSD
#   plimsoll-holder   demo holder of PUSD
set -euo pipefail

NETWORK=testnet
WASM_DIR=target/wasm32v1-none/release
DEMO_CODE=PUSD
# Circle's testnet USDC issuer.
TESTNET_USDC_ISSUER=GBBD47IF6LWK7P7MDEVSCWR7DPUWV3NY3DTQEVFL4NAT4AQH3ZLLFLA5
OUT=deployments/testnet.json

key() {
  local name=$1
  if ! stellar keys address "$name" >/dev/null 2>&1; then
    stellar keys generate "$name" --network "$NETWORK" --fund >/dev/null
  else
    stellar keys fund "$name" --network "$NETWORK" >/dev/null 2>&1 || true
  fi
  stellar keys address "$name"
}

echo "==> identities"
ADMIN=$(key plimsoll-admin)
POSTER=$(key plimsoll-poster)
AUDITOR=$(key plimsoll-auditor)
ISSUER=$(key plimsoll-issuer)
HOLDER=$(key plimsoll-holder)
echo "admin   $ADMIN"
echo "poster  $POSTER"
echo "auditor $AUDITOR"
echo "issuer  $ISSUER"
echo "holder  $HOLDER"

echo "==> build"
stellar contract build >/dev/null

echo "==> deploy reporter-registry"
REGISTRY=$(stellar contract deploy --wasm "$WASM_DIR/reporter_registry.wasm" \
  --source plimsoll-admin --network "$NETWORK" -- --admin "$ADMIN")
echo "registry $REGISTRY"

echo "==> deploy coverage-ledger"
LEDGER=$(stellar contract deploy --wasm "$WASM_DIR/coverage_ledger.wasm" \
  --source plimsoll-admin --network "$NETWORK" -- --admin "$ADMIN" --registry "$REGISTRY")
echo "ledger $LEDGER"

echo "==> register reporters"
stellar contract invoke --id "$REGISTRY" --source plimsoll-admin --network "$NETWORK" -- \
  set_reporter --reporter "$POSTER" --role 3 --name "Plimsoll indexer" >/dev/null
stellar contract invoke --id "$REGISTRY" --source plimsoll-admin --network "$NETWORK" -- \
  set_reporter --reporter "$AUDITOR" --role 1 --name "Demo Auditor (testnet)" >/dev/null

echo "==> issue demo asset $DEMO_CODE"
stellar tx new change-trust --source plimsoll-holder --network "$NETWORK" \
  --line "$DEMO_CODE:$ISSUER" >/dev/null
stellar tx new payment --source plimsoll-issuer --network "$NETWORK" \
  --destination "$HOLDER" --asset "$DEMO_CODE:$ISSUER" --amount 10000000000000 >/dev/null
PUSD_SAC=$(stellar contract asset deploy --asset "$DEMO_CODE:$ISSUER" \
  --source plimsoll-admin --network "$NETWORK" 2>/dev/null \
  || stellar contract id asset --asset "$DEMO_CODE:$ISSUER" --network "$NETWORK")
echo "PUSD SAC $PUSD_SAC"

echo "==> testnet USDC SAC"
USDC_SAC=$(stellar contract id asset --asset "USDC:$TESTNET_USDC_ISSUER" --network "$NETWORK")
stellar contract asset deploy --asset "USDC:$TESTNET_USDC_ISSUER" \
  --source plimsoll-admin --network "$NETWORK" >/dev/null 2>&1 || true
echo "USDC SAC $USDC_SAC"

echo "==> list assets"
stellar contract invoke --id "$LEDGER" --source plimsoll-admin --network "$NETWORK" -- \
  list_asset --sac "$PUSD_SAC" >/dev/null
stellar contract invoke --id "$LEDGER" --source plimsoll-admin --network "$NETWORK" -- \
  list_asset --sac "$USDC_SAC" >/dev/null

echo "==> deploy covered-vault (PUSD, 100%, 7 days, issuer-signed or better)"
VAULT=$(stellar contract deploy --wasm "$WASM_DIR/covered_vault.wasm" \
  --source plimsoll-admin --network "$NETWORK" -- \
  --ledger "$LEDGER" --asset "$PUSD_SAC" --min_bps 10000 --max_age 604800 --min_tier 2)
echo "vault $VAULT"

LATEST=$(curl -s -X POST https://soroban-testnet.stellar.org \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"getLatestLedger"}' \
  | sed -E 's/.*"sequence":([0-9]+).*/\1/')

mkdir -p deployments
cat > "$OUT" <<JSON
{
  "network": "testnet",
  "rpcUrl": "https://soroban-testnet.stellar.org",
  "networkPassphrase": "Test SDF Network ; September 2015",
  "deployedAtLedger": $LATEST,
  "contracts": {
    "reporterRegistry": "$REGISTRY",
    "coverageLedger": "$LEDGER",
    "coveredVault": "$VAULT"
  },
  "assets": {
    "PUSD": { "code": "$DEMO_CODE", "issuer": "$ISSUER", "sac": "$PUSD_SAC" },
    "USDC": { "code": "USDC", "issuer": "$TESTNET_USDC_ISSUER", "sac": "$USDC_SAC" }
  },
  "accounts": {
    "admin": "$ADMIN",
    "supplyPoster": "$POSTER",
    "demoAuditor": "$AUDITOR",
    "demoIssuer": "$ISSUER",
    "demoHolder": "$HOLDER"
  }
}
JSON

cat <<EOF

================ Plimsoll testnet deployment ================
REPORTER_REGISTRY_ID=$REGISTRY
COVERAGE_LEDGER_ID=$LEDGER
COVERED_VAULT_ID=$VAULT
PUSD_SAC=$PUSD_SAC
USDC_SAC=$USDC_SAC
DEPLOYED_AT_LEDGER=$LATEST
=============================================================
Written to $OUT
EOF
