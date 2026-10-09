#!/usr/bin/env bash
# Seed an UNDER-BACKED demo asset on testnet, to show Plimsoll refusing one.
#
#   ./scripts/seed-underbacked-demo.sh
#
# Requires deployments/testnet.json from deploy-testnet.sh, and the doc
# docs/demo/qusd-reserves-2026-10-08.md pushed to GitHub (its hash goes
# on-chain). Creates or reuses these identities:
#   plimsoll-issuer2      issuer of QUSD
#   plimsoll-transcriber  registered Transcriber
# Re-running is safe: steps that already happened are skipped.
set -euo pipefail

NETWORK=testnet
DEP=deployments/testnet.json
CODE=QUSD
SUPPLY_UNITS=500000          # issued to the demo holder
RESERVES_STROOPS=4800000000000  # 480,000.0000000 -> 96%
DOC_PATH=docs/demo/qusd-reserves-2026-10-08.md
DOC_URI="https://raw.githubusercontent.com/plimsoll-protocol/plimsoll-contracts/main/$DOC_PATH"
AS_OF_DATE="2026-10-08 00:00:00"

json() { python -c "import json,sys;d=json.load(open('$DEP'));print(eval(sys.argv[1]))" "$1"; }
LEDGER=$(json "d['contracts']['coverageLedger']")
REGISTRY=$(json "d['contracts']['reporterRegistry']")

key() {
  stellar keys address "$1" >/dev/null 2>&1 || stellar keys generate "$1" --network "$NETWORK" --fund >/dev/null
  stellar keys address "$1"
}
invoke() { stellar contract invoke --network "$NETWORK" "$@"; }

echo "==> identities"
ISSUER=$(key plimsoll-issuer2)
TRANSCRIBER=$(key plimsoll-transcriber)
HOLDER=$(stellar keys address plimsoll-holder)
echo "issuer2     $ISSUER"
echo "transcriber $TRANSCRIBER"

echo "==> register transcriber (role 2)"
invoke --source plimsoll-admin --id "$REGISTRY" -- \
  set_reporter --reporter "$TRANSCRIBER" --role 2 --name "Demo Transcriber (testnet)" >/dev/null

echo "==> issue $SUPPLY_UNITS $CODE to the demo holder"
if ! curl -s "https://horizon-testnet.stellar.org/accounts/$HOLDER" | grep -q "\"asset_code\": \"$CODE\""; then
  stellar tx new change-trust --source plimsoll-holder --network "$NETWORK" --line "$CODE:$ISSUER" >/dev/null
  stellar tx new payment --source plimsoll-issuer2 --network "$NETWORK" \
    --destination "$HOLDER" --asset "$CODE:$ISSUER" --amount $((SUPPLY_UNITS * 10000000)) >/dev/null
fi

echo "==> asset contract and listing"
SAC=$(stellar contract asset deploy --asset "$CODE:$ISSUER" --source plimsoll-admin --network "$NETWORK" 2>/dev/null \
  || stellar contract id asset --asset "$CODE:$ISSUER" --network "$NETWORK")
if [ "$(invoke --send=no --source plimsoll-admin --id "$LEDGER" -- get_asset --sac "$SAC" 2>/dev/null | tail -1)" = "null" ]; then
  invoke --source plimsoll-admin --id "$LEDGER" -- list_asset --sac "$SAC" >/dev/null
else
  echo "(already listed)"
fi
echo "QUSD SAC $SAC"

echo "==> transcribed reserve report (96%)"
DOC_HASH=$(curl -sL "$DOC_URI" | sha256sum | cut -c1-64)
[ "$DOC_HASH" = "$(sha256sum < "$DOC_PATH" | cut -c1-64)" ] || { echo "served document differs from $DOC_PATH; push it first"; exit 1; }
AS_OF=$(date -u -d "$AS_OF_DATE" +%s)
if [ "$AS_OF" -gt "$(date -u +%s)" ]; then
  echo "as_of $AS_OF_DATE UTC is in the future; the contract would reject it (FutureTimestamp). Re-run after it."
  exit 1
fi
EXISTING=$(invoke --send=no --source plimsoll-admin --id "$LEDGER" -- get_report --sac "$SAC" --tier 1 2>/dev/null | tail -1)
if echo "$EXISTING" | grep -q "\"as_of\":$AS_OF"; then
  echo "(transcribed report for this as_of already posted)"
else
  invoke --source plimsoll-transcriber --id "$LEDGER" -- post_reserve \
    --reporter "$TRANSCRIBER" --sac "$SAC" --amount "$RESERVES_STROOPS" --as_of "$AS_OF" \
    --doc_hash "$DOC_HASH" --doc_uri "$DOC_URI" >/dev/null
fi

echo "==> QUSD example vault (100%, 31 days, any tier)"
VAULT2=$(json "d.get('contracts',{}).get('coveredVaultQusd','')")
if [ -z "$VAULT2" ]; then
  VAULT2=$(stellar contract deploy --wasm target/wasm32v1-none/release/covered_vault.wasm \
    --source plimsoll-admin --network "$NETWORK" -- \
    --ledger "$LEDGER" --asset "$SAC" --min_bps 10000 --max_age 2678400 --min_tier 1)
fi
echo "QUSD vault $VAULT2"

python - "$DEP" "$SAC" "$ISSUER" "$VAULT2" "$TRANSCRIBER" <<'EOF'
import json, sys
path, sac, issuer, vault, transcriber = sys.argv[1:]
d = json.load(open(path))
d["assets"]["QUSD"] = {"code": "QUSD", "issuer": issuer, "sac": sac}
d["contracts"]["coveredVaultQusd"] = vault
d["accounts"]["demoIssuer2"] = issuer
d["accounts"]["demoTranscriber"] = transcriber
with open(path, "w", newline="\n") as f:
    json.dump(d, f, indent=2)
    f.write("\n")
EOF

cat <<EOF

================ under-backed demo ================
QUSD_SAC=$SAC
QUSD_VAULT_ID=$VAULT2
Supply is posted by the indexer within ~15 minutes.
Then try:  stellar contract invoke --network testnet --source plimsoll-holder \\
  --id $VAULT2 -- deposit --from $HOLDER --amount 1000000000
Expected:  Error(Contract, #2)  NotCovered
===================================================
EOF
