# Guide 03 workspace: Solana Security Deep-Dive

Runnable practice for [03 · Solana Security Deep-Dive](../../03-solana-security-deep-dive.md#exercises). It needs only [Rust](https://rustup.rs) (stable).

The guide's runtime and SPL Token invariants are written here as code. The model ([`starter/src/model.rs`](./starter/src/model.rs)) adds SPL Token `Mint` / `TokenAccount` state and stand-in program and sysvar ids. These are toy byte values, not the real base58 keys.

```bash
cd exercises/03-solana-security-deep-dive
cargo test -p starter            # your work
cargo test -p solutions          # reference answers
cargo test -p capstone           # exploit_* tests fail until you fix the bugs
cargo test -p capstone-solution
```

The functions you edit are in `starter/src/lib.rs`. The full test suite is in [`starter/tests/exercises.rs`](./starter/tests/exercises.rs) and is also run against `solutions/`. `cargo test` from inside `starter/` or `solutions/` works too.

## Exercises (`starter/`)

| # | Function | Invariant |
| --- | --- | --- |
| 1.1 | `check_owner` | trust account data only from the expected owner program |
| 1.2 | `check_distinct` | duplicate mutable accounts are rejected |
| 1.3 | `fee_amount` | fees round in the protocol's favour, and the u128 math cannot overflow |
| 2.1 | `validate_token_account` | right mint, right owner, not frozen |
| 2.2 | `transfer_checked` | the Token program's own rules: mint and decimals, frozen, owner or delegate within allowance |
| 2.3 | `check_cpi_program` | no arbitrary CPI: the target is pinned and executable |
| 3.1 | `check_sysvar` | sysvars are checked by key, never by contents (the fake-sysvar class) |
| 3.2 | `initialize_vault`, `close_account` | one-time init, safe close, no revival |
| 3.3 | `audit` | the audit checklist as code: given how a handler uses each account, list every missing check |

The suite in [`starter/tests/exercises.rs`](./starter/tests/exercises.rs) has 17 tests. They include an integration property where random transfers between three accounts by random signers must conserve supply and keep a delegate inside its allowance.

## Capstone: the share vault (`capstone/`)

Users deposit tokens into a vault through a Token-program CPI and get 1:1 shares. The tests deploy a fake "token program" (it returns success and moves nothing) into the local program registry, just as an attacker can deploy anything on a real cluster. **There are two bugs.** Together they let Mallory turn 50 tokens into Alice's 1,000.

1. Fill in guide 03's audit table for `deposit`. Exercise 3.3 is the same checklist.
2. Write the missing sentence for each bug.
3. Fix [`capstone/src/lib.rs`](./capstone/src/lib.rs) until `cargo test -p capstone` is green.
4. Compare with [`capstone-solution/src/lib.rs`](./capstone-solution/src/lib.rs).

<details>
<summary>Hint 1</summary>

Two of `deposit`'s inputs are chosen by the caller and never compared with anything the vault stored.
</details>

<details>
<summary>Hint 2 (names the bug classes)</summary>

Arbitrary CPI, and an unvalidated destination token account (account substitution).
</details>
