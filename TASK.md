# TASK: FENET binary outcome-market contract (Soroban testnet PoC)

## Context

FENET is building cash-settled binary contracts on European macro/policy events, settled on Stellar in EURC. This task is a **testnet proof-of-concept** of the core settlement mechanic only: minting and merging fully-collateralized YES/NO outcome-token pairs, plus stubs for resolution and redemption. It is the artifact we attach to a Stellar Community Fund (SCF) Build proposal to show the integration is already underway. It is **not** production code and does **not** include the oracle, the AMM, or mainnet EURC.

The defining invariant of the whole product: **one complete set (1 YES + 1 NO) is always backed by exactly 1 unit of collateral.** Everything below exists to make that verifiable on-chain.

## Objective

Build, test, and deploy to **Stellar testnet** a Soroban smart contract (Rust) that implements `mint`, `merge`, `set_outcome` (stub), and `redeem`, using YES and NO tokens that are real SEP-41 fungible tokens and a test collateral token standing in for EURC. Deliver a public repo with passing tests, deployed testnet contract IDs, and a README mapping the work to FENET's architecture and SCF tranches.

## Tech stack (use, do not reinvent)

- **Rust + Soroban SDK** (`soroban-sdk`).
- **OpenZeppelin Stellar Contracts library** for all token logic — crates `stellar-tokens` (fungible: Base + mintable + burnable + AllowList extension), `stellar-access` (Ownable), `stellar-macros`. Do not hand-roll SEP-41 token internals.
  - The library is in active development; **check the current stable version and exact trait/API names** at `https://github.com/OpenZeppelin/stellar-contracts` and `https://docs.openzeppelin.com/stellar-contracts`, and pin the version in `Cargo.toml`. Do not assume APIs from memory.
  - You may scaffold the YES/NO and collateral tokens with the OZ Contract Wizard (`https://wizard.openzeppelin.com/stellar`) and adapt.
- **Stellar CLI** for build (`wasm32` target) and testnet deploy; **Friendbot** to fund testnet accounts.
- Test collateral ("tEURC"): a test SEP-41 token (either a SAC-wrapped classic testnet asset or an OZ fungible token). The market must interact with it only through the standard token interface so it is swappable for the real EURC SAC on mainnet. **Label it clearly as a testnet placeholder everywhere.**

## Scope

### In scope
- One **market** contract with: `__constructor`, `mint`, `merge`, `set_outcome` (admin stub), `redeem`.
- YES and NO as real SEP-41 fungible tokens (OZ), minted/burned exclusively by the market contract (market contract is their owner/minter).
- A deployed test collateral token.
- Unit tests, deployment scripts, README.

### Out of scope (do NOT build)
- Any oracle / data ingestion / real resolution logic — `set_outcome` is an **admin-only stub**.
- The AMM / pricing / liquidity pools.
- Real EURC, mainnet deployment, KYC, or a frontend.
- The AllowList/whitelist may be included on the outcome tokens if cheap, but is **optional** for this PoC; do not block delivery on it.

## Contract design

### Roles & state
- `admin: Address` — can call `set_outcome` (stands in for the future resolver).
- `collateral: Address` — the test collateral token contract.
- `yes: Address`, `no: Address` — the two outcome-token contracts (market contract is their minter/owner).
- `outcome: enum { Unresolved, Yes, No }` — starts `Unresolved`.

### Functions (behavior is normative; signatures may be adapted to current SDK/OZ idioms)

**`__constructor(env, admin, collateral, yes, no)`**
Store the addresses and set `outcome = Unresolved`. Assumes the market contract has been (or will be) granted minter/owner rights on `yes` and `no`.

