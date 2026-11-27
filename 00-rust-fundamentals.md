# Rust Fundamentals for Solana Developers

> **Rugproof** · Start of the series · [Series overview](./README.md) · [01 · Solana Foundations](./01-solana-foundations.md) →

## How to use this guide

Solana programs are written in a narrow but demanding slice of Rust. You don't need all of Rust to start, but the parts you do need (borrowing, lifetimes, traits, error handling, integer behaviour and byte layouts) show up in every program and in almost every bug. This guide teaches exactly those parts, each with a small example and a note on where it appears on Solana.

Every section ends with a pointer to a graded exercise. The exercises are a real Cargo workspace in [`exercises/00-rust-fundamentals`](./exercises/00-rust-fundamentals): fill in the `todo!()`s in `starter/`, and run the tests until they pass. Reference answers live in `solutions/` and in collapsible blocks below.

## Learning objectives

By the end of this guide you can:

- Explain ownership, borrowing and lifetimes well enough to read `AccountInfo<'info>` without guessing
- Model program state with structs and enums, and decode instruction bytes with `match` without ever panicking
- Write errors with `Result`, `?` and `thiserror`
- Use traits and generics to write one function that works for many account types
- Use iterators, modules, crates, Cargo and tests the way real Solana repos do
- Predict integer overflow, truncating casts and rounding, and know what `unsafe` gives up
- Predict the exact Borsh bytes of a struct

## Prerequisites

