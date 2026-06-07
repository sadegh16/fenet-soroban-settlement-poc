# FENET Settlement PoC — Binary Outcome Market on Soroban

A proof-of-concept on-chain **settlement layer** for FENET prediction markets,
built as a Soroban (Stellar smart contracts) testnet deployment.

It demonstrates the core money-handling primitive of a prediction market:
collateral is locked to mint a *complete set* of YES + NO outcome shares, the
set can be redissolved back into collateral at any time before resolution, and
after an admin resolves the market the winning share redeems 1:1 for collateral
while the losing share becomes worthless.

This is deliberately the **settlement core only** — not speculation, not an
order book, not an oracle. It is the piece that holds user funds and guarantees
solvency, which is the part that most needs the auditability and determinism of
a smart contract.

## What's here

| Contract | Crate | Role |
|----------|-------|------|
| `OutcomeToken` | `contracts/token` | SEP-41 fungible token (OpenZeppelin). Reused for `tEURC` collateral and the `YES` / `NO` shares. |
| `Market` | `contracts/market` | The outcome-market settlement contract: `mint`, `merge`, `set_outcome`, `redeem`. |

- **`tEURC`** is a testnet placeholder for EURC (the real EURC stablecoin is not
  used on testnet). On the collateral token the deployer acts as a faucet.
- **`YES` / `NO`** are real SEP-41 fungible tokens whose supply can *only* be
  minted/burned by the `Market` contract — the market is wired in as their
  `minter` at deploy time.

### Token logic is not hand-rolled

The token uses the audited OpenZeppelin Stellar `fungible` module
(`Base` + `FungibleBurnable`) via `#[contractimpl(contracttrait)]`. The only
custom code on top is a thin `minter` access gate so the market is the sole
entity allowed to call `mint` (and, through SEP-41 `burn`, to destroy shares).

## Lifecycle & invariant

```
mint(to, amount)        lock `amount` tEURC  -> mint `amount` YES AND `amount` NO
merge(from, amount)     burn `amount` YES AND `amount` NO -> return `amount` tEURC
set_outcome(winner)     admin-only, once-only: resolve to Yes or No   (STUB)
redeem(holder, amount)  after resolution: burn `amount` of the WINNING
                        share -> pay out `amount` tEURC
```

Core solvency invariant, enforced by construction while the market is
**Unresolved**:

```
collateral_balance(market) == YES.total_supply == NO.total_supply
```

Every test asserts this after each operation (see `contracts/market/src/test.rs`).

### Safety rules (all covered by tests)

- `mint` / `merge` revert after resolution.
- `redeem` reverts before resolution.
- `set_outcome` is admin-only and can only be called once; `Unresolved` is
  rejected as a winner.
- All value-moving entrypoints require the relevant party's `require_auth()`.
- Non-positive amounts revert; you cannot merge/redeem more than you hold
  (enforced by the underlying token burn).

## `set_outcome` is a stub

`set_outcome` is an **admin switch** standing in for a future resolution
mechanism (oracle, dispute window, multisig committee, …). Oracle integration,
an AMM/order book, KYC, real EURC, and mainnet are explicitly out of scope for
this PoC.

## Build & test

```bash
# one-time
rustup target add wasm32v1-none

# run the test suite (14 tests: happy paths, invariants, revert cases)
cargo test

# build both contracts to wasm (uses the OZ spec-shaking feature)
stellar contract build
```

> If you build the wasm with plain `cargo build --target wasm32v1-none --release`
> instead of `stellar contract build`, set
> `SOROBAN_SDK_BUILD_SYSTEM_SUPPORTS_SPEC_SHAKING_V2=1` (the env var the CLI
> sets), because the OpenZeppelin library enables soroban-sdk's
> `experimental_spec_shaking_v2` feature.

## Deploy to testnet

```bash
./scripts/deploy.sh
```

The script funds ephemeral identities via Friendbot, deploys `tEURC` / `YES` /
`NO` and the `Market`, wires the market as the YES/NO minter, then runs a full
`mint -> merge` and `mint -> set_outcome(Yes) -> redeem` round-trip on-chain and
prints the contract IDs plus stellar.expert links.

### Deployed contract IDs (testnet)

Live deployment exercising a full `mint -> merge` and
`mint -> set_outcome(Yes) -> redeem` round-trip:

| Contract | ID | Explorer |
|----------|----|----------|
| tEURC | `CB5ZSBJCF26FLXQY4PAD4J63GX35MXI5SLEPJH33AP42FXMJLDWM3HFN` | https://stellar.expert/explorer/testnet/contract/CB5ZSBJCF26FLXQY4PAD4J63GX35MXI5SLEPJH33AP42FXMJLDWM3HFN |
| YES | `CAIGUZMH5XPGW44LVCTYNF35AXRWC2RUEUZ4BS5VUE4YQ6VOUTAJHLLW` | https://stellar.expert/explorer/testnet/contract/CAIGUZMH5XPGW44LVCTYNF35AXRWC2RUEUZ4BS5VUE4YQ6VOUTAJHLLW |
| NO | `CCFYUG47AWEH3674TP2LMULEE5Y657VANX2PDIO2OZLJBHQOLGX3XR66` | https://stellar.expert/explorer/testnet/contract/CCFYUG47AWEH3674TP2LMULEE5Y657VANX2PDIO2OZLJBHQOLGX3XR66 |
| Market | `CATXXTZBQCCRZNIZTRCA5NG7FQUJ4KHKMZ3VXBTSOVHG25P4QHWIUCZS` | https://stellar.expert/explorer/testnet/contract/CATXXTZBQCCRZNIZTRCA5NG7FQUJ4KHKMZ3VXBTSOVHG25P4QHWIUCZS |

> Re-running `deploy.sh` produces fresh contract IDs; update the table above
> accordingly.

## How this maps to FENET

FENET is a Kalshi-style prediction-market platform (Go microservices + Elixir +
Vue + Postgres). Today, matched positions and collateral are tracked off-chain
in the order/ledger services. This PoC moves the **settlement and custody**
substep on-chain:

- A FENET market with a binary outcome maps to one `Market` instance plus its
  `YES`/`NO` token pair.
- A user buying a matched, fully-collateralised position corresponds to
  `mint` (acquire a complete set) followed by trading one leg; closing out maps
  to `merge`.
- Market resolution in FENET's backend maps to `set_outcome`; user payouts map
  to `redeem`.
- The on-chain invariant gives a cryptographic solvency guarantee that the
  platform can never owe more than it custodies — the property regulators and
  users care about most.

The off-chain matching engine, pricing, and UX stay where they are; only the
trust-critical money movement is anchored on Stellar.

## SCF alignment (Tranche 1)

This repository is scoped as the **first concrete deliverable** of an SCF
build: a working, tested, open-source settlement contract on testnet with a
reproducible deploy. It establishes the on-chain foundation (SEP-41 outcome
tokens + collateral custody + 1:1 redemption) that later tranches extend with
oracle-driven resolution, an AMM/order interface, and an EURC-backed mainnet
path. It is intentionally framed as *settlement infrastructure, not
speculation*.

## Versions (pinned)

- `soroban-sdk = 25.3.1`
- `stellar-tokens / stellar-access / stellar-macros = 0.7.1` (OpenZeppelin)

OpenZeppelin 0.7.1 requires soroban-sdk `^25.3.0`, so the SDK is pinned to the
25.x line.

## License

Apache-2.0. See [LICENSE](./LICENSE).
