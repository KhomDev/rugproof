# Exercises

Every Rugproof guide has a graded **Exercises** section in the guide itself, with hints and answers in collapsible blocks. Each guide also has a **runnable Cargo workspace** in this folder: practice functions with a test suite, plus a **capstone** audit exercise. All you need is [Rust](https://rustup.rs) (stable). Nothing here needs a validator, a wallet or a network connection, and nothing is ever deployed.

| Guide | Workspace | Exercises (tests) | Capstone: spot the bug, then fix it (tests) |
| --- | --- | --- | --- |
| 00 | [00-rust-fundamentals](./00-rust-fundamentals/README.md) | Lifetimes, overflow, errors, decoding, Borsh, discriminators, share math, `RefCell` (16) | **Points ledger**: wrapping arithmetic, truncating cast (8) |
| 01 | [01-solana-foundations](./01-solana-foundations/README.md) | Rent, ownership, signers, the runtime's account rules, a native counter program (19) | **Treasury**: missing signer check, missing owner check (8) |
| 02 | [02-anchor-deep-dive](./02-anchor-deep-dive/README.md) | Discriminators, `InitSpace`, `Account<T>` loading, PDAs and canonical bumps, `has_one`, a staking pool (19) | **Airdrop distributor**: non-canonical PDA bump, missing `has_one` (8) |
| 03 | [03-solana-security-deep-dive](./03-solana-security-deep-dive/README.md) | Owner and duplicate checks, SPL Token transfer rules, arbitrary CPI, fake sysvars, re-init and revival, the audit checklist as code (17) | **Share vault**: arbitrary CPI, substituted token account (7) |
| 04 | [04-advanced-security-and-exploits](./04-advanced-security-and-exploits/README.md) | TWAP and oracle checks, AMM math and slippage, vault inflation, flash-loan introspection, borrow limits, liquidation (17) | **Lending market**: spot-price oracle manipulation, limit ignores existing debt (8 shared + 3 solution unit tests) |

## How a workspace is laid out

```text
0N-guide-name/
├── starter/            your work: todo!() stubs           cargo test -p starter
├── solutions/          reference answers                  cargo test -p solutions
├── capstone/           a deliberately flawed program      cargo test -p capstone
├── capstone-solution/  the same program, patched          cargo test -p capstone-solution
└── shared/
    ├── exercises.rs    one test suite, run against starter AND solutions
    ├── capstone.rs     one test suite, run against capstone AND capstone-solution
    └── model.rs        the small pure-Rust model of accounts, tokens, prices...
```

```bash
cd exercises/01-solana-foundations
cargo test -p starter l1_        # one level at a time
cargo test -p capstone exploit_  # just the attacker's moves
```

**Exercises.** The same unit, property-based, adversarial and integration tests run against `starter` and `solutions`. When `starter` is green, your answers hold up against the same attacks as the reference ones.

**Capstones.** Each capstone is a small toy program with two planted bugs from a well-documented class. Its test suite has two kinds of test:

- `honest_*`: normal use. These pass on both the flawed and the patched crate.
- `exploit_*`: the attacker's moves, run locally against the toy. Each one fails with `EXPLOIT SUCCEEDED: ...` on the flawed crate and passes once the bug is fixed.

Your job: read `capstone/src/lib.rs`, write down the missing sentence for each bug, patch the file until `cargo test -p capstone` is green, then compare with `capstone-solution/`. There, each `// FIX` block is one missing check, and `// WAS:` shows the line it replaced. The workspace README has two levels of hints.

CI proves each capstone does what it claims. The patched crate passes everything, the flawed crate passes every honest test, and **every single `exploit_*` test fails on the flawed crate** ([`scripts/check_capstone.sh`](./scripts/check_capstone.sh)).

## What the model is and isn't

Guides 01–04 teach real `solana_program` / Anchor / LiteSVM code. These workspaces teach the *checks* with a few dozen lines of pure-Rust model instead: `AccountInfo`, `TokenAccount`, a toy PDA curve, an AMM. That way they run anywhere in seconds and can't touch a real network. Every checked rule (runtime account rules, rent numbers, Anchor discriminators and error order, Token transfer rules) follows the real one. The model's limits are spelled out in each workspace README. When a guide asks you to build the real program, do that too.

Do not deploy deliberately broken examples. Only test against programs you wrote, or against public educational repos such as [sealevel-attacks](https://github.com/coral-xyz/sealevel-attacks).

## Maintainers

`starter/` (guides 01–04) and `capstone/` (all guides) are generated from `solutions/` and `capstone-solution/`. After editing a reference crate, run:

```bash
python3 exercises/scripts/sync_generated.py   # CI runs it with --check
```
