# Guide 00 workspace: Rust Fundamentals

Runnable practice for [00 · Rust Fundamentals](../../00-rust-fundamentals.md#14-exercises). It needs only [Rust](https://rustup.rs) (stable).

```bash
cd exercises/00-rust-fundamentals
cargo test -p starter            # your work: fails until you replace the todo!()s
cargo test -p starter l1_        # only Level 1
cargo test -p solutions          # reference answers: all green
cargo test -p capstone           # the capstone: exploit_* tests fail until you fix the bugs
cargo test -p capstone-solution  # the patched capstone: all green
```

The exercises (`starter/`, `solutions/`, [`shared/exercises.rs`](./shared/exercises.rs)) are described level by level in the guide's [section 14](../../00-rust-fundamentals.md#14-exercises). There are 16 tests: unit, property-based and adversarial.

## Capstone: the points ledger (`capstone/`)

No Solana yet, just Rust. A points ledger supports minting, transfers with a flat fee, and batch payouts. It compiles, the honest tests pass, and a user holding 10 points can print 18 quintillion. **There are two bugs**, both from guide 00's Rust security section (section 13).

1. Go through section 13 (overflow, truncating casts, rounding, panics) line by line against [`capstone/src/lib.rs`](./capstone/src/lib.rs).
2. Write the missing sentence for each bug.
3. Fix the file until `cargo test -p capstone` is green.
4. Compare with [`capstone-solution/src/lib.rs`](./capstone-solution/src/lib.rs). Each `// FIX` block replaces the `// WAS:` line under it.

<details>
<summary>Hint 1</summary>

`overflow-checks = true` is on in this workspace. Look for arithmetic that opts out of it, and for conversions that cannot fail even when they should.
</details>

<details>
<summary>Hint 2 (names the bug classes)</summary>

`wrapping_add` used on user-controlled amounts, and a truncating `as u64` cast from `u128`.
</details>
