#!/usr/bin/env bash
#
# Deploy the FENET outcome-market PoC to the Stellar testnet and run a full
# mint -> merge and mint -> set_outcome -> redeem round-trip on-chain.
#
# Requirements:
#   - stellar-cli v25.2.0+   (https://developers.stellar.org/docs/tools/cli)
#   - rustup target: wasm32v1-none
#
# Usage:
#   ./scripts/deploy.sh
#
# The script funds ephemeral identities via Friendbot, deploys the two WASM
# contracts (token + market), instantiates tEURC / YES / NO, wires the market
# as the minter of YES/NO, and exercises the contract. Contract IDs and
# stellar.expert links are printed at the end (record them in the README).

set -euo pipefail

NETWORK="${NETWORK:-testnet}"
RPC_URL="${RPC_URL:-https://soroban-testnet.stellar.org}"
PASSPHRASE="Test SDF Network ; September 2015"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

DECIMALS=7
NAME_COLLATERAL="Test EURC"
SYM_COLLATERAL="tEURC"

say() { printf '\n=== %s ===\n' "$1"; }
expert() { echo "  https://stellar.expert/explorer/testnet/contract/$1"; }

say "1/8 Configure network"
stellar network add "$NETWORK" \
  --rpc-url "$RPC_URL" \
  --network-passphrase "$PASSPHRASE" 2>/dev/null || true

say "2/8 Create & fund identities (admin = market resolver, user = trader)"
for who in admin user; do
  stellar keys generate --network "$NETWORK" --fund "$who" 2>/dev/null || true
done
ADMIN=$(stellar keys address admin)
USER=$(stellar keys address user)
echo "  admin: $ADMIN"
echo "  user:  $USER"

say "3/8 Build WASM (stellar contract build sets spec-shaking env)"
( cd "$ROOT" && stellar contract build )
TOKEN_WASM="$ROOT/target/wasm32v1-none/release/outcome_token.wasm"
MARKET_WASM="$ROOT/target/wasm32v1-none/release/market.wasm"

say "4/8 Upload WASM"
TOKEN_HASH=$(stellar contract upload --source admin --network "$NETWORK" --wasm "$TOKEN_WASM")
MARKET_HASH=$(stellar contract upload --source admin --network "$NETWORK" --wasm "$MARKET_WASM")
echo "  token wasm hash:  $TOKEN_HASH"
echo "  market wasm hash: $MARKET_HASH"

deploy_token() {
  # $1 name, $2 symbol, $3 minter
  stellar contract deploy --source admin --network "$NETWORK" --wasm-hash "$TOKEN_HASH" -- \
    --minter "$3" --decimals "$DECIMALS" --name "$1" --symbol "$2"
}

say "5/8 Deploy tEURC collateral + YES + NO (admin is initial minter)"
COLLATERAL=$(deploy_token "$NAME_COLLATERAL" "$SYM_COLLATERAL" "$ADMIN")
YES=$(deploy_token "YES Share" "YES" "$ADMIN")
NO=$(deploy_token "NO Share" "NO" "$ADMIN")
echo "  tEURC: $COLLATERAL"
echo "  YES:   $YES"
echo "  NO:    $NO"

say "6/8 Deploy market and hand YES/NO minting to it"
MARKET=$(stellar contract deploy --source admin --network "$NETWORK" --wasm-hash "$MARKET_HASH" -- \
  --admin "$ADMIN" --collateral "$COLLATERAL" --yes "$YES" --no "$NO")
echo "  market: $MARKET"
stellar contract invoke --source admin --network "$NETWORK" --id "$YES" -- set_minter --new_minter "$MARKET"
stellar contract invoke --source admin --network "$NETWORK" --id "$NO"  -- set_minter --new_minter "$MARKET"

say "7/8 Faucet tEURC to user, then mint a complete set and merge half"
stellar contract invoke --source admin --network "$NETWORK" --id "$COLLATERAL" -- mint --to "$USER" --amount 1000
stellar contract invoke --source user  --network "$NETWORK" --id "$MARKET" -- mint  --to "$USER"   --amount 100
stellar contract invoke --source user  --network "$NETWORK" --id "$MARKET" -- merge --from "$USER" --amount 40

say "8/8 Resolve YES and redeem the winning shares"
stellar contract invoke --source admin --network "$NETWORK" --id "$MARKET" -- set_outcome --winner Yes
stellar contract invoke --source user  --network "$NETWORK" --id "$MARKET" -- redeem --holder "$USER" --amount 60

say "Done. Contract IDs (record these in README.md):"
echo "tEURC  $COLLATERAL"; expert "$COLLATERAL"
echo "YES    $YES";        expert "$YES"
echo "NO     $NO";         expert "$NO"
echo "MARKET $MARKET";     expert "$MARKET"
