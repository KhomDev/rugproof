# Guide 01 workspace: Solana Foundations

Runnable practice for [01 · Solana Foundations](../../01-solana-foundations.md#exercises). It needs only [Rust](https://rustup.rs) (stable): no Solana CLI, no validator, no network.

The crates use a small pure-Rust model of the account model ([`starter/src/model.rs`](./starter/src/model.rs)): `Pubkey`, `AccountInfo` (key, owner, lamports, data, signer, writable, executable) and a subset of `ProgramError`. The checks you write are the same ones a real `solana_program` processor needs. The model just leaves out the parts that would need a validator. When you are ready for the real thing, the guide's native vault, Anchor port and LiteSVM exercises take it from there.

```bash
cd exercises/01-solana-foundations
cargo test -p starter            # your work: fails until you replace the todo!()s
cargo test -p starter l1_        # one level at a time
cargo test -p solutions          # reference answers: all green
cargo test -p capstone           # the capstone: exploit_* tests fail until you fix the bugs
cargo test -p capstone-solution  # the patched capstone: all green
```

The functions you edit are in `starter/src/lib.rs`. The full test suite is in [`starter/tests/exercises.rs`](./starter/tests/exercises.rs) and is also run against `solutions/`. `cargo test` from inside `starter/` or `solutions/` works too.

## Exercises (`starter/`)

| # | Function | You prove that… |
| --- | --- | --- |
| 1.1 | `rent_exempt_minimum` | you can compute rent exemption exactly (checked against mainnet values: 890,880 lamports for 0 bytes, 2,039,280 for a token account) |
| 1.2 | `can_modify` | only the owner program may debit or write an account, only if it is writable, and never an executable one |
| 1.3 | `require_signer` | a signer check is a one-liner you never forget |
| 2.1 | `next_account` | running out of accounts is an error, not a panic |
| 2.2 | `decode_counter_ix` | instruction data with any wrong length is rejected |
| 2.3 | `check_transition` | you know the runtime's rules: lamports conserved, foreign accounts only credited, read-only accounts untouched, owner changes only on zeroed data |
| 3.1 | `load_counter` | state is trusted only after the owner, length and discriminator checks |
| 3.2 | `process_instruction` | a whole native counter program (initialize / increment / close) validates everything before it writes, and a closed counter cannot be revived |

The suite in [`starter/tests/exercises.rs`](./starter/tests/exercises.rs) has 19 tests: unit tests, adversarial cases (attacker-owned look-alike accounts, unsigned authorities, close-to-self, overflow) and property tests. The strongest is an integration property: whatever accounts and bytes an attacker sends, your program never panics, never breaks a runtime rule from 2.3, and a failed instruction changes nothing.

## Capstone: the treasury (`capstone/`)

A tiny native program keeps a treasury and a state account that says who may withdraw from it. It compiles, the honest tests pass, and it can be drained. **There are two bugs.** They are the two most common missing checks in Solana programs.

1. Read [`capstone/src/lib.rs`](./capstone/src/lib.rs) like an auditor. For every account, ask who owns it, who signed it, and which line proves each.
2. Write the missing sentence for each bug.
3. Fix the file until `cargo test -p capstone` is green.
4. Compare with [`capstone-solution/src/lib.rs`](./capstone-solution/src/lib.rs). Each `// FIX` block there is one missing check.

<details>
<summary>Hint 1</summary>

Run `cargo test -p capstone exploit_` and read the two test names. One attacker never signs anything. The other brings an account she created herself.
</details>

<details>
<summary>Hint 2 (names the bug classes)</summary>

Missing signer check (exercise 1.3) and missing owner check (exercise 3.1).
</details>
