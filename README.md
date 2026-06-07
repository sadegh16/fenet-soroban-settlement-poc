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
`mint -> set_outcome(Yes) -> redeem` round-trip. These instances were deployed
from the **`v0.1.0` GitHub release artifacts** (see [Source verification](#source-verification)),
so their on-chain Wasm hashes match the attested build:

| Contract | ID | Explorer |
|----------|----|----------|
| tEURC | `CDLN5SFRB7GQOA7OHRPL7PATZGB3LEHL73HIL2EUZJTHMYJHCPJU25UQ` | https://stellar.expert/explorer/testnet/contract/CDLN5SFRB7GQOA7OHRPL7PATZGB3LEHL73HIL2EUZJTHMYJHCPJU25UQ |
| YES | `CDKNWOKJM2G7MEASMDZFONIM5F3BXYT6ZGD5CVHBMG6T2H5QSBIT625P` | https://stellar.expert/explorer/testnet/contract/CDKNWOKJM2G7MEASMDZFONIM5F3BXYT6ZGD5CVHBMG6T2H5QSBIT625P |
| NO | `CCGVSAYCZNC2HXIV3TUA6YVXZSOAGAALVMOFRONCNJPEI63ZBZHHIJT2` | https://stellar.expert/explorer/testnet/contract/CCGVSAYCZNC2HXIV3TUA6YVXZSOAGAALVMOFRONCNJPEI63ZBZHHIJT2 |
| Market | `CCMGIGNUNAWD7XNK27XRSLUNPYQ2YJKZTUG7IADXKSADWNKU55RA3LG4` | https://stellar.expert/explorer/testnet/contract/CCMGIGNUNAWD7XNK27XRSLUNPYQ2YJKZTUG7IADXKSADWNKU55RA3LG4 |

On-chain Wasm hashes (sha256), matching the `v0.1.0` release artifacts:

- `outcome-token`: `e6aee2d983df84c1562df50c514498feae33fca8ad560ef18a34422804853e24`
- `market`: `34ca30ec1c1fd712b4f3613ae98f749f481f124219ea0af85c5de443c2c4309e`

> Re-running `deploy.sh` builds locally and produces fresh contract IDs whose
> hashes will differ from the release artifacts; for verifiable deployments,
> deploy the `.wasm` attached to the GitHub release instead.

## Source verification

This repo ships `.github/workflows/release.yml`. Pushing a `v*` git tag builds
both contracts with `stellar contract build --optimize` (pinned `stellar-cli`
26.1.0), then for each contract it:

1. publishes the optimized `.wasm` as a GitHub release asset,
2. produces a signed [build-provenance attestation](https://github.com/sadegh16/fenet-soroban-settlement-poc/attestations)
   (`actions/attest-build-provenance`), and
3. POSTs the build metadata to stellar.expert's
   [contract-validation](https://stellar.expert/explorer/public/contract/validation)
   match API.

> The workflow is a self-contained adaptation of
> [`stellar-expert/soroban-build-workflow`](https://github.com/stellar-expert/soroban-build-workflow).
> The upstream reusable workflow pins `stellar-cli` 25.1.0, which cannot build
> the OpenZeppelin-based token (it enables the soroban-sdk
> `experimental_spec_shaking_v2` feature, which requires `stellar-cli` >= 25.2.0).

The `v0.1.0` release artifacts and their sha256 hashes:

| Package | Release asset | sha256 |
|---------|---------------|--------|
| `outcome-token` | [`outcome-token_v0.1.0.wasm`](https://github.com/sadegh16/fenet-soroban-settlement-poc/releases/tag/v0.1.0_outcome-token_cli26.1.0) | `e6aee2d983df84c1562df50c514498feae33fca8ad560ef18a34422804853e24` |
| `market` | [`market_v0.1.0.wasm`](https://github.com/sadegh16/fenet-soroban-settlement-poc/releases/tag/v0.1.0_market_cli26.1.0) | `34ca30ec1c1fd712b4f3613ae98f749f481f124219ea0af85c5de443c2c4309e` |

The live contracts in the table above were **deployed from these exact release
artifacts**, so their on-chain Wasm hashes match the attestation. You can verify
independently:

```bash
# hash of the on-chain code, compared against the release asset
stellar contract info interface --network testnet \
  --id CCMGIGNUNAWD7XNK27XRSLUNPYQ2YJKZTUG7IADXKSADWNKU55RA3LG4
gh release download v0.1.0_market_cli26.1.0 --repo sadegh16/fenet-soroban-settlement-poc
shasum -a 256 market_v0.1.0.wasm   # -> 34ca30ec...309e
```



## Versions (pinned)

- `soroban-sdk = 25.3.1`
- `stellar-tokens / stellar-access / stellar-macros = 0.7.1` (OpenZeppelin)

OpenZeppelin 0.7.1 requires soroban-sdk `^25.3.0`, so the SDK is pinned to the
25.x line.

## License

Apache-2.0. See [LICENSE](./LICENSE).