**`mint(env, to: Address, amount: i128)`**
- `to.require_auth()`. Require `amount > 0` and `outcome == Unresolved`.
- Pull `amount` of `collateral` from `to` into the market contract (token transfer; relies on `to`'s auth).
- Mint `amount` of `yes` to `to` and `amount` of `no` to `to`.
- Postcondition: contract collateral balance increased by `amount`; `yes.total_supply == no.total_supply`.

**`merge(env, from: Address, amount: i128)`**
- `from.require_auth()`. Require `amount > 0` and `outcome == Unresolved`.
- Destroy a complete set: burn `amount` of `yes` and `amount` of `no` held by `from`. Use a clean OZ-supported mechanism (e.g. `from` transfers the tokens to the market contract which burns them, or `burn_from` with prior allowance). Document the chosen mechanism.
- Transfer `amount` of `collateral` from the market contract back to `from`.
- Postcondition: supplies and contract collateral balance each decrease by `amount`; the 1:1 backing invariant still holds.

**`set_outcome(env, winner: <Yes|No>)`** — STUB
- `admin.require_auth()`. Require `outcome == Unresolved`. Set `outcome = winner`.
- This is a placeholder for the future optimistic resolver. No oracle, no dispute window — that is deliberately out of scope. Add a short doc comment saying so.

**`redeem(env, holder: Address, amount: i128)`**
- `holder.require_auth()`. Require `outcome != Unresolved` and `amount > 0`.
- Burn `amount` of the **winning** token from `holder`; transfer `amount` of `collateral` to `holder`.
- The losing token is not redeemable (worth 0). Optionally expose a no-op/explicit revert path for clarity.

### Invariants to hold and to test
1. While `Unresolved`: `contract_collateral_balance == yes.total_supply == no.total_supply`.
2. `mint` and `merge` revert once `outcome != Unresolved`.
3. After resolution, each unit of collateral in the contract is claimable by exactly one unit of the winning token; total winning redemptions cannot exceed collateral held.
4. Only `admin` can `set_outcome`; it can be set only once.
5. All value-moving entry points require the relevant party's `require_auth()`.

## Tests (Soroban test env)
- Happy paths: `mint` then `merge` round-trip returns exactly the collateral; `mint` → `set_outcome(Yes)` → `redeem` pays 1:1; symmetric for `No`.
- Invariant checks after each operation (balances and supplies).
- Failure cases (must revert): `merge`/`redeem` for more than held; `redeem` before resolution; `mint`/`merge` after resolution; `set_outcome` by non-admin; `set_outcome` called twice; zero/negative amounts.

## Deployment
- Build to `wasm32` with the Stellar CLI; deploy to **testnet**.
- Provide a reproducible script (or `Makefile`/`justfile`) that: deploys the collateral token, the two outcome tokens, and the market contract; wires ownership/minter rights; runs a sample `mint → merge` and a `mint → set_outcome → redeem` round-trip via CLI.
- Record all deployed contract IDs and link them on `stellar.expert` (testnet).

## Deliverables
1. Public GitHub repo, permissively licensed (open-source is an SCF expectation), with clear module layout.
2. The market contract + the three token deployments, building cleanly.
3. Passing test suite covering the cases above.
4. Deployment script + recorded testnet contract IDs + stellar.expert links.
5. `README.md` that: explains the PoC, maps each function to FENET's architecture (outcome-token layer of the settlement design) and to SCF tranche 1, states explicitly that the collateral token is a **testnet placeholder for EURC** and that the oracle and AMM are out of scope here, and lists the explorer links.

## Acceptance criteria
- `cargo test` passes; contracts build to wasm.
- Contracts are deployed to testnet and verifiable via the provided explorer links.
- Token logic is provided by the pinned OpenZeppelin library, not custom SEP-41 code.
- README and scope notes are accurate and do not overclaim (no implication of real EURC, oracle, AMM, or mainnet).

## References
- OpenZeppelin Stellar Contracts: https://github.com/OpenZeppelin/stellar-contracts , https://docs.openzeppelin.com/stellar-contracts
- OZ Contract Wizard (Stellar): https://wizard.openzeppelin.com/stellar
- Stellar smart-contract docs / CLI / testnet: https://developers.stellar.org/docs/build/smart-contracts
- Fungible token example: https://developers.stellar.org/docs/build/smart-contracts/example-contracts/fungible-token
