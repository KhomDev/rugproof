# Guide 02 workspace: Anchor Deep-Dive

Runnable practice for [02 · Anchor Deep-Dive](../../02-anchor-deep-dive.md#exercises). It needs only [Rust](https://rustup.rs) (stable): no Anchor CLI, no validator.

Each exercise writes out, in plain Rust, a check that an Anchor macro generates for you. You see exactly what `Account<'info, T>`, `#[derive(InitSpace)]`, `has_one`, `seeds` + `bump` and `Signer` do, and what happens when one is missing. The model is in [`starter/src/model.rs`](./starter/src/model.rs), and SHA-256 and PDA helpers are in [`starter/src/pda_base.rs`](./starter/src/pda_base.rs).

> **Toy curve.** Discriminators use real SHA-256: `sighash("global", "initialize")` gives Anchor's real bytes. PDA derivation uses the real hash input order and the real bump search, but the on-curve test is a stand-in that rejects about half of all hashes, like the real ed25519 check. So addresses differ from mainnet. The logic is the same.

```bash
cd exercises/02-anchor-deep-dive
cargo test -p starter            # your work
cargo test -p solutions          # reference answers
cargo test -p capstone           # exploit_* tests fail until you fix the bugs
cargo test -p capstone-solution
```

The functions you edit are in `starter/src/lib.rs`. The full test suite is in [`starter/tests/exercises.rs`](./starter/tests/exercises.rs) and is also run against `solutions/`. `cargo test` from inside `starter/` or `solutions/` works too.

## Exercises (`starter/`)

| # | Function | Anchor feature it re-implements |
| --- | --- | --- |
| 1.1 | `sighash` | instruction and account discriminators (`sha256("namespace:name")[..8]`) |
| 1.2 | `field_space`, `account_space` | `#[derive(InitSpace)]` / `space = ...` (checked against real Borsh output) |
| 2.1 | `load_account` | `Account<'info, T>`: owner, then discriminator, then deserialize, with Anchor's error for each |
| 2.2 | `create_program_address`, `find_program_address` | PDAs, the canonical bump, seed limits |
| 2.3 | `check_seeds` | `seeds = [...], bump`: canonical bump only |
| 3.1 | `initialize_config` | `init` + `seeds` + `payer`, a fee cap, no re-init |
| 3.2 | `set_fee` | `has_one = admin`, `Signer`, `bump = config.bump` |
| 3.3 | `distribute`, `pending`, `stake`, `unstake` | the staking pool from the guide's exercises 2–8, MasterChef-style reward accounting |

The suite in [`starter/tests/exercises.rs`](./starter/tests/exercises.rs) has 19 tests. One property test, `prop_pool_is_solvent`, runs random stake / unstake / distribute sequences and checks the pool never pays more rewards than it received. It caught a real 1-unit over-payment in the textbook "round everything down" version, which is why `reward_debt` rounds **up**. Read the doc comment on `pending`.

## Capstone: the airdrop distributor (`capstone/`)

Each wallet may claim a fixed airdrop once. A receipt PDA remembers the claim, and an admin can sweep what is left. The doc comments show the Accounts structs the author *meant* to write. The code is what actually runs. **There are two bugs.** One lets a single wallet drain the whole airdrop.

1. For each account, list the constraints the intended Accounts struct implies, then find the code that enforces each one.
2. Write the missing sentence for each bug.
3. Fix [`capstone/src/lib.rs`](./capstone/src/lib.rs) until `cargo test -p capstone` is green.
4. Compare with [`capstone-solution/src/lib.rs`](./capstone-solution/src/lib.rs).

<details>
<summary>Hint 1</summary>

How many different receipt addresses can one wallet produce if it chooses the `bump` argument itself? See exercise 2.3.
</details>

<details>
<summary>Hint 2 (names the bug classes)</summary>

A non-canonical PDA bump, and a missing `has_one`.
</details>
