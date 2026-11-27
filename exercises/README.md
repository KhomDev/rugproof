# Exercises

Every Rugproof guide has a graded **Exercises** section. Hints and answers are in collapsible blocks in the guide. Work in order; later guides link back to earlier ones instead of teaching the same idea twice.

| Guide | Where the exercises live | What you will do |
| --- | --- | --- |
| 00 | [Rust Fundamentals — Exercises](../00-rust-fundamentals.md#exercises) | A Cargo workspace in this folder (starter/ vs solutions/), then spot-the-red-flag drills in the guide |
| 01 | [Solana Foundations — Exercises](../01-solana-foundations.md#exercises) | Account-model quizzes, a read-only look at official programs, a native vault, an Anchor port, LiteSVM proof-of-check tests |
| 02 | [Anchor Deep-Dive — Exercises](../02-anchor-deep-dive.md#exercises) | Read an incomplete Accounts struct, then build a staking pool instruction by instruction |
| 03 | [Security Deep-Dive — Exercises](../03-solana-security-deep-dive.md#exercises) | Runtime and Token quizzes, explorer/CLI verification, a hypothetical pool checklist, detection tests against sealevel-attacks |
| 04 | [Advanced Security — Exercises](../04-advanced-security-and-exploits.md#exercises) | Incident quiz, token checklist, proof-of-check work, and a capstone due-diligence report |

## Guide 00's Cargo workspace

You need [Rust](https://rustup.rs) (stable).

`ash
cd exercises/00-rust-fundamentals

cargo test -p starter        # your work: fails until you replace the todo!()s
cargo test -p starter l1_    # only Level 1
cargo test -p solutions      # reference answers: all green
`

The same unit, property-based and adversarial tests run against both crates.

Guides 01–04 are written in the guides themselves (some ask you to build a local program or use LiteSVM). They tell you what to install when you need it. Do not deploy deliberately broken examples. Only test against programs you wrote, or against public educational repos such as [sealevel-attacks](https://github.com/coral-xyz/sealevel-attacks).