- Comfort with any programming language
- Rust installed with [`rustup`](https://rustup.rs) (stable toolchain)
- Optional: the first few chapters of [The Rust Book](https://doc.rust-lang.org/book/)

> **About the code.** Blocks marked **runnable** compile as a standalone `main.rs` or as part of the exercise crate. Other blocks are **snippets** that show one idea.

## Contents

1. [Cargo, crates and modules](#1-cargo-crates-and-modules)
2. [Ownership and borrowing](#2-ownership-and-borrowing)
3. [Lifetimes](#3-lifetimes)
4. [Structs, enums and `match`](#4-structs-enums-and-match)
5. [Error handling: `Option`, `Result`, `?` and `thiserror`](#5-error-handling-option-result--and-thiserror)
6. [Traits and generics](#6-traits-and-generics)
7. [Iterators and closures](#7-iterators-and-closures)
8. [Smart pointers: `Rc` and `RefCell`](#8-smart-pointers-rc-and-refcell)
9. [Integers: overflow, casts and rounding](#9-integers-overflow-casts-and-rounding)
10. [Serialization with Borsh](#10-serialization-with-borsh)
11. [Testing: unit, property and adversarial](#11-testing-unit-property-and-adversarial)
12. [`unsafe` and `no_std`: recognition only](#12-unsafe-and-no_std-recognition-only)
13. [Exercises](#13-exercises)

---

## 1. Cargo, crates and modules

A **crate** is one compilation unit: a library (`lib.rs`) or a binary (`main.rs`). A **package** is a `Cargo.toml` with one or more crates. A **workspace** groups packages that share a `Cargo.lock` and a `target/` directory. Almost every Solana repo is a workspace: `programs/*` for on-chain code, plus clients, tests and SDK crates.

```bash
cargo new --lib my_vault     # library crate
cd my_vault
cargo add borsh --features derive
cargo add thiserror
cargo build                  # debug build
cargo test                   # build and run tests
cargo clippy                 # lints (install with: rustup component add clippy)
cargo fmt                    # formatting
```

Modules split a crate into namespaces. Items are **private by default**; `pub` exports them.

```rust
// snippet: src/lib.rs
pub mod state;          // loads src/state.rs
mod processor;          // private: only this crate can use it

pub use state::Vault;   // re-export so users write `my_vault::Vault`
```

**On Solana:** an Anchor program is a crate with `crate-type = ["cdylib", "lib"]`. `cdylib` produces the `.so` file the validator loads; `lib` lets tests and other crates import its types. `Cargo.lock` tells you the exact versions in use, which is the first thing an auditor checks.

**Features** are compile-time switches. Many Solana crates hide code behind features (`no-entrypoint`, `cpi`, `idl-build`). When code "doesn't exist", check which features are on.

## 2. Ownership and borrowing

Rust's central rule: **every value has exactly one owner, and you may have either many shared references (`&T`) or one mutable reference (`&mut T`) at a time, never both.** The compiler checks this before your program runs.

```rust
// runnable
fn main() {
    let name = String::from("vault");   // `name` owns the String
    let moved = name;                   // ownership moves; `name` is now unusable
    // println!("{name}");              // compile error: value moved

    let mut balance: u64 = 100;
    let r1 = &balance;                  // shared borrow
    let r2 = &balance;                  // another shared borrow: fine
    println!("{r1} {r2} {moved}");
    let m = &mut balance;               // mutable borrow: fine, r1/r2 no longer used
    *m -= 10;                           // `*` reaches through the reference
    println!("{balance}");
}
```

Small, fixed-size types (`u64`, `bool`, `[u8; 32]`) are `Copy`: assigning them copies the bits instead of moving. `String`, `Vec<T>` and most structs move unless you `.clone()` them.

**Function arguments follow the same rules.** `fn f(x: T)` takes ownership, `fn f(x: &T)` borrows read-only, `fn f(x: &mut T)` borrows mutably.

**On Solana:** your program never owns account data. The runtime owns it and lends it to you for the duration of one instruction, which is why accounts arrive as references and why you see `&mut` and `borrow_mut()` everywhere.

**Exercise 1.3** uses `&mut Account` twice in one function.

## 3. Lifetimes

A **lifetime** names how long a borrow is valid. Most of the time the compiler infers it. You must write one when a function returns a reference derived from its inputs, or when a struct stores a reference.

```rust
// runnable
// The returned &str lives no longer than the shorter of `a` and `b`.
fn longest<'a>(a: &'a str, b: &'a str) -> &'a str {
    if b.len() > a.len() { b } else { a }
}

// A struct that borrows: it cannot outlive the data it points to.
struct View<'a> {
    data: &'a [u8],
}

fn main() {
    let bytes = vec![1, 2, 3];
    let v = View { data: &bytes };
    println!("{} {}", longest("pool", "vault"), v.data.len());
}
```

`'static` means "valid for the whole program", e.g. string literals and constants.

**On Solana:** every `AccountInfo<'info>` borrows from the input buffer the runtime hands your program. The `'info` lifetime says "this account view lives as long as the instruction's input", and Anchor's `Context<'info, T>` threads it through. When you see a wall of `'info`, read it as "borrowed from the transaction input".

**Exercise 1.1** asks you to write `longest`.

## 4. Structs, enums and `match`

Structs group named fields. Enums are "one of these variants", and each variant can carry data. `match` must handle every variant, which is what makes enums good for instruction dispatch.

```rust
// runnable
#[derive(Debug)]
enum Instruction {
    Initialize,
    Deposit { amount: u64 },
    Withdraw { amount: u64 },
}

fn describe(ix: &Instruction) -> String {
    match ix {
        Instruction::Initialize => "init".to_string(),
        Instruction::Deposit { amount } => format!("deposit {amount}"),
        Instruction::Withdraw { amount } if *amount == 0 => "no-op withdraw".to_string(),
        Instruction::Withdraw { amount } => format!("withdraw {amount}"),
    }
}

fn main() {
    println!("{}", describe(&Instruction::Deposit { amount: 5 }));
}
```

`Option<T>` (`Some(v)` / `None`) and `Result<T, E>` (`Ok(v)` / `Err(e)`) are ordinary enums. `if let` and `let … else` are shorthand for a one-arm `match`:

```rust
// snippet
let Some((&tag, rest)) = data.split_first() else {
    return Err(VaultError::InvalidInstruction);
};
```

**On Solana:** a native program's `process_instruction` receives raw `&[u8]` instruction data and must decode it. Decoding untrusted bytes is attack surface: any input that makes your decoder panic, or that two variants both accept, is a bug.

**Exercise 2.1** decodes instruction bytes and must never panic.

## 5. Error handling: `Option`, `Result`, `?` and `thiserror`

Rust has no exceptions. Fallible functions return `Result<T, E>`, and the `?` operator returns early with the error:

```rust
// snippet
fn read_u64(data: &[u8], offset: usize) -> Result<u64, VaultError> {
    let end = offset.checked_add(8).ok_or(VaultError::InvalidInstruction)?;
    let bytes: [u8; 8] = data
        .get(offset..end)                        // None if out of range (no panic)
        .ok_or(VaultError::InvalidInstruction)?
        .try_into()
        .map_err(|_| VaultError::InvalidInstruction)?;
    Ok(u64::from_le_bytes(bytes))
}
```

`unwrap()` and `expect()` turn errors into panics. In on-chain code a panic aborts the transaction with a generic error, so prefer returning a specific error.

The [`thiserror`](https://docs.rs/thiserror) crate derives `std::error::Error` and `Display` for your error enum:

```rust
// snippet (used in the exercise crate)
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum VaultError {
    #[error("insufficient funds: have {have}, need {need}")]
    InsufficientFunds { have: u64, need: u64 },
    #[error("arithmetic overflow")]
    Overflow,
    #[error("invalid instruction data")]
    InvalidInstruction,
}
```

**On Solana:** native programs convert custom errors to `ProgramError::Custom(code)`. Anchor's `#[error_code]` does the same and adds a numeric offset (6000 and up for your errors). Error *codes* are part of your public API: reordering variants changes them.

**Exercise 1.3** returns a `thiserror` error and must leave state unchanged on failure.

## 6. Traits and generics

A **trait** is a set of methods (and constants) a type promises to provide. **Generics** let one function work for any type that keeps the promise. Trait **bounds** (`T: Trait`) state the promise.

```rust
// runnable
trait Discriminator {
    const DISCRIMINATOR: [u8; 8];
}

struct Vault;
impl Discriminator for Vault {
    const DISCRIMINATOR: [u8; 8] = *b"vault___";
}

// Works for any T that has a discriminator.
fn has_tag<T: Discriminator>(data: &[u8]) -> bool {
    data.len() >= 8 && data[..8] == T::DISCRIMINATOR
}

fn main() {
    println!("{}", has_tag::<Vault>(b"vault___rest"));
}
```

Traits you'll meet constantly: `Clone`, `Copy`, `Debug`, `PartialEq`, `Default`, `From`/`Into`, `TryFrom`, `AsRef`, and serialization traits like `BorshSerialize`/`BorshDeserialize`. `#[derive(...)]` generates implementations for you.

**On Solana:** Anchor is built from traits. `Accounts`, `AccountSerialize`, `AccountDeserialize`, `Owner` and `Discriminator` are all traits, and `Account<'info, T>` only accepts a `T` that implements them. That is how Anchor checks the owner and the 8-byte discriminator for you. Programs that hand-roll loading often forget the discriminator check, which leads to *type cosplay* (one account type passed where another was expected), covered in guide 04.

**Exercise 3.1** builds a generic `load::<T>()` that rejects the wrong account type.

## 7. Iterators and closures

Iterators are lazy pipelines over a collection. Closures (`|x| x + 1`) are inline functions that can capture variables.

```rust
// runnable
fn main() {
    let balances = [5u64, 0, 9, 5, 1];
    let total: u64 = balances.iter().sum();
    let non_zero = balances.iter().filter(|b| **b > 0).count();
    let doubled: Vec<u64> = balances.iter().map(|b| b * 2).collect();
    // Checked sum: stops at the first overflow and returns None.
    let safe_total = balances.iter().try_fold(0u64, |acc, b| acc.checked_add(*b));
    println!("{total} {non_zero} {doubled:?} {safe_total:?}");
}
```

Know these: `iter`, `iter_mut`, `into_iter`, `map`, `filter`, `enumerate`, `zip`, `take`, `skip`, `fold`, `try_fold`, `any`, `all`, `find`, `collect`.

**On Solana:** `remaining_accounts` and account lists are iterated constantly. Watch for loops that are unbounded in user-controlled data (compute exhaustion) and for `sum()` on `u64` with overflow checks off (silent wrap in release builds unless `overflow-checks = true`).

**Exercises 1.2 and 2.2** use `try_fold`, `filter` and `sort_by_key`.

## 8. Smart pointers: `Rc` and `RefCell`

`Rc<T>` lets several owners share one value (single-threaded, reference counted). `RefCell<T>` allows mutation through a shared reference, moving the borrow check **from compile time to runtime**: `borrow()` and `borrow_mut()` panic if the rules are broken, and `try_borrow_mut()` returns an error instead.

```rust
// runnable
use std::cell::RefCell;
use std::rc::Rc;

fn main() {
    let lamports = Rc::new(RefCell::new(100u64));
    let alias = Rc::clone(&lamports);            // second owner of the same cell

    *alias.borrow_mut() -= 30;
    println!("{}", lamports.borrow());           // 70

    let _held = lamports.borrow_mut();
    assert!(alias.try_borrow_mut().is_err());    // second mutable borrow: error, not UB
}
```

**On Solana:** `AccountInfo` stores `lamports: Rc<RefCell<&mut u64>>` and `data: Rc<RefCell<&mut [u8]>>`. The reason: a transaction can pass **the same account twice** in one instruction, and both `AccountInfo`s must point at the same bytes. `RefCell` is what stops two live mutable views of one account. Logic that assumes two accounts are distinct, without checking, is the *duplicate mutable accounts* bug (guide 04).

**Exercise 3.3** moves lamports between two `RefCell`s and must handle "both are the same cell".

## 9. Integers: overflow, casts and rounding

- **Overflow.** In debug builds, `u64::MAX + 1` panics. In release builds it **wraps** to 0 unless the profile sets `overflow-checks = true`. Solana and Anchor templates set it, but check every `Cargo.toml` you audit.
- **Checked / saturating / wrapping.** `checked_add` returns `Option`; `saturating_sub` clamps at 0; `wrapping_mul` wraps on purpose. On-chain math should almost always use `checked_*`.
- **Casts.** `as` never fails: `300u16 as u8 == 44`, `-1i64 as u64 == u64::MAX`, `u128 as u64` keeps the low 64 bits. Use `u64::try_from(x)` when a value might not fit.
- **Division rounds toward zero.** Decide who the rounding should favour. In vaults and lending, round **against the user** (shares minted down, assets paid out down, debt rounded up), or attackers can extract value by repeating tiny operations.
- **Multiply before divide, in a wider type.** `amount * total_shares / total_assets` can overflow `u64`; do it in `u128`, then convert back with `try_from`.

```rust
// runnable
fn main() {
    let x: u128 = (u64::MAX as u128) + 5;
    println!("{}", x as u64);                // 4: silently truncated
    println!("{:?}", u64::try_from(x));      // Err(..): explicit failure
    println!("{:?}", 10u64.checked_sub(11)); // None
    println!("{}", 7u64 / 2);                // 3: rounds down
}
```

**Exercise 3.2** implements share math in `u128` and proves, with a property test, that a deposit followed by a withdrawal never returns more than went in.

## 10. Serialization with Borsh

[Borsh](https://borsh.io) (Binary Object Representation Serializer for Hashing) is the default serialization format for Solana programs and Anchor accounts. It is deterministic and has no field names or type tags:

| Type | Encoding |
| --- | --- |
| `u8`, `u16`, `u32`, `u64`, `u128` (and signed) | Little-endian, fixed size |
| `bool` | 1 byte, `0` or `1` |
| `[u8; N]` / `Pubkey` | `N` raw bytes (32 for a pubkey) |
| `Option<T>` | 1 byte tag (`0` = None, `1` = Some), then `T` if present |
| `Vec<T>`, `String` | `u32` little-endian length, then the elements |
| enum | 1 byte variant index, then the variant's fields |
| struct | Fields in declaration order, no padding |

```rust
// runnable with: cargo add borsh --features derive
use borsh::{BorshDeserialize, BorshSerialize};

#[derive(BorshSerialize, BorshDeserialize, Debug)]
struct Example { a: u8, b: u64, c: Vec<u16> }

fn main() {
    let e = Example { a: 1, b: 2, c: vec![3, 4] };
    let bytes = borsh::to_vec(&e).unwrap();
    // 1 (a) + 8 (b) + 4 (len) + 2*2 (c) = 17
    println!("{} {:?}", bytes.len(), bytes);
    let back = Example::try_from_slice(&bytes).unwrap();
    println!("{back:?}");
}
```

Two consequences matter for security:

1. **Any bytes of the right shape decode as any struct of that shape.** Borsh can't tell a `Vault { authority, balance }` from a `Config { admin, fee_bps }`. That is why Anchor prefixes every account with an 8-byte discriminator, and why you must check it when loading by hand.
2. **`try_from_slice` rejects trailing bytes**, but `deserialize(&mut &data[..])` does not. Know which one the code uses.

**Exercise 2.3** asks you to predict the Borsh length of an account. **Exercise 3.1** adds a discriminator.

## 11. Testing: unit, property and adversarial

```rust
// snippet
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transfer_moves_funds() {
        // arrange, act, assert
    }

    #[test]
    #[should_panic(expected = "overflow")]
    fn overflow_panics_in_debug() {
        let _ = u64::MAX + std::hint::black_box(1);
    }
}
```

- **Unit tests** live next to the code in a `#[cfg(test)]` module. **Integration tests** live in `tests/` and use only the public API.
- **Property tests** ([`proptest`](https://docs.rs/proptest)) generate thousands of random inputs and check an invariant: "the decoder never panics", "lamports are conserved", "a round trip never makes money".
- **Adversarial tests** are hand-written attacks: the wrong account type, the same account twice, `u64::MAX`, empty data, trailing bytes. In security work, the test that *fails before the fix and passes after* is the proof of a finding.

The exercise crate uses all three. Run `cargo test -p solutions` to see them pass.

**On Solana:** program tests run the compiled program in an in-process VM such as [LiteSVM](https://github.com/LiteSVM/litesvm) or [Mollusk](https://github.com/anza-xyz/mollusk). Guide 01 writes your first exploit test with LiteSVM.

## 12. `unsafe` and `no_std`: recognition only

`unsafe` blocks let code dereference raw pointers, call `unsafe fn`s and reinterpret memory. Inside them the compiler no longer proves memory safety, so you must. High-performance Solana frameworks (for example [Pinocchio](https://github.com/anza-xyz/pinocchio)) and zero-copy account types use `unsafe` to read the input buffer without copying.

You don't need to write `unsafe` yet. You need to recognize it and read it slowly, checking bounds, alignment and aliasing (two `&mut` to the same bytes) line by line.

`#![no_std]` crates don't link the standard library; on-chain programs often avoid allocation because the default heap is small (32 KiB). Expect fixed-size arrays, slices and manual parsing instead of `Vec` and `String`.

## 13. Exercises

Clone the repo, then:

```bash
cd exercises/00-rust-fundamentals
cargo test -p starter            # fails until you replace the todo!()s
cargo test -p starter l1_        # just level 1
cargo test -p solutions          # the reference answers, all green
```

The same test suite ([`shared/exercises.rs`](./exercises/00-rust-fundamentals/shared/exercises.rs)) runs against both crates, so when `starter` is green your answers behave like the reference ones, including the property and adversarial tests.

### Level 1: the basics

- [ ] **1.1 `longest`** (lifetimes, §3). Return the longer of two `&str`; on a tie return the first.
- [ ] **1.2 `total_lamports`** (iterators, overflow, §7, §9). Sum account balances; return `None` on overflow.
- [ ] **1.3 `transfer`** (borrowing, errors, §2, §5). Move lamports between two `&mut Account`. Return `InsufficientFunds { have, need }` or `Overflow`, and leave **both** accounts unchanged on any error.

<details>
<summary>Level 1 answers</summary>

```rust
pub fn longest<'a>(a: &'a str, b: &'a str) -> &'a str {
    if b.len() > a.len() { b } else { a }
}

pub fn total_lamports(accounts: &[Account]) -> Option<u64> {
    accounts.iter().try_fold(0u64, |acc, a| acc.checked_add(a.lamports))
}

pub fn transfer(from: &mut Account, to: &mut Account, amount: u64) -> Result<(), VaultError> {
    // Compute both new values first, then write: no partial updates on error.
    let new_from = from.lamports.checked_sub(amount)
        .ok_or(VaultError::InsufficientFunds { have: from.lamports, need: amount })?;
    let new_to = to.lamports.checked_add(amount).ok_or(VaultError::Overflow)?;
    from.lamports = new_from;
    to.lamports = new_to;
    Ok(())
}
```

Why can't the caller pass the same account as `from` and `to`? Two `&mut` to one value is a compile error. On Solana the same situation *is* possible, through `RefCell` (exercise 3.3).

</details>

### Level 2: data in, data out

- [ ] **2.1 `decode_instruction`** (enums, `match`, §4). Decode `[tag, payload…]`: `0` = `Initialize` with no payload; `1` = `Deposit`, `2` = `Withdraw`, each with exactly 8 bytes of little-endian `u64`. Anything else is `InvalidInstruction`. A property test feeds it random bytes: it must never panic.
- [ ] **2.2 `top_holders`** (iterators, §7). Return the owners of the `n` richest accounts, richest first, skipping zero balances; ties keep input order.
- [ ] **2.3 `borsh_len`** (Borsh, §10). Return the serialized length of an `Account`. Predict the number before you run the test.

<details>
<summary>Level 2 answers</summary>

```rust
pub fn decode_instruction(data: &[u8]) -> Result<VaultInstruction, VaultError> {
    let (&tag, rest) = data.split_first().ok_or(VaultError::InvalidInstruction)?;
    let amount = || -> Result<u64, VaultError> {
        let bytes: [u8; 8] = rest.try_into().map_err(|_| VaultError::InvalidInstruction)?;
        Ok(u64::from_le_bytes(bytes))
    };
    match tag {
        0 if rest.is_empty() => Ok(VaultInstruction::Initialize),
        1 => Ok(VaultInstruction::Deposit { amount: amount()? }),
        2 => Ok(VaultInstruction::Withdraw { amount: amount()? }),
        _ => Err(VaultError::InvalidInstruction),
    }
}

pub fn top_holders(accounts: &[Account], n: usize) -> Vec<Pubkey> {
    let mut v: Vec<&Account> = accounts.iter().filter(|a| a.lamports > 0).collect();
    v.sort_by_key(|a| std::cmp::Reverse(a.lamports)); // stable: ties keep input order
    v.into_iter().take(n).map(|a| a.owner).collect()
}

pub fn borsh_len(account: &Account) -> usize {
    borsh::to_vec(account).expect("Vec writer cannot fail").len() // 32 + 8 = 40
}
```

</details>

### Level 3: the security-shaped ones

- [ ] **3.1 `store` / `load`** (traits, generics, Borsh, §6, §10). `store` writes `T::DISCRIMINATOR` then the Borsh bytes. `load::<T>` rejects data whose first 8 bytes aren't `T::DISCRIMINATOR` (`WrongAccountType`) and data with trailing bytes. The tests try to load a `Config` as a `Vault`: both have the same Borsh shape, so only the discriminator can stop it.
- [ ] **3.2 `deposit_shares` / `withdraw_assets`** (integers, §9). Vault share math in `u128`, rounding **down** both ways; first depositor gets 1:1; `None` on overflow, division by zero or burning more shares than exist. A property test checks that a deposit followed by a full withdrawal never returns more than was deposited.
- [ ] **3.3 `move_lamports`** (`RefCell`, §8). Move lamports between two `&RefCell<u64>`. If both arguments are the **same cell**, return `DuplicateAccount` instead of panicking; if another borrow is held, return an error instead of panicking; never leave a partial update.

<details>
<summary>Level 3 answers</summary>

```rust
pub fn store<T: Discriminator + BorshSerialize>(value: &T) -> Vec<u8> {
    let mut out = T::DISCRIMINATOR.to_vec();
    value.serialize(&mut out).expect("Vec writer cannot fail");
    out
}

pub fn load<T: Discriminator + BorshDeserialize>(data: &[u8]) -> Result<T, VaultError> {
    if data.len() < 8 || data[..8] != T::DISCRIMINATOR {
        return Err(VaultError::WrongAccountType);
    }
    T::try_from_slice(&data[8..]).map_err(|_| VaultError::InvalidInstruction) // rejects trailing bytes
}

pub fn deposit_shares(amount: u64, total_assets: u64, total_shares: u64) -> Option<u64> {
    if total_shares == 0 { return Some(amount); }
    if total_assets == 0 { return None; }
    let s = (amount as u128).checked_mul(total_shares as u128)? / total_assets as u128;
    u64::try_from(s).ok()
}

pub fn withdraw_assets(shares: u64, total_assets: u64, total_shares: u64) -> Option<u64> {
    if total_shares == 0 || shares > total_shares { return None; }
    let a = (shares as u128).checked_mul(total_assets as u128)? / total_shares as u128;
    u64::try_from(a).ok()
}

pub fn move_lamports(from: &RefCell<u64>, to: &RefCell<u64>, amount: u64) -> Result<(), VaultError> {
    if std::ptr::eq(from, to) { return Err(VaultError::DuplicateAccount); }
    let mut f = from.try_borrow_mut().map_err(|_| VaultError::DuplicateAccount)?;
    let mut t = to.try_borrow_mut().map_err(|_| VaultError::DuplicateAccount)?;
    let nf = f.checked_sub(amount).ok_or(VaultError::InsufficientFunds { have: *f, need: amount })?;
    let nt = t.checked_add(amount).ok_or(VaultError::Overflow)?;
    *f = nf;
    *t = nt;
    Ok(())
}
```

Note that `deposit_shares` with a first-deposit 1:1 rule is the shape behind the well-known *first-depositor inflation* attack on share vaults. Guide 04 explains it and the usual fixes (virtual shares, minimum initial deposit).

</details>

### Stretch

- [ ] Add a `Close` variant (tag `3`, no payload) to `decode_instruction` and extend the tests.
- [ ] Rewrite `load` with `deserialize(&mut &data[8..])` instead of `try_from_slice` and watch the trailing-bytes test fail. Explain why.
- [ ] Add `proptest` cases of your own for `top_holders` (output length ≤ `n`, output sorted).

## Self-check

1. Why does `AccountInfo` wrap its lamports in `Rc<RefCell<…>>` instead of handing out `&mut u64`?
2. What does `x as u64` do when `x: u128` is larger than `u64::MAX`? What should you use instead?
3. Two structs have identical field types. What stops one from being decoded as the other?
4. Your `Cargo.toml` has no `[profile.release]` section. What does `u64::MAX + 1` do in a release build?
5. What's the difference between `?` on a `Result` and `.unwrap()` for on-chain code?

<details>
<summary>Answers</summary>

1. The same account can be passed twice in one instruction; both `AccountInfo`s must share one buffer, so the "one mutable borrow" rule is enforced at runtime.
2. It keeps the low 64 bits and silently drops the rest. Use `u64::try_from(x)`.
3. Only a type tag such as an 8-byte discriminator, checked on load. Borsh itself carries no type information.
4. It wraps to `0`, because release builds disable overflow checks by default. Set `overflow-checks = true`.
5. `?` returns a specific, catchable error to the caller; `unwrap()` panics and aborts the transaction with a generic error.

</details>

## Further reading

- [The Rust Book](https://doc.rust-lang.org/book/): chapters 4 (ownership), 6 (enums), 9 (errors), 10 (generics, traits, lifetimes), 13 (iterators), 15 (smart pointers)
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/)
- [Borsh specification](https://borsh.io)
- [`thiserror`](https://docs.rs/thiserror) and [`proptest`](https://docs.rs/proptest) docs
- [The Rustonomicon](https://doc.rust-lang.org/nomicon/), for when you're ready to read `unsafe` seriously

---

**Next:** [01 · Solana Foundations: Rust and Solana Concepts in Code →](./01-solana-foundations.md)
