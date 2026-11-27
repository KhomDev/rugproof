# Solana Foundations: Rust and Solana Concepts in Code

> **Rugproof** · ← [00 · Rust Fundamentals](./00-rust-fundamentals.md) · [Series overview](./README.md) · [02 · Anchor Deep-Dive](./02-anchor-deep-dive.md) →

## Learning objectives

By the end of this guide you can:

- Read the Rust idioms Solana code relies on (`'info` lifetimes, `Rc<RefCell<…>>`, traits, macros, byte layouts) without stopping
- Map every core Solana term (account, owner, signer, PDA, rent, CPI, sysvar) to the struct or call where it lives
- Write a small native program by hand and explain which attack each of its checks stops
- Port that program to Anchor and say what the macros generate
- Write your first exploit test (a proof of concept) in LiteSVM

## Prerequisites

- [00 · Rust Fundamentals](./00-rust-fundamentals.md), or equivalent: roughly the first half of [The Rust Book](https://doc.rust-lang.org/book/)
- A terminal, and an evening to install the toolchain (see *Setup* below)

> **About the code.** Blocks introduced as a *full program* or *Cargo.toml* are meant to build as written. Every other block is a **snippet**: it shows one idea and omits imports or surrounding code. Solana crates release often, so if an import path has moved in your version, check the crate docs on [docs.rs](https://docs.rs) (and please send a PR).

## How to use this guide

**The uncomfortable truth first:** Solana security material usually feels hard because it assumes two things most newcomers don't have yet: Rust past the basics (lifetimes, `RefCell`, traits, macros, byte layouts) and the habit of seeing Solana concepts as concrete structs and function calls. This guide builds both. It is not optional reading; skipping it means memorizing bug classes you can't recognize in real code. \[Likely\]

**Who this is for:** someone about a month into the Rust Book, or who has finished [guide 00](./00-rust-fundamentals.md). Every Rust idea here is shown through the Solana code where you will actually meet it, so you learn the language and the platform together.

**The path**

| Phase | What you do | Time | You're done when… |
| --- | --- | --- | --- |
| 1. Read this guide | Parts A–F, in order. Type every code block yourself; don't copy-paste | 2–3 weeks | You can explain every line of the native program in Part C without looking |
| 2. Exercises | The graded set in the Exercises section. Levels 1–3 required, level 4 stretch | 2–3 weeks | Each exercise compiles, its tests pass, and you checked your answer against the self-check |
| 3. Self-assessment | The self-assessment at the end of this guide | 1 sitting (3–4 h) | You score ≥ 70% on your own honest marking |
| 4. Move on | [02 · Anchor Deep-Dive](./02-anchor-deep-dive.md), then [03 · Security Deep-Dive](./03-solana-security-deep-dive.md), using the Bridge section as a map | | |

If a self-assessment section goes badly, go back to the matching part of this guide before moving on.

**Setup before you start** (one evening):

- Rust via `rustup`, and the Solana CLI (Agave) + Anchor via the official installer on [solana.com/docs/intro/installation](https://solana.com/docs/intro/installation). \[Likely; installer URL may change\]
- An editor with `rust-analyzer`. Hovering types is how you learn `'info` lifetimes.
- `cargo install cargo-expand` so you can see what Anchor macros generate (Part D).
- A scratch Cargo workspace for Part A examples that don't need Solana.

## Part A: The Rust you need for Solana

Solana programs use a narrow slice of Rust, but they use it hard: borrowed data with explicit lifetimes, shared mutable state through `Rc<RefCell<…>>`, traits for serialization, heavy macros, and raw byte layouts. Learn these ten topics and you can read any Solana codebase. Each topic: the concept, a minimal example, then where it shows up in Solana. \[Certain\]

### A.1 Ownership and borrowing: the one rule

Every value has one owner. You can lend it out either as **many shared references** (`&T`, read-only) **or one mutable reference** (`&mut T`), never both at once. The compiler enforces this at compile time.

```rust
let mut balance: u64 = 100;
let r1 = &balance;          // shared borrow
let r2 = &balance;          // another shared borrow: fine
println!("{r1} {r2}");
let m = &mut balance;       // mutable borrow: fine, r1/r2 are no longer used
*m -= 10;                   // `*` dereferences to reach the value
```

**In Solana:** your program never owns account data. The runtime owns it and *lends* it to you. That is why every account arrives as a reference with a lifetime (`&'a mut [u8]`), and why you will see `borrow()` and `borrow_mut()` everywhere.

### A.2 Lifetimes: what `'info` and `'a` mean

A lifetime is a name for "how long this borrow is valid". You write one when a struct holds a reference, so the compiler can prove the struct does not outlive the data.

```rust
struct Wrapper<'a> {
    name: &'a str,           // Wrapper cannot outlive the string it points to
}

fn longest<'a>(x: &'a str, y: &'a str) -> &'a str {
    if x.len() > y.len() { x } else { y }
}
```

**In Solana:** the input buffer the runtime gives your program lives for the whole instruction. Every `AccountInfo<'a>` (native) or `AccountInfo<'info>` (Anchor) borrows from that buffer. `'info` is just a lifetime name Anchor chose; it means "for the duration of this instruction". When the compiler complains about `'info`, it is almost always because you tried to store an account reference somewhere that lives longer than the instruction, or mixed two structs with different lifetimes.

```rust
pub struct AccountInfo<'a> {
    pub key: &'a Pubkey,
    pub lamports: Rc<RefCell<&'a mut u64>>,
    pub data: Rc<RefCell<&'a mut [u8]>>,
    pub owner: &'a Pubkey,
    pub rent_epoch: u64,
    pub is_signer: bool,
    pub is_writable: bool,
    pub executable: bool,
}
```

This struct (from `solana-account-info`) is the most important type in Solana. Read every field type until it makes sense; the next section explains `Rc<RefCell<…>>`. \[Certain\]

### A.3 `Rc` and `RefCell`: shared mutable state checked at runtime

The borrow rule from A.1 is checked at compile time. Sometimes you need several handles to the same data and still want to mutate it. Two tools:

- **`Rc<T>`** (reference counted): many owners of one value. `Rc::clone(&x)` makes another handle, not a copy of the data.
- **`RefCell<T>`**: moves the borrow check to *runtime*. `.borrow()` gives a shared borrow, `.borrow_mut()` a mutable one; breaking the rule **panics** (or returns `Err` with `try_borrow_mut`).

```rust
use std::{cell::RefCell, rc::Rc};

let data = Rc::new(RefCell::new(vec![0u8; 8]));
let a = Rc::clone(&data);
let b = Rc::clone(&data);

a.borrow_mut()[0] = 7;          // fine: borrow ends at the semicolon
println!("{}", b.borrow()[0]);  // prints 7: same data

let _guard = a.borrow_mut();
// b.borrow_mut();              // would PANIC: already mutably borrowed
```

**In Solana:** `lamports` and `data` are `Rc<RefCell<&mut …>>` because the same account can appear twice in an instruction (duplicate accounts), and both `AccountInfo`s must point to the same bytes. That's the runtime basis of the duplicate-mutable-accounts bug in the deep-dive. Also, if you hold `account.data.borrow_mut()` and then call a CPI that needs the same account, you get an `AccountBorrowFailed` error: drop your borrow first (end its scope, or `drop(guard)`).

```rust
// reading and writing lamports in a native program
let amount = **from.lamports.borrow();          // two derefs: RefCell guard → &mut u64 → u64
**from.try_borrow_mut_lamports()? -= 10;
**to.try_borrow_mut_lamports()? += 10;
```

### A.4 Structs, enums, `match`, and `Option`/`Result`

Solana instructions are usually an `enum`; dispatch is a `match`.

```rust
enum VaultInstruction {
    Initialize,
    Deposit { amount: u64 },
    Withdraw { amount: u64 },
}

match instruction {
    VaultInstruction::Initialize => initialize(accounts),
    VaultInstruction::Deposit { amount } => deposit(accounts, amount),
    VaultInstruction::Withdraw { amount } => withdraw(accounts, amount),
}
```

`Option<T>` is `Some(value)` or `None`. `Result<T, E>` is `Ok(value)` or `Err(error)`. The SPL Token program even has its own `COption<Pubkey>` (a C-compatible Option) for `mint_authority`, because its byte layout must be fixed.

### A.5 Error handling: `?` and custom errors

`?` means: if this is `Err`, return that error from the current function now; otherwise unwrap the value.

```rust
fn parse(data: &[u8]) -> Result<u64, ProgramError> {
    let bytes: [u8; 8] = data
        .get(..8)                                  // Option<&[u8]>
        .ok_or(ProgramError::InvalidInstructionData)? // Option → Result, then `?`
        .try_into()
        .map_err(|_| ProgramError::InvalidInstructionData)?;
    Ok(u64::from_le_bytes(bytes))
}
```

**In Solana:** every handler returns `ProgramResult` (= `Result<(), ProgramError>`). Returning `Err` aborts the **whole transaction** and rolls back every account change. That is your main safety mechanism: check, and return an error if the check fails. Anchor adds `require!(cond, MyError::X)` and `#[error_code]` enums, which are just sugar over this.

**Never `unwrap()` on user input in a program.** A panic also aborts the transaction (so it's not a fund-loss bug by itself), but it hides intent and makes errors useless for debugging.

### A.6 Traits and generics

A **trait** is a promise that a type makes: "I have these functions." **Generics** let you write one function that works for any type that kept the promise. Solana code, and Anchor especially, is built on both. \[Certain\]

**Step 1: the problem.** Say you have two types and want each to describe itself.

```rust
struct Dog { name: String }
struct Car { model: String }
```

You could write `describe_dog()` and `describe_car()`, but every new type needs its own function, and nothing can work with "anything describable".

**Step 2: a trait is the promise.**

```rust
trait Describe {
    fn describe(&self) -> String;   // only the signature: no body
}
```

On its own this does nothing. It says: "a type that is `Describe` has a `describe` method returning a `String`."

**Step 3: types keep the promise with `impl`.**

```rust
impl Describe for Dog {
    fn describe(&self) -> String { format!("dog named {}", self.name) }
}
impl Describe for Car {
    fn describe(&self) -> String { format!("car model {}", self.model) }
}
```

Each type writes its own body. The compiler checks the signature matches the trait exactly.

**Step 4: generics, one function for every type that kept the promise.**

```rust
fn print_it<T: Describe>(item: &T) {
    println!("{}", item.describe());
}

print_it(&Dog { name: "Rex".into() });     // T = Dog
print_it(&Car { model: "Civic".into() });  // T = Car
```

Read `<T: Describe>` as: **"T is a placeholder for some type, and that type must implement `Describe`."** The `: Describe` part is a **trait bound**. It is the reason you may call `item.describe()`:

```rust
fn print_it<T>(item: &T) {
    println!("{}", item.describe());   // ERROR: no method `describe` found for `&T`
}
```

Without the bound, `T` could be any type (like `u64`, which has no `describe`), so the compiler refuses. At compile time Rust makes a separate copy of `print_it` for each type you actually use, so generics cost nothing at runtime.

**Step 5: a trait function with no `self`.** Some trait functions answer a question about the *type*, not a value:

```rust
trait Owner {
    fn owner() -> Pubkey;   // no &self: "which program owns accounts of this type?"
}
```

You call these on the type with `::`, not on a value with `.`:

```rust
Vault::owner()      // ✅ called on the type
my_vault.owner()    // ❌ not how this one works
T::owner()          // ✅ inside a generic: "the owner for whatever type T is"
```

**Step 6: the Solana example, line by line.**

```rust
trait Owner { fn owner() -> Pubkey; }                              // the promise

struct Vault { amount: u64 }
impl Owner for Vault { fn owner() -> Pubkey { crate::ID } }        // Vault keeps it

fn check_owner<T: Owner>(acc: &AccountInfo) -> Result<(), ProgramError> {
    if *acc.owner != T::owner() { return Err(ProgramError::IllegalOwner); }
    Ok(())
}

check_owner::<Vault>(vault_info)?;    // compares acc.owner with Vault::owner()
check_owner::<Config>(config_info)?;  // works for any type that implements Owner
```

You write the owner check once instead of once per account type. `::<Vault>` tells the compiler which type `T` is in this call.

**In Solana / Anchor:** this is exactly how `Account<'info, T>` works. `#[account] pub struct Vault {...}` makes the macro write `impl Owner for Vault`, `impl Discriminator for Vault` and the serialization traits for you. `Account<'info, Vault>` then runs a generic check: is the account's owner `Vault::owner()`, and do its first 8 bytes equal `Vault`'s discriminator? That's why it proves "owned by this program and really a `Vault`", and nothing about *which* vault. Other traits you'll meet constantly: `BorshSerialize`/`BorshDeserialize`, `Pack` (SPL Token), `Pod`/`Zeroable` (bytemuck), `Accounts`, `ToAccountInfos`, `Key`.

**Check yourself**

1. Add a `Config` struct and make `check_owner::<Config>` compile. What must you write?
2. Why can't you write `fn check_owner<T>(...)` without `: Owner`?
3. In `impl Owner for Vault`, what happens if `owner()` returns `system_program::ID` instead of `crate::ID`?

Answers: (1) `impl Owner for Config { fn owner() -> Pubkey { crate::ID } }`. (2) Without the bound, `T` might not have `owner()`, so `T::owner()` doesn't compile. (3) The check would accept accounts owned by the System Program (plain wallets) as vaults and reject real ones: a broken owner check.

### A.7 Macros: code that writes code

Two kinds:

- **Declarative** (`macro_rules!`, called with `!`): `msg!`, `require!`, `entrypoint!`, `declare_id!`.
- **Procedural**: attributes and derives that rewrite your code at compile time: `#[program]`, `#[derive(Accounts)]`, `#[account]`, `#[derive(BorshSerialize)]`.

The security consequence: **what you see is not what runs.** `#[derive(Accounts)]` generates the validation code. To audit it, run `cargo expand` and read the output. Part D does this.

### A.8 Integers: overflow, casts, and rounding

```rust
let a: u64 = u64::MAX;
let b = a.checked_add(1);         // None: overflow detected
let c = a.wrapping_add(1);        // 0: silently wraps
let d = a.saturating_add(1);      // u64::MAX: clamps
// let e = a + 1;                 // debug: panic. release: wraps UNLESS overflow-checks = true

let big: u128 = 1 << 70;
let t = big as u64;               // silently TRUNCATES. Dangerous.
let s = u64::try_from(big);       // Err: safe

let shares = 7u64 / 2;            // 3: integer division always rounds DOWN
let up = (7u64 + 2 - 1) / 2;      // 4: ceiling division
```

**In Solana:** every token amount is a `u64` in the smallest unit (`decimals` says where the point goes). Math is done in `u128` to avoid overflow during multiplication, then converted back with `try_from`. Check `Cargo.toml` for `overflow-checks = true` under `[profile.release]`. Rounding direction decides who loses the dust: the protocol should always round in its own favour.

### A.9 Bytes and layout: Borsh, `repr(C)`, zero-copy

One fact drives this whole topic: **account data is just a row of bytes.** Solana has no idea what's inside. Your program must turn bytes into a struct when reading, and the struct back into bytes when writing. \[Certain\]

**Step 1: how values become bytes.** Every type has a fixed size:

| Type | Bytes |
| --- | --- |
| `u8`, `bool` | 1 |
| `u16` | 2 |
| `u32` | 4 |
| `u64` | 8 |
| `u128` | 16 |
| `Pubkey` | 32 |

Numbers are stored **little-endian** (smallest byte first):

```rust
300u64.to_le_bytes()  // [44, 1, 0, 0, 0, 0, 0, 0]   because 44 + 1×256 = 300
```

**Step 2: Borsh puts fields in a row.** Borsh, the format most Solana programs use, writes each field in order, back to back, with no gaps:

```rust
#[derive(BorshSerialize, BorshDeserialize)]
pub struct Counter {
    pub authority: [u8; 32],
    pub count: u64,
}
```

```text
bytes 0..32   authority   (32 bytes)
bytes 32..40  count       (8 bytes, little-endian)
total: 40 bytes
```

No field names, no type name: only values. Two more rules: `Vec<T>` and `String` start with a **4-byte length** then the items (a `Vec<u16>` with 3 items = 4 + 3×2 = 10 bytes); `Option<T>` starts with **1 byte** (`0` = `None`, `1` = `Some`) followed by the value.

**Step 3: read, change, write back.**

```rust
// 1. READ: copy the bytes out of the account into a Rust struct
let mut c = Counter::try_from_slice(&account.data.borrow())?;

// 2. CHANGE: edit the copy (the account has NOT changed yet)
c.count += 1;

// 3. WRITE BACK: turn the struct into bytes, written into the account
c.serialize(&mut &mut account.data.borrow_mut()[..])?;
```

`c` is a **copy**. Forget step 3 and your change is silently lost. In Anchor, step 3 happens automatically for `mut` accounts, which is why changes without `mut` vanish (Part D).

About `&mut &mut`: `serialize` writes into anything writable; a `&mut [u8]` slice is writable and moves forward as bytes go in; to let it move, Borsh needs a mutable reference to the slice itself. Copy the pattern; you don't need more than that.

One trap: `try_from_slice` fails if bytes are **left over**. If the account is 50 bytes but `Counter` is 40, it errors. Size the account exactly, or use `deserialize`, which ignores extra bytes (Anchor does this).

**Step 4: zero-copy, reading in place.** Copying is fine for small accounts. For big ones (an order book of 50 KB), copying costs compute units and doesn't fit the program's 4 KB stack frames. Zero-copy treats the bytes **as the struct, right where they are**. The catch: the CPU wants numbers at aligned addresses (a `u64` at a multiple of 8), so Rust inserts invisible **padding**, and `#[repr(C)]` makes that layout predictable:

```rust
#[repr(C)]
struct Example { a: u8, b: u64 }
```

| Format | Layout | Size |
| --- | --- | --- |
| Borsh | `a` (1) + `b` (8) | 9 bytes |
| `repr(C)` in memory | `a` (1) + **padding (7)** + `b` (8) | 16 bytes |

Same struct, two layouts, so **never mix the formats**. Libraries like `bytemuck` refuse structs with hidden padding; you add explicit fields (`_pad: [u8; 7]`) so every byte is accounted for. Without `#[repr(C)]`, Rust may reorder fields. You'll see zero-copy as `#[account(zero_copy)]` + `AccountLoader` in Anchor, `bytemuck::from_bytes` in native code, and pointer casts in Pinocchio.

**Step 5: why the discriminator exists.** Bytes carry no type, so these two structs look identical in an account:

```rust
struct Counter { authority: Pubkey, count: u64 }  // 40 bytes
struct Config  { admin: Pubkey,     fee: u64 }    // 40 bytes
```

Read a `Config` as a `Counter` and Borsh succeeds: `admin` becomes `authority`, `fee` becomes `count`. An attacker who is `admin` of some `Config` can pass it where a `Counter` is expected and act as its `authority`. That's the **type cosplay** bug. The fix is a type tag at the start:

```text
bytes 0..8    discriminator   ← "this is a Counter"
bytes 8..40   authority
bytes 40..48  count
```

Anchor's `#[account]` writes `sha256("account:Counter")[..8]` there and `Account<'info, Counter>` rejects anything else; that's why Anchor space is always `8 + …`. In Part C you do it by hand with `VAULT_DISC`.

**Check yourself**

1. How many Borsh bytes is `struct S { a: u8, b: Vec<u64>, c: Pubkey }` when `b` holds 2 items?
2. You call `try_from_slice`, change a field, and return `Ok(())`. What's in the account afterwards?
3. Why would `try_from_slice` fail on a 48-byte Anchor account holding a 40-byte struct?
4. Two account types both start with a 32-byte pubkey. What stops one being passed as the other in Anchor? In Part C?

Answers: (1) 1 + (4 + 2×8) + 32 = 53. (2) The old value: you only changed the copy. (3) The 8-byte discriminator comes first, so the bytes don't line up; strip it first, as Anchor does. (4) Anchor: the discriminator check in `Account<T>`. Part C: the owner check plus `VAULT_DISC` in `load_vault`.

### A.10 `unsafe`, `no_std`, and Pinocchio (recognition only)

Programs run in a 4 KB-frame stack, 32 KB heap VM. High-performance frameworks (Pinocchio, p-token) avoid allocations and parse the input buffer with raw pointers inside `unsafe` blocks. You don't need to write `unsafe` yet; you need to recognize that **inside `unsafe`, the compiler stops protecting you**, so auditors read those blocks line by line (bounds, alignment, aliasing). \[Certain\]

### A.11 Self-check

You're ready for Part B when you can answer, without looking:

1. Why is `AccountInfo::data` an `Rc<RefCell<&mut [u8]>>` and not a `&mut [u8]`?
2. What happens at runtime if you call `borrow_mut()` twice on the same `RefCell` without dropping the first?
3. What does `?` do on an `Err`? On a `None` (inside a function returning `Option`)?
4. What does `x as u64` do when `x: u128` is larger than `u64::MAX`?
5. Why can't a Borsh-serialized struct tell you what type it is?

## Part B: Solana terms, as they look in code

Each term below gets a one-line meaning and the code you'll see it in. Client-side code is TypeScript or Rust using the SDK; on-chain code is the program. Imports use the `solana_program` re-exports, which still work; newer code imports the split crates (`solana_pubkey`, `solana_account_info`, `solana_instruction`, …) with the same names. \[Likely\]

### B.1 Quick reference

| Term | Meaning | You see it as |
| --- | --- | --- |
| Lamport | Smallest unit of SOL; 1 SOL = 1,000,000,000 lamports | `u64` fields named `lamports` |
| Pubkey / address | 32 bytes naming an account | `Pubkey`, `pubkey!("...")`, base58 strings |
| Keypair / signature | ed25519 private key; a 64-byte signature over the message | `Keypair`, `Signature`, client only |
| Account | Address → {lamports, data, owner, executable} | `AccountInfo` on-chain, `Account` off-chain |
| Program | An executable account holding sBPF bytecode | `program_id: &Pubkey`, `declare_id!` |
| Owner | Program allowed to change an account's data | `account.owner` |
| Instruction | One call: program ID + account list + data bytes | `Instruction { program_id, accounts, data }` |
| AccountMeta | One entry in that list: pubkey + is\_signer + is\_writable | `AccountMeta::new(pk, true)` |
| Transaction | Signed list of instructions, all-or-nothing | `Transaction`, `VersionedTransaction` |
| Signer | Account whose key signed the transaction (or a PDA signed via CPI) | `account.is_signer`, Anchor `Signer<'info>` |
| Writable | Account the tx may modify; takes a write lock | `account.is_writable`, Anchor `#[account(mut)]` |
| System Program | Creates accounts, transfers SOL, owns wallets | `system_program::ID`, `system_instruction::create_account` |
| Rent / rent-exempt | Minimum lamports for an account to exist | `Rent::get()?.minimum_balance(len)` |
| PDA | Address derived from seeds + program ID, no private key | `Pubkey::find_program_address(&[seeds], &program_id)` |
| Bump | The extra seed byte that pushes a PDA off the curve | `u8`, Anchor `ctx.bumps.vault` |
| CPI | One program calling another | `invoke`, `invoke_signed`, Anchor `CpiContext` |
| Sysvar | Special read-only accounts with cluster data | `Clock::get()?`, `Rent::get()?`, Instructions sysvar |
| Slot / epoch | \~400 ms block slot; epoch ≈ 2 days of slots | `Clock::get()?.slot`, `.unix_timestamp` |
| Blockhash | Recent hash a tx must include; expires after \~150 slots | client: `getLatestBlockhash` |
| Compute units (CU) | Gas-like execution budget; 200k per ix default, 1.4M per tx max | `ComputeBudgetInstruction::set_compute_unit_limit` |
| Entrypoint | The function the runtime calls in your program | `entrypoint!(process_instruction)` |
| Discriminator | Bytes at the start of data that say which type/instruction this is | Anchor: first 8 bytes of `sha256("account:Name")` |
| IDL | JSON describing a program's instructions and accounts | `target/idl/*.json` (Anchor) |
| Mint / token account / ATA | A token type / a balance of that token / the default balance address per wallet | `Mint`, `TokenAccount`, `get_associated_token_address` |
| Upgrade authority | Key that can replace a program's code | `solana program show <ID>` |

&#91;Certain, except CU numbers which are cluster parameters: Likely\]

### B.2 Accounts and the entrypoint

```rust
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint, entrypoint::ProgramResult,
    msg, pubkey::Pubkey,
};

entrypoint!(process_instruction);   // the runtime calls this function

pub fn process_instruction(
    program_id: &Pubkey,            // this program's address
    accounts: &[AccountInfo],       // every account the caller listed, in order
    instruction_data: &[u8],        // raw bytes the caller sent
) -> ProgramResult {
    let iter = &mut accounts.iter();
    let user = next_account_info(iter)?;     // accounts[0]
    let state = next_account_info(iter)?;    // accounts[1]

    msg!("user={} signer={} writable={}", user.key, user.is_signer, user.is_writable);
    msg!("state owner={} lamports={} len={}", state.owner, state.lamports(), state.data_len());
    Ok(())
}
```

The whole program interface is those three arguments. **Accounts come by position**, so the program decides that `accounts[1]` is "the state", and the caller can put anything there. That's why checks matter.

### B.3 Instructions and transactions (client side)

```rust
use solana_sdk::{instruction::{AccountMeta, Instruction}, transaction::Transaction};

let ix = Instruction {
    program_id: my_program::ID,
    accounts: vec![
        AccountMeta::new(user.pubkey(), true),      // writable + signer
        AccountMeta::new(state_pda, false),         // writable, not signer
        AccountMeta::new_readonly(system_program::ID, false),
    ],
    data: vec![1, 100, 0, 0, 0, 0, 0, 0, 0],        // tag 1 + u64 amount 100 (little-endian)
};
let tx = Transaction::new_signed_with_payer(&[ix], Some(&user.pubkey()), &[&user], blockhash);
```

The signer and writable flags are chosen by **the client**, not the program. A program must check them.

### B.4 Owner and signer checks (native)

```rust
if state.owner != program_id {
    return Err(ProgramError::IncorrectProgramId);   // someone else's account: data is untrusted
}
if !user.is_signer {
    return Err(ProgramError::MissingRequiredSignature);
}
if !state.is_writable {
    return Err(ProgramError::InvalidAccountData);
}
```

### B.5 PDAs and bumps

```rust
// derive (client or program): returns the canonical bump
let (vault, bump) = Pubkey::find_program_address(
    &[b"vault", user.key.as_ref()],   // seeds: a fixed prefix + the user's key
    program_id,
);

// verify (program): the passed account must be exactly this address
if vault_info.key != &vault {
    return Err(ProgramError::InvalidSeeds);
}

// cheaper re-derivation when you stored the bump at init:
let expected = Pubkey::create_program_address(
    &[b"vault", user.key.as_ref(), &[stored_bump]], program_id)?;
```

`find_program_address` loops from bump 255 down (costs CU); `create_program_address` takes your bump (cheap, but you must have stored the canonical one).

### B.6 Rent and creating an account

```rust
use solana_program::{program::invoke_signed, rent::Rent, system_instruction, sysvar::Sysvar};

let space = 8 + 32 + 8;                               // discriminator + authority + u64
let lamports = Rent::get()?.minimum_balance(space);

invoke_signed(
    &system_instruction::create_account(user.key, vault.key, lamports, space as u64, program_id),
    &[user.clone(), vault.clone(), system_program.clone()],
    &[&[b"vault", user.key.as_ref(), &[bump]]],      // the PDA "signs" for its own creation
)?;
```

After this call: the vault exists, holds `lamports`, has `space` zero bytes, and is owned by `program_id`. Only now can your program write its data.

### B.7 CPI

```rust
use solana_program::program::{invoke, invoke_signed};

// user pays: the user's signature flows through
invoke(
    &system_instruction::transfer(user.key, vault.key, amount),
    &[user.clone(), vault.clone(), system_program.clone()],
)?;

// program-owned token vault pays: the PDA authority signs via seeds
invoke_signed(
    &spl_token::instruction::transfer(token_program.key, vault_ata.key, user_ata.key,
                                      vault_authority.key, &[], amount)?,
    &[vault_ata.clone(), user_ata.clone(), vault_authority.clone(), token_program.clone()],
    &[&[b"authority", pool.key.as_ref(), &[auth_bump]]],
)?;
```

### B.8 Sysvars and compute

```rust
use solana_program::{clock::Clock, sysvar::Sysvar};
let now = Clock::get()?.unix_timestamp;   // syscall: no account needed
let slot = Clock::get()?.slot;
// compute budget is set by the CLIENT with a ComputeBudget instruction;
// on-chain you only see CU run out as an error.
```

### B.9 Discriminators and IDLs

In Anchor, an account's first 8 bytes are `sha256("account:<StructName>")[..8]`; an instruction's data starts with `sha256("global:<fn_name>")[..8]`. Anchor 1.x also allows custom discriminators. The IDL (`target/idl/<program>.json`) lists every instruction, its accounts (with `writable`/`signer` flags), args and account types: when auditing, start there. \[Certain\]

### B.10 Self-check

1. Who decides whether an account is writable in an instruction: the program or the client?
2. Why does creating a PDA account need `invoke_signed` and not `invoke`?
3. What is the difference between `account.owner` and the `owner` field inside an SPL token account?
4. Why is `find_program_address` more expensive than `create_program_address`?
5. Where in Anchor would you look to see every account an instruction takes, without reading the Rust?

## Part C: A native program, end to end

This is a per-user SOL vault written without Anchor: three instructions (Initialize, Deposit, Withdraw), every check written by hand. Writing it once is the fastest way to understand what Anchor does for you and, more importantly, what it doesn't. Type it out; don't paste. \[Certain for logic; check crate versions when you build\]

### C.1 Project setup

```toml
# Cargo.toml
[package]
name = "native_vault"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "lib"]   # cdylib = the .so the validator loads

[dependencies]
# Add these with `cargo add solana-program` and `cargo add borsh --features derive`
# so Cargo records the current versions. Don't leave a dependency as "*".
solana-program = "…"
borsh = { version = "1", features = ["derive"] }

[profile.release]
overflow-checks = true
```

Build with `cargo build-sbf`; the output is `target/deploy/native_vault.so`.

### C.2 The full program

```rust
use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint, entrypoint::ProgramResult,
    msg,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction, system_program,
    sysvar::Sysvar,
};

entrypoint!(process_instruction);

// ---------- data types ----------
#[derive(BorshSerialize, BorshDeserialize)]
pub enum VaultIx {
    Initialize,
    Deposit { amount: u64 },
    Withdraw { amount: u64 },
}

pub const VAULT_DISC: [u8; 8] = *b"VAULT_v1";
pub const VAULT_LEN: usize = 8 + 32 + 1;   // disc + authority + bump

#[derive(BorshSerialize, BorshDeserialize)]
pub struct Vault {
    pub discriminator: [u8; 8],
    pub authority: Pubkey,
    pub bump: u8,
}

// ---------- dispatch ----------
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let ix = VaultIx::try_from_slice(data)
        .map_err(|_| ProgramError::InvalidInstructionData)?;
    match ix {
        VaultIx::Initialize => initialize(program_id, accounts),
        VaultIx::Deposit { amount } => deposit(program_id, accounts, amount),
        VaultIx::Withdraw { amount } => withdraw(program_id, accounts, amount),
    }
}

// ---------- helpers ----------
fn load_vault(program_id: &Pubkey, vault: &AccountInfo) -> Result<Vault, ProgramError> {
    if vault.owner != program_id {                               // (1) owner check
        return Err(ProgramError::IllegalOwner);
    }
    let v = Vault::try_from_slice(&vault.data.borrow())
        .map_err(|_| ProgramError::InvalidAccountData)?;
    if v.discriminator != VAULT_DISC {                           // (2) type check
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(v)
}

// ---------- Initialize ----------
// accounts: [authority (signer, writable), vault (writable), system_program]
fn initialize(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let it = &mut accounts.iter();
    let authority = next_account_info(it)?;
    let vault = next_account_info(it)?;
    let system = next_account_info(it)?;

    if !authority.is_signer {                                    // (3) signer check
        return Err(ProgramError::MissingRequiredSignature);
    }
    if system.key != &system_program::ID {                       // (4) program ID check
        return Err(ProgramError::IncorrectProgramId);
    }
    let (expected, bump) =
        Pubkey::find_program_address(&[b"vault", authority.key.as_ref()], program_id);
    if vault.key != &expected {                                  // (5) PDA identity check
        return Err(ProgramError::InvalidSeeds);
    }

    let lamports = Rent::get()?.minimum_balance(VAULT_LEN);
    invoke_signed(
        &system_instruction::create_account(
            authority.key, vault.key, lamports, VAULT_LEN as u64, program_id),
        &[authority.clone(), vault.clone(), system.clone()],
        &[&[b"vault", authority.key.as_ref(), &[bump]]],
    )?;

    let v = Vault { discriminator: VAULT_DISC, authority: *authority.key, bump };
    v.serialize(&mut &mut vault.data.borrow_mut()[..])?;
    msg!("vault initialized for {}", authority.key);
    Ok(())
}

// ---------- Deposit ----------
// accounts: [depositor (signer, writable), vault (writable), system_program]
fn deposit(program_id: &Pubkey, accounts: &[AccountInfo], amount: u64) -> ProgramResult {
    let it = &mut accounts.iter();
    let depositor = next_account_info(it)?;
    let vault = next_account_info(it)?;
    let system = next_account_info(it)?;

    load_vault(program_id, vault)?;                              // must be a real vault
    if system.key != &system_program::ID {
        return Err(ProgramError::IncorrectProgramId);
    }
    invoke(
        &system_instruction::transfer(depositor.key, vault.key, amount),
        &[depositor.clone(), vault.clone(), system.clone()],
    )
}

// ---------- Withdraw ----------
// accounts: [authority (signer), vault (writable), recipient (writable)]
fn withdraw(program_id: &Pubkey, accounts: &[AccountInfo], amount: u64) -> ProgramResult {
    let it = &mut accounts.iter();
    let authority = next_account_info(it)?;
    let vault = next_account_info(it)?;
    let recipient = next_account_info(it)?;

    if !authority.is_signer {                                    // (3) signer
        return Err(ProgramError::MissingRequiredSignature);
    }
    let v = load_vault(program_id, vault)?;                      // (1)+(2)
    if v.authority != *authority.key {                           // (6) relationship check
        return Err(ProgramError::InvalidAccountData);
    }
    let expected = Pubkey::create_program_address(
        &[b"vault", authority.key.as_ref(), &[v.bump]], program_id)?;
    if vault.key != &expected {                                  // (5) PDA identity
        return Err(ProgramError::InvalidSeeds);
    }

    let min = Rent::get()?.minimum_balance(vault.data_len());
    let remaining = vault.lamports()
        .checked_sub(amount).ok_or(ProgramError::InsufficientFunds)?;
    if remaining < min {                                         // (7) stay rent-exempt
        return Err(ProgramError::InsufficientFunds);
    }

    // the vault holds data, so System `transfer` can't debit it: move lamports directly
    **vault.try_borrow_mut_lamports()? -= amount;
    **recipient.try_borrow_mut_lamports()? += amount;
    Ok(())
}
```

### C.3 Every check, and the attack it stops

| # | Check | Attack if missing | Deep-dive link |
| --- | --- | --- | --- |
| 1 | `vault.owner == program_id` | Attacker creates an account they own with bytes that look like a `Vault` with `authority = attacker` | Owner checks (6.1 #2) |
| 2 | Discriminator equals `VAULT_DISC` | Another account type of this program with the same length is accepted as a vault | Type cosplay (6.1 #3) |
| 3 | `authority.is_signer` | Anyone passes the victim's pubkey and withdraws | Signer authorization (6.1 #0) |
| 4 | `system.key == system_program::ID` | Attacker passes their own program as "system"; the CPI calls it | Arbitrary CPI (6.1 #5) |
| 5 | Vault key equals the derived PDA | A different user's vault, or any account, passes as "your" vault | PDA sharing (6.1 #8) |
| 6 | `v.authority == authority.key` | A signer withdraws from a vault that isn't theirs | Account data matching (6.1 #1) |
| 7 | Remaining ≥ rent minimum | Withdraw fails the whole tx (`InsufficientFundsForRent`) | Rent (1.4) |

### C.4 Questions the code leaves open (find the answers yourself)

1. Check 5 in `withdraw` is redundant given checks 1, 2 and 6. Why? Would you keep it anyway?
2. What happens if someone sends 1 lamport to the vault PDA address *before* `initialize` runs?
3. What happens if `recipient` and `vault` are the same account?
4. In `deposit`, nothing checks that `depositor` is a signer. Is that a bug? (Hint: who checks it inside the CPI?)
5. There is no `Close` instruction. Sketch one, and list three things it must do.

## Part D: The same program in Anchor

Anchor moves the account checks from your function body into a struct, and the struct becomes generated validation code. The vault below is functionally the same as Part C. Compare them side by side until you can map every native check to the Anchor line that replaces it. Syntax follows Anchor 1.x; on 0.30–0.32 it is the same except where noted. \[Likely: `CpiContext::new` argument types have shifted between versions; follow your compiler\]

### D.1 The full program

```rust
use anchor_lang::prelude::*;
use anchor_lang::system_program::{transfer, Transfer};

declare_id!("11111111111111111111111111111111");   // replaced by `anchor keys sync`

#[program]
pub mod anchor_vault {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let vault = &mut ctx.accounts.vault;
        vault.authority = ctx.accounts.authority.key();
        vault.bump = ctx.bumps.vault;            // canonical bump found during validation
        Ok(())
    }

    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        let cpi = CpiContext::new(
            System::id(),   // Anchor 0.x: ctx.accounts.system_program.to_account_info()
            Transfer {
                from: ctx.accounts.depositor.to_account_info(),
                to: ctx.accounts.vault.to_account_info(),
            },
        );
        transfer(cpi, amount)
    }

    pub fn withdraw(ctx: Context<Withdraw>, amount: u64) -> Result<()> {
        let vault = ctx.accounts.vault.to_account_info();
        let min = Rent::get()?.minimum_balance(vault.data_len());
        let remaining = vault.lamports().checked_sub(amount).ok_or(VaultError::Insufficient)?;
        require!(remaining >= min, VaultError::Insufficient);

        vault.sub_lamports(amount)?;
        ctx.accounts.recipient.to_account_info().add_lamports(amount)?;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        init,
        payer = authority,
        space = 8 + Vault::INIT_SPACE,
        seeds = [b"vault", authority.key().as_ref()],
        bump,
    )]
    pub vault: Account<'info, Vault>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut)]
    pub depositor: Signer<'info>,
    #[account(mut, seeds = [b"vault", vault.authority.as_ref()], bump = vault.bump)]
    pub vault: Account<'info, Vault>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    pub authority: Signer<'info>,
    #[account(
        mut,
        has_one = authority,
        seeds = [b"vault", authority.key().as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, Vault>,
    #[account(mut)]
    pub recipient: SystemAccount<'info>,
}

#[account]
#[derive(InitSpace)]
pub struct Vault {
    pub authority: Pubkey,
    pub bump: u8,
}

#[error_code]
pub enum VaultError {
    #[msg("insufficient funds")]
    Insufficient,
}
```

### D.2 Native check → Anchor line

| Native check (Part C) | Anchor equivalent | Where it runs |
| --- | --- | --- |
| (1) `vault.owner == program_id` | `Account<'info, Vault>` | Generated `try_accounts`, before your handler |
| (2) discriminator | `#[account]` on `Vault` writes and checks 8 bytes `sha256("account:Vault")[..8]` | Same |
| (3) `authority.is_signer` | `Signer<'info>` | Same |
| (4) system program ID | `Program<'info, System>` | Same |
| (5) PDA identity | `seeds = [...], bump` | Same |
| (6) `v.authority == authority.key` | `has_one = authority` | Same |
| (7) rent minimum | Your `require!` in the handler | Your code |
| `create_account` CPI + serialize | `init, payer, space` | Before handler; data written on exit |
| Instruction parsing | `#[program]` generates dispatch by 8-byte instruction discriminator + Borsh args | Generated entrypoint |

### D.3 What the macros generate

Run `cargo expand > expanded.rs` in the program folder and search for `impl<'info> anchor_lang::Accounts<'info, WithdrawBumps> for Withdraw<'info>`. Simplified, its `try_accounts` does this, in field order: \[Likely in exact shape; Certain in substance\]

```rust
// authority: Signer
let authority = Signer::try_from(&accounts[0])?;              // is_signer
// vault: Account<Vault>
let vault: Account<Vault> = Account::try_from(&accounts[1])?; // owner == ID, discriminator, Borsh decode
// recipient: SystemAccount
let recipient = SystemAccount::try_from(&accounts[2])?;       // owner == System Program

// constraints on vault
if !vault.to_account_info().is_writable { return Err(ConstraintMut) }
if vault.authority != authority.key()   { return Err(ConstraintHasOne) }
let pda = Pubkey::create_program_address(
    &[b"vault", authority.key().as_ref(), &[vault.bump]], &ID)
    .map_err(|_| ConstraintSeeds)?;
if vault.key() != pda                   { return Err(ConstraintSeeds) }
// constraints on recipient
if !recipient.to_account_info().is_writable { return Err(ConstraintMut) }
```

After your handler returns `Ok`, Anchor calls `exit()` on every `mut` `Account<T>`, which re-serializes the struct into the account data. That's why changes to an `Account<T>` without `mut` are silently lost.

### D.4 Things to notice

- In `Deposit`, the seeds use `vault.authority` (a field read **from the account being validated**). That's fine here only because `Account<Vault>` already proved owner and type before seeds run. If `vault` were an `UncheckedAccount`, reading a field to build its own seeds would prove nothing.
- `bump = vault.bump` uses the bump stored at init (cheap); `bump` alone would re-search (expensive but canonical by construction).
- `recipient: SystemAccount` means "owned by System Program". It does **not** mean "the authority's wallet". If you wanted that, you'd add `address = authority.key()`.
- Anchor errors appear in logs as `AnchorError caused by account: vault. Error Code: ConstraintHasOne` — learn to read these; tests in Part F assert on them.

### D.5 Self-check

1. Which constraint in `Withdraw` would you delete to create the signer-authorization bug? The account-data-matching bug? The PDA-sharing bug?
2. Why does `Initialize` need `system_program` but `Withdraw` doesn't?
3. What would change if `vault` in `Withdraw` were not `mut`?

## Part E: Tokens in code

Tokens are the reason most Solana programs exist, and the token program's account model trips up most beginners: a wallet does not hold tokens directly. Each (wallet, mint) pair needs its own **token account**, owned by the Token program, whose internal `owner` field names the wallet. Learn the byte layouts once; you'll recognize them in every audit. \[Certain\]

### E.1 The two account types, byte by byte

**Mint (82 bytes)**

| Bytes | Field | Type |
| --- | --- | --- |
| 0–35 | `mint_authority` | `COption<Pubkey>` (4-byte tag + 32) |
| 36–43 | `supply` | `u64` |
| 44 | `decimals` | `u8` |
| 45 | `is_initialized` | `bool` |
| 46–81 | `freeze_authority` | `COption<Pubkey>` |

**Token account (165 bytes)**

| Bytes | Field | Type |
| --- | --- | --- |
| 0–31 | `mint` | `Pubkey` |
| 32–63 | `owner` | `Pubkey` (the wallet or PDA that controls it) |
| 64–71 | `amount` | `u64` |
| 72–107 | `delegate` | `COption<Pubkey>` |
| 108 | `state` | `u8` (0 uninitialized, 1 initialized, 2 frozen) |
| 109–120 | `is_native` | `COption<u64>` (wrapped SOL rent reserve) |
| 121–128 | `delegated_amount` | `u64` |
| 129–164 | `close_authority` | `COption<Pubkey>` |

Token-2022 accounts start with the same bytes, then an account-type byte and TLV extensions. \[Certain\]

### E.2 Reading a token account (native)

```rust
use solana_program::program_pack::Pack;
use spl_token::state::Account as TokenAccount;

if token_info.owner != &spl_token::ID {                 // the ACCOUNT owner: the Token program
    return Err(ProgramError::IllegalOwner);
}
let ta = TokenAccount::unpack(&token_info.data.borrow())?;
if ta.mint != expected_mint { return Err(ProgramError::InvalidAccountData); }
if ta.owner != *user.key  { return Err(ProgramError::InvalidAccountData); }  // the TOKEN owner: the wallet
msg!("balance = {}", ta.amount);
```

Two different "owners" in five lines. Confusing them is a classic beginner and audit bug.

### E.3 Associated token accounts

```rust
use spl_associated_token_account::get_associated_token_address_with_program_id;

let ata = get_associated_token_address_with_program_id(&wallet, &mint, &spl_token::ID);
// = find_program_address(&[wallet, token_program_id, mint], &ATA_PROGRAM_ID).0
```

The ATA is just the default token account address for a (wallet, mint, token program). Wallets can also hold tokens in non-ATA accounts.

### E.4 A token vault in Anchor

Users deposit any SPL or Token-2022 token into a vault ATA controlled by a program PDA; the PDA signs to send tokens back.

```rust
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked},
};

#[derive(Accounts)]
pub struct DepositTokens<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(
        mut,
        token::mint = mint,
        token::authority = user,
        token::token_program = token_program,
    )]
    pub user_ata: InterfaceAccount<'info, TokenAccount>,
    /// CHECK: PDA that only signs as the vault's token authority
    #[account(seeds = [b"authority", mint.key().as_ref()], bump)]
    pub vault_authority: UncheckedAccount<'info>,
    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = mint,
        associated_token::authority = vault_authority,
        associated_token::token_program = token_program,
    )]
    pub vault_ata: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,   // Token OR Token-2022
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn deposit_tokens(ctx: Context<DepositTokens>, amount: u64) -> Result<()> {
    let before = ctx.accounts.vault_ata.amount;
    token_interface::transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.key(),   // Anchor 0.x: .to_account_info()
            TransferChecked {
                from: ctx.accounts.user_ata.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.vault_ata.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        amount,
        ctx.accounts.mint.decimals,
    )?;
    ctx.accounts.vault_ata.reload()?;                       // data changed during the CPI
    let received = ctx.accounts.vault_ata.amount
        .checked_sub(before).ok_or(ProgramError::ArithmeticOverflow)?;
    msg!("credited {}", received);                          // credit `received`, not `amount`
    Ok(())
}
```

Withdrawing: the vault authority PDA signs.

```rust
let mint_key = ctx.accounts.mint.key();
let seeds: &[&[u8]] = &[b"authority", mint_key.as_ref(), &[ctx.bumps.vault_authority]];

token_interface::transfer_checked(
    CpiContext::new_with_signer(
        ctx.accounts.token_program.key(),
        TransferChecked {
            from: ctx.accounts.vault_ata.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            to: ctx.accounts.user_ata.to_account_info(),
            authority: ctx.accounts.vault_authority.to_account_info(),
        },
        &[seeds],
    ),
    amount,
    ctx.accounts.mint.decimals,
)?;
```

### E.5 What to notice

- **`reload()` after the CPI.** `vault_ata` was deserialized before the transfer. Without `reload()`, `amount` is the old balance.
- **`received` vs `amount`.** With a Token-2022 transfer-fee mint the vault receives less than `amount`. Measuring the balance difference is the robust pattern.
- **`transfer_checked`, not `transfer`.** It passes the mint and decimals, and is required for Token-2022 extensions like transfer hooks.
- **The vault authority's seeds are `[b"authority", mint]`.** One authority per mint, shared by every user. That's fine only if the program tracks each user's share separately (otherwise any user could withdraw everything). This is the PDA-sharing design question from the deep-dive.
- **`init_if_needed` on the vault ATA** is safe-ish here because the ATA's address already pins mint, authority and token program. On your own state accounts it is a reinitialization risk.

### E.6 Self-check

1. A token account's `AccountInfo.owner` is X and its `TokenAccount.owner` field is Y. What are X and Y for a user's USDC ATA?
2. Why is `token::authority = user` important on `user_ata`, given the Token program already checks the user signed?
3. What goes wrong if you remove `reload()` and use `vault_ata.amount` after the transfer?

## Part F: Testing, and your first exploit test

An auditor's finding is only as good as its proof of concept. On Solana a PoC is a test that runs the real program in an embedded runtime and shows the attack succeeding. Learn one tool well. **LiteSVM** runs whole transactions in-process in milliseconds; **Mollusk** runs single instructions with full control of the accounts; `anchor test` runs a local validator with TypeScript tests (slow, but realistic). Start with LiteSVM. \[Likely: both tools' APIs drift between releases; follow the README for the version you install\]

| Tool | Level | Speed | Use it for |
| --- | --- | --- | --- |
| [LiteSVM](https://github.com/LiteSVM/litesvm) | Full transactions | ms | PoCs, multi-instruction attacks, CPI flows |
| [Mollusk](https://github.com/anza-xyz/mollusk) | One instruction | µs–ms | Forging exact account states, CU benchmarking |
| `anchor test` / `solana-test-validator` | Real validator | seconds | Integration tests, client code |
| Trident (fuzzing) | Many random txs | — | Invariant testing, later |

### F.1 The pattern: prove the check matters

1. Write a test that performs the attack and asserts it **fails** against the secure program.
2. Remove the check that should stop it, rebuild, run again: the test now **fails** because the attack succeeded.
3. Put the check back. You now have a regression test and a PoC.

### F.2 Example: the missing-signer attack on the Part C vault

Dev-dependencies: `litesvm`, `solana-sdk` (or the split `solana-keypair`, `solana-transaction`, … crates matching LiteSVM's version), `borsh`.

```rust
// tests/exploit.rs
use borsh::to_vec;
use litesvm::LiteSVM;
use native_vault::VaultIx;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_program,
    transaction::Transaction,
};

#[test]
fn withdraw_without_victim_signature_must_fail() {
    let program_id = Pubkey::new_unique();
    let mut svm = LiteSVM::new();
    svm.add_program_from_file(program_id, "target/deploy/native_vault.so").unwrap();

    let victim = Keypair::new();
    let attacker = Keypair::new();
    svm.airdrop(&victim.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&attacker.pubkey(), 1_000_000_000).unwrap();

    let (vault, _) =
        Pubkey::find_program_address(&[b"vault", victim.pubkey().as_ref()], &program_id);

    // 1. victim creates the vault and deposits 5 SOL (two instructions, one tx)
    let init = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(victim.pubkey(), true),
            AccountMeta::new(vault, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: to_vec(&VaultIx::Initialize).unwrap(),
    };
    let deposit = Instruction {
        program_id,
        accounts: init.accounts.clone(),
        data: to_vec(&VaultIx::Deposit { amount: 5_000_000_000 }).unwrap(),
    };
    let tx = Transaction::new_signed_with_payer(
        &[init, deposit], Some(&victim.pubkey()), &[&victim], svm.latest_blockhash());
    svm.send_transaction(tx).unwrap();

    // 2. attacker names the victim as `authority` but does NOT have their signature
    let steal = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(victim.pubkey(), false),   // is_signer = false
            AccountMeta::new(vault, false),
            AccountMeta::new(attacker.pubkey(), false),           // recipient
        ],
        data: to_vec(&VaultIx::Withdraw { amount: 5_000_000_000 }).unwrap(),
    };
    let tx = Transaction::new_signed_with_payer(
        &[steal], Some(&attacker.pubkey()), &[&attacker], svm.latest_blockhash());
    let before = svm.get_balance(&attacker.pubkey()).unwrap();
    let result = svm.send_transaction(tx);

    assert!(result.is_err(), "attack succeeded: signer check is missing");
    // only the tx fee was paid
    assert!(svm.get_balance(&attacker.pubkey()).unwrap() < before);
}
```

> **Version note.** This test was written against the LiteSVM and `solana-sdk` APIs current at the time of writing. Newer `solana-sdk` releases moved some modules into split crates (for example program IDs into `solana-sdk-ids` and the System Program interface into `solana-system-interface`); if an import fails, search the name on [docs.rs](https://docs.rs).

Now comment out check (3) in `withdraw`, run `cargo build-sbf && cargo test`, and watch it fail. Every other check passes for the attacker: the vault is real (1, 2), it is the victim's PDA (5), and its stored authority equals the key the attacker passed (6). Only the signature proves the victim agreed.

### F.3 Forging accounts

Many attacks need an account in a state you can't reach through normal instructions (a fake oracle, a fake config owned by the attacker). LiteSVM and Mollusk let you write any account directly:

```rust
use solana_sdk::account::Account;

let fake = Pubkey::new_unique();
let mut data = Vec::new();
data.extend_from_slice(b"VAULT_v1");                  // correct discriminator
data.extend_from_slice(attacker.pubkey().as_ref());   // authority = attacker
data.push(255);                                       // bump
svm.set_account(fake, Account {
    lamports: 10_000_000_000,
    data,
    owner: attacker_program_id,   // NOT our program: check (1) must reject it
    executable: false,
    rent_epoch: 0,
}).unwrap();
```

In an audit, if forging the account is the only way to reach the bug, ask whether an attacker could actually create that state on-chain: an attacker-owned program can create such an account, so owner checks really are needed.

### F.4 Reading failures

- Program logs: `svm.send_transaction(tx)` returns an error that includes `meta.logs`; print them. Anchor errors appear as `AnchorError ... Error Code: ConstraintSeeds`.
- `custom program error: 0x...` is your program's error enum index (Anchor custom errors start at 6000 = 0x1770).
- `Program failed to complete` / `Access violation` usually means a panic or out-of-bounds read inside the program.

### F.5 Self-check

1. In F.2, why does removing check (6) alone not let the attacker steal? (Connect it to question C.4.1.)
2. Write the forged-account test from F.3 as a full test that asserts withdraw fails. Which error do you expect?
3. What would a PoC need to show for the duplicate-account bug (C.4.3)?

## Exercises

Do them in order. Levels 1–3 are the core; level 4 is a stretch. Each exercise is done when it compiles, has at least one test, and you can explain why it works. Tick them off here as you go.

### Level 1: Rust (Part A)

- [ ] **1.1 RefCell panic.** Write a program that panics with `already mutably borrowed`. Then fix it two ways: with a scope block, and with `drop()`. Then rewrite it with `try_borrow_mut()` returning an error instead of panicking.
- [ ] **1.2 Parse bytes.** Write `fn read_u64_at(data: &[u8], offset: usize) -> Result<u64, String>` that never panics, for any input. Test with an empty slice, an offset past the end, and `usize::MAX`.
- [ ] **1.3 Share math.** Write `deposit_shares(amount, total_assets, total_shares) -> Option<u64>` and `withdraw_assets(shares, total_assets, total_shares) -> Option<u64>` using `u128` internally. Deposit rounds shares **down**, withdraw rounds assets **down**. Write a test showing a user can't get back more than they put in after 1,000 round trips.
- [ ] **1.4 Borsh layout.** Serialize a struct `{ a: u8, b: u64, c: Vec<u16> }` with Borsh and print the bytes. Predict the length before running.

### Level 2: Native programs (Parts B, C, F)

- [ ] **2.1 Build and test Part C.** Build the native vault and get the F.2 test passing. Then make it fail by removing check (3).
- [ ] **2.2 Add `Close`.** Add an instruction that sends all lamports to the authority and closes the vault. Requirements: authority signs, zero the data, assign the account back to the System Program, and the recipient must not be the vault itself. Write a test that a second `Withdraw` on the closed vault fails.
- [ ] **2.3 Fix the pre-funding DoS.** Write a test where an attacker sends 1 lamport to the victim's future vault PDA before `initialize`, and show `initialize` fails. Then fix `initialize` (transfer the shortfall, then `allocate` and `assign` with `invoke_signed`) and show the test passes.
- [ ] **2.4 Duplicate accounts.** Write a test passing the vault as both `vault` and `recipient` in `Withdraw`. Explain what happens to the lamports, and add a check if you think one is needed.

### Level 3: Anchor and tokens (Parts D, E)

- [ ] **3.1 Port.** Build the Part D Anchor vault and port your `Close` from 2.2 using `close = authority`. Run `cargo expand` and find the generated close code.
- [ ] **3.2 Break it on purpose.** For each of `has_one`, `seeds`, `Signer`, write a PoC test that succeeds once that one constraint is removed.
- [ ] **3.3 Token vault with accounting.** Extend the Part E token vault with a per-user `Position` PDA (`[b"position", mint, user]`) storing deposited shares, and a `withdraw_tokens` that only allows withdrawing your own share. Use your share math from 1.3.
- [ ] **3.4 Find the bugs.** The struct below has at least four bugs. List each, the attack, and the fix. Answers are below.

```rust
#[derive(Accounts)]
pub struct Claim<'info> {
    pub user: AccountInfo<'info>,
    #[account(mut)]
    pub reward_pool: Account<'info, RewardPool>,
    #[account(mut, seeds = [b"pool_auth"], bump)]
    /// CHECK: signer for the pool vault
    pub pool_authority: UncheckedAccount<'info>,
    #[account(mut)]
    pub pool_vault: InterfaceAccount<'info, TokenAccount>,
    #[account(mut)]
    pub user_ata: InterfaceAccount<'info, TokenAccount>,
    pub token_program: AccountInfo<'info>,
}
```

### Level 4: Stretch

- [ ] **4.1** Write PoCs for sealevel-attacks 0–3 (signer authorization, account data matching, owner checks, type cosplay) using LiteSVM.
- [ ] **4.2** Port the Part C vault to [Pinocchio](https://github.com/anza-xyz/pinocchio) and compare compute units per instruction.
- [ ] **4.3** Create a Token-2022 mint with a transfer fee in a LiteSVM test and show that crediting `amount` instead of `received` in Part E makes the vault insolvent.

### Answers (read only after trying)

**Part A self-check.** (1) The same account can appear twice in one instruction; both `AccountInfo`s must share one buffer, so mutability is checked at runtime. (2) It panics (`already mutably borrowed`); `try_borrow_mut` returns `Err` instead. (3) It returns early with that `Err` / `None`. (4) It keeps the low 64 bits and silently drops the rest. (5) Borsh writes only field values, no type names, so any bytes of the right length decode as any struct of that shape.

**Part B self-check.** (1) The client, via `AccountMeta`; the program can only check. (2) A PDA has no private key, and `create_account` requires the new account to sign; `invoke_signed` supplies that signature from seeds. (3) `account.owner` is the program that owns the account (Token program); the token account's `owner` field is the wallet/PDA that controls the tokens. (4) It tries bumps from 255 downward, hashing each time until one is off-curve. (5) The IDL in `target/idl/`.

**Part C.4.** (1) Check 6 ties the vault to the signer and check 5 re-derives the PDA from the same signer, so with 1, 2 and 6 the vault's identity is already fixed; keep 5 anyway as defence in depth (it also catches a mis-stored bump). (2) `create_account` fails because the address already holds lamports: the victim can never initialize (DoS). (3) The debit and credit cancel out; lamports are unchanged, but in more complex programs aliasing breaks accounting. (4) Not a bug: the System Program's `transfer` checks `from` signed; the user's signer privilege flows into the CPI. (5) Check authority signs; move all lamports to the authority; zero data and assign to System Program (also: refuse recipient == vault).

**Part D.5.** (1) Signer: change `Signer` to `UncheckedAccount`. Data matching: remove `has_one = authority`, but then `seeds` still binds it, so you must remove both. PDA sharing: change seeds to `[b"vault"]`. (2) `Initialize` creates an account via CPI to System; `Withdraw` moves lamports directly from a program-owned account. (3) Lamport changes would fail (account not writable), so the tx errors.

**Part E.6.** (1) X = Token program (`Tokenkeg…`), Y = the user's wallet. (2) Without it, the user could pass someone else's token account where they are a *delegate*; the Token program would allow it, but the program's accounting would credit the wrong person. Also it pins the mint. (3) `received` is computed from a stale balance (0 change), so the user is credited nothing, or with `amount` the fee loss is hidden.

**Part F.5.** (1) Check 5 derives the PDA from the attacker's key, which isn't the victim's vault address, so it fails. (2) `IllegalOwner` from check (1). (3) That a single account passed in two roles produces a balance or state change that couldn't happen with two distinct accounts.

**Exercise 3.4.** (a) `user` is `AccountInfo`, not `Signer`: anyone claims for anyone. (b) `pool_authority` seeds `[b"pool_auth"]` are global: one authority for every pool, so a claim against one pool can drain another's vault. (c) No relationship between `reward_pool`, `pool_vault` and `user_ata` (`has_one`, `token::authority`, `token::mint`): attacker substitutes accounts. (d) `token_program` is `AccountInfo`: arbitrary CPI. Also `pool_authority` doesn't need `mut`.

## Bridge: from this guide to the Security Deep-Dive

When levels 1–3 are done (and after [02 · Anchor Deep-Dive](./02-anchor-deep-dive.md)), read [03 · Solana Security Deep-Dive](./03-solana-security-deep-dive.md) with this table beside you. Each deep-dive section should point to code you've already written. If a section still feels abstract, go back to the matching part here before moving on.

| Deep-dive section | Builds on (this guide) | Make sure you can… |
| --- | --- | --- |
| 1.1–1.3 Accounts, ownership rules, signer/writable | A.2, A.3, B.2–B.4, C | Point to the line in your Part C vault that each ownership rule protects |
| 1.4 Rent | B.6, C check (7), exercise 2.3 | Explain the pre-funding DoS using your own test |
| 1.5 PDAs | B.5, C check (5), E (vault authority) | Say why `[b"authority", mint]` in Part E is shared and what that requires |
| 1.6 Transactions, duplicates, introspection | B.3, exercise 2.4 | Explain duplicates using the `Rc<RefCell<…>>` model from A.3 |
| 2 Anchor | D, exercise 3.1–3.4 | Do deep-dive exercise 2.4 in under 10 minutes |
| 3.1 System Program | B.6, B.7, C | Say why `withdraw` in Part C can't use System `transfer` |
| 3.2–3.4 Token, ATA, Token-2022 | E, exercise 3.3, 4.3 | Name the two "owners" and the transfer-fee pattern without notes |
| 3.5 Upgradeable loader | B.1 (upgrade authority) | Explain why upgrade authority is a trust assumption |
| 4 CPIs | B.7, C (`invoke_signed`), E.4 (`reload`) | Trace signer privilege through your Part E withdraw |
| 5 Protocols | All of it | Read Raydium CP-Swap's `swap_base_input.rs` and list every account check |
| 6 Bug classes | C.3, exercise 3.2, 3.4, 4.1 | Map each sealevel-attack to a check from C.3 |
| 7 Runtime | A.3, B.2, F | Explain what `deserialize_parameters` does with your vault's lamport change |

**How to read it:** one deep-dive section per day. Before each, write down the three most important points you expect it to make, then read and compare. Note anything that still doesn't click; if the guide is unclear, [open an issue](https://github.com/KhomDev/rugproof/issues) so it can be improved.

## Self-assessment

There is no answer key for this part on purpose: write your answers first, then check them against the guide and mark yourself honestly. Better still, swap with a study partner and mark each other.

**Suggested format** (about 3–4 hours, closed book, no running code except in section 4):

| Section | What to do | Weight | Covers |
| --- | --- | --- | --- |
| 1. Rust for Solana | Answer the Part A self-check again, then predict the output of three snippets from Part A | 15% | Part A |
| 2. Concepts | Explain owner vs token-account owner, signer vs writable, PDA derivation, rent exemption, and CPI signer privileges, each in two sentences | 20% | Part B |
| 3. Code review | Take the Part D vault and Exercise 3.4's struct; for each, list every bug, the attack, and the fix | 35% | Parts C–E |
| 4. PoC | Remove one check from your Part C vault (pick it at random) and write a LiteSVM test that exploits it | 20% | Part F |
| 5. Design | Write half a page on how you'd extend the vault to many users without a shared PDA authority | 10% | Parts C–E |

**Pass mark:** ≥ 70% overall and ≥ 60% in section 3 means you're ready for the deep-dives. Below that, redo the parts that cost you the most points.

**Ground rule:** "I don't know" is a better answer than a confident wrong guess. In audits, a false positive costs credibility.

---

## Further reading

- [Solana docs: core concepts](https://solana.com/docs/core)
- [The Rust Book](https://doc.rust-lang.org/book/), chapters 10 (generics, traits, lifetimes) and 15 (smart pointers, `RefCell`)
- [LiteSVM](https://github.com/LiteSVM/litesvm) and [Mollusk](https://github.com/anza-xyz/mollusk) for fast program tests
- [sealevel-attacks](https://github.com/coral-xyz/sealevel-attacks)

---

**Previous:** [← 00 · Rust Fundamentals](./00-rust-fundamentals.md) · **Next:** [02 · Anchor Framework Deep-Dive →](./02-anchor-deep-dive.md)
