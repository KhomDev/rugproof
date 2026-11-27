# Solana Foundations: Rust and Solana Concepts in Code

> **Rugproof** · ← [00 · Rust Fundamentals](./00-rust-fundamentals.md) · [Series overview](./README.md) · [02 · Anchor Deep-Dive](./02-anchor-deep-dive.md) →

**Prerequisites:** [00 · Rust Fundamentals](./00-rust-fundamentals.md), or equivalent (roughly the first half of [The Rust Book](https://doc.rust-lang.org/book/) plus 00 §13). A terminal, and an evening to install the toolchain (see *Setup* below).

**This file covers:** Solana terms as they look in code (accounts, owner, signer, PDAs, rent, CPIs, sysvars, discriminators); a native vault written by hand; the same vault ported to Anchor; SPL / Token-2022 account layouts and a token vault; and the LiteSVM test pattern that proves a check matters. This is the home for the account model as code.

**This file does not cover:** Rust language basics or the Rust security checklist (those stay in [00](./00-rust-fundamentals.md)); Anchor macros, constraints and Anchor-specific bug patterns (those start in [02](./02-anchor-deep-dive.md)); the Agave runtime, explorer verification, and reading live protocols (those are [03](./03-solana-security-deep-dive.md)); or the full bug-class catalogue and incident write-ups ([04](./04-advanced-security-and-exploits.md)).

**Exercises:** [jump to the Exercises section](#exercises) — graded items with hints and answers in collapsible sections.

## Learning objectives

By the end of this guide you can:

- Read the Solana types that 00 prepared you for (`AccountInfo<'info>`, `Rc<RefCell<…>>`, `ProgramResult`, Borsh accounts, discriminators) without stopping
- Map every core Solana term (account, owner, signer, PDA, rent, CPI, sysvar) to the struct or call where it lives
- Write a small native program by hand and explain which attack each of its checks stops
- Port that program to Anchor and say what the macros generate
- Write your first proof-of-check test in LiteSVM (a test that the attack fails against the secure program, and succeeds if you remove the check)

## How to use this guide

**The uncomfortable truth first:** Solana security material usually feels hard because it assumes two things most newcomers don't have yet: Rust past the basics, and the habit of seeing Solana concepts as concrete structs and function calls. Guide 00 built the first. This guide builds the second. It is not optional reading; skipping it means memorizing bug classes you can't recognize in real code. \[Likely\]

**Who this is for:** someone who has finished [guide 00](./00-rust-fundamentals.md), or is about a month into the Rust Book and willing to keep 00 open for reference. Part A does not re-teach Rust. It only shows where those ideas live in Solana types.

**The path**

| Phase | What you do | Time | You're done when… |
| --- | --- | --- | --- |
| 1. Read this guide | Parts A–F, in order. Type every code block yourself; don't copy-paste | 2–3 weeks | You can explain every line of the native program in Part C without looking |
| 2. Exercises | The graded set in the Exercises section. Levels 1–3 required, level 4 stretch | 2–3 weeks | Each exercise compiles, its tests pass, and you checked your answer against the self-check |
| 3. Self-assessment | The self-assessment at the end of this guide | 1 sitting (3–4 h) | You score ≥ 70% on your own honest marking |
| 4. Move on | [02 · Anchor Deep-Dive](./02-anchor-deep-dive.md), then [03 · Solana Security Deep-Dive](./03-solana-security-deep-dive.md), using the Bridge section as a map | | |

If a self-assessment section goes badly, go back to the matching part of this guide before moving on.

**Setup before you start** (one evening):

- Rust via `rustup`, and the Solana CLI (Agave) + Anchor via the official installer on [solana.com/docs/intro/installation](https://solana.com/docs/intro/installation). \[Likely; installer URL may change\]
- An editor with `rust-analyzer`. Hovering types is how you learn `'info` lifetimes.
- `cargo install cargo-expand` so you can see what Anchor macros generate (Part D).
- You do **not** need a scratch Cargo workspace for Rust-only drills anymore; those live in [00's exercise crate](./exercises/00-rust-fundamentals).

> **About the code.** Blocks introduced as a *full program* or *Cargo.toml* are meant to build as written. Every other block is a **snippet**: it shows one idea and omits imports or surrounding code. Solana crates release often, so if an import path has moved in your version, check the crate docs on [docs.rs](https://docs.rs) (and please send a PR). Vulnerable snippets are deliberately broken: don't deploy them. Only run tests against programs you wrote.

## Part A: Rust applied to Solana types

The language itself lives in [00 · Rust Fundamentals](./00-rust-fundamentals.md). This part only shows how those ideas appear in Solana types, plus two topics 00 does not cover: macros, and zero-copy / `repr(C)` layouts. If a Rust idea still feels shaky, go back to the linked section in 00 before continuing. \[Certain\]

| You need this idea | Home in 00 | You will meet it here as |
| --- | --- | --- |
| Ownership and borrowing | [§2](./00-rust-fundamentals.md#2-ownership-and-borrowing) | The runtime owns account data and lends it for one instruction |
| Lifetimes | [§3](./00-rust-fundamentals.md#3-lifetimes) | `AccountInfo<'a>` / `AccountInfo<'info>` |
| `Rc` and `RefCell` | [§8](./00-rust-fundamentals.md#8-smart-pointers-rc-and-refcell) | `lamports` and `data` on `AccountInfo` |
| Structs, enums, `match` | [§4](./00-rust-fundamentals.md#4-structs-enums-and-match) | Instruction enums and dispatch (Part C) |
| `Result`, `?`, custom errors | [§5](./00-rust-fundamentals.md#5-error-handling-option-result--and-thiserror) | `ProgramResult`; never `unwrap()` on user input ([00 §13.2](./00-rust-fundamentals.md#132-panics-are-a-denial-of-service)) |
| Traits and generics | [§6](./00-rust-fundamentals.md#6-traits-and-generics) | `Owner` / `Discriminator` bounds behind `Account<'info, T>` |
| Integers, casts, rounding | [§9](./00-rust-fundamentals.md#9-integers-overflow-casts-and-rounding), [§13](./00-rust-fundamentals.md#13-rust-security-for-on-chain-code) | Token amounts are `u64`; math in `u128`; `overflow-checks = true` |
| Borsh | [§10](./00-rust-fundamentals.md#10-serialization-with-borsh) | Account bytes, then zero-copy / discriminators below |
| `unsafe`, `no_std` | [§12](./00-rust-fundamentals.md#12-unsafe-and-no_std-recognition-only) | Pinocchio and p-token (recognition only) |

### A.1 `AccountInfo`: one struct, every account

Your program never owns account data. The runtime owns it and *lends* it for the duration of the instruction. That is why every account arrives as a reference with a lifetime. `'info` is just a lifetime name Anchor chose; it means "for the duration of this instruction". When the compiler complains about `'info`, you almost always tried to store an account reference somewhere that lives longer than the instruction, or mixed two structs with different lifetimes.

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

This struct (from `solana-account-info`) is the most important type in Solana. Read every field type until it makes sense. `is_signer` and `is_writable` are **flags on this transaction**, not properties stored in the account (Part B). \[Certain\]

`lamports` and `data` are `Rc<RefCell<&mut …>>` because the same account can appear twice in one instruction, and both `AccountInfo`s must point to the same bytes. That is the runtime basis of the duplicate-mutable-accounts bug ([04 §1.9](./04-advanced-security-and-exploits.md#19-duplicate-mutable-accounts)). If you hold `account.data.borrow_mut()` and then call a CPI that needs the same account, you get `AccountBorrowFailed`: drop the borrow first (end its scope, or `drop(guard)`). Use `try_borrow_mut` in program code so a conflict is an error, not a panic ([00 §13.6](./00-rust-fundamentals.md#136-refcell-borrow-panics-in-accountinfo)).

```rust
// reading and writing lamports in a native program
let amount = **from.lamports.borrow();          // two derefs: RefCell guard → &mut u64 → u64
**from.try_borrow_mut_lamports()? -= 10;
**to.try_borrow_mut_lamports()? += 10;
```

### A.2 Errors stop the whole transaction

Every handler returns `ProgramResult` (`Result<(), ProgramError>`). Returning `Err` aborts the **whole transaction** and rolls back every account change. That is the main safety mechanism: check, and return an error if the check fails. Anchor's `require!(cond, MyError::X)` and `#[error_code]` enums are sugar over this. The `?` operator is the same one taught in [00 §5](./00-rust-fundamentals.md#5-error-handling-option-result--and-thiserror).

**Never `unwrap()` on user input in a program.** A panic also aborts the transaction (so it is not a fund-loss bug by itself), but it hides intent and, on a path other users depend on, is a denial of service ([00 §13.2](./00-rust-fundamentals.md#132-panics-are-a-denial-of-service)).

The SPL Token program uses its own `COption<Pubkey>` (a C-compatible Option) for `mint_authority` and `freeze_authority`, because the mint's byte layout must be fixed. You will use those fields as a defender in [01, Part E](#part-e-tokens-in-code) and verify them on-chain in [03 §3.6](./03-solana-security-deep-dive.md#36-read-only-verification-explorers-and-the-cli).

### A.3 Traits, as `Account<'info, T>` uses them

[00 §6](./00-rust-fundamentals.md#6-traits-and-generics) is the home for traits and generics. Here is the Solana form. A trait can describe the *type*, not a value:

```rust
trait Owner { fn owner() -> Pubkey; }

struct Vault { amount: u64 }
impl Owner for Vault { fn owner() -> Pubkey { crate::ID } }

fn check_owner<T: Owner>(acc: &AccountInfo) -> Result<(), ProgramError> {
    if *acc.owner != T::owner() { return Err(ProgramError::IllegalOwner); }
    Ok(())
}

check_owner::<Vault>(vault_info)?;
```

You write the owner check once. `#[account] pub struct Vault { … }` makes the Anchor macro write `impl Owner for Vault`, `impl Discriminator for Vault` and the serialization traits for you. `Account<'info, Vault>` then runs a generic check: is the account's owner `Vault::owner()`, and do its first 8 bytes equal `Vault`'s discriminator? That proves "owned by this program and really a `Vault`", and nothing about *which* vault. Other traits you will meet constantly: `BorshSerialize`/`BorshDeserialize`, `Pack` (SPL Token), `Pod`/`Zeroable` (bytemuck), `Accounts`, `ToAccountInfos`, `Key`. The macros behind this are in A.4; the full Anchor expansion is [02](./02-anchor-deep-dive.md).

If `owner()` returned `system_program::ID` instead of `crate::ID`, the check would accept plain wallets as vaults and reject real ones.

### A.4 Macros: what you see is not what runs

00 does not cover macros. Two kinds appear everywhere on Solana:

- **Declarative** (`macro_rules!`, called with `!`): `msg!`, `require!`, `entrypoint!`, `declare_id!`.
- **Procedural**: attributes and derives that rewrite your code at compile time: `#[program]`, `#[derive(Accounts)]`, `#[account]`, `#[derive(BorshSerialize)]`.

The security consequence: **what you see is not what runs.** `#[derive(Accounts)]` generates the validation code. To audit it, run `cargo expand` and read the output. Part D does this, and [02](./02-anchor-deep-dive.md) is the full map.

### A.5 Account bytes: Borsh, `repr(C)`, zero-copy, discriminators

Account data is just a row of bytes. Solana has no idea what's inside. [00 §10](./00-rust-fundamentals.md#10-serialization-with-borsh) is the home for Borsh sizes and rules (`Vec`/`String` length prefixes, `Option` tags, little-endian integers, no type names). This section is only what you do with those bytes in a program. \[Certain\]

**Read, change, write back.** The struct you deserialize is a **copy**. Forget the write-back and your change is silently lost. In Anchor, the write-back happens automatically for `mut` accounts, which is why changes without `mut` vanish (Part D).

```rust
let mut c = Counter::try_from_slice(&account.data.borrow())?;
c.count += 1;
c.serialize(&mut &mut account.data.borrow_mut()[..])?;
```

`try_from_slice` fails if bytes are **left over**. If the account is 50 bytes but `Counter` is 40, it errors. Size the account exactly, or use `deserialize`, which ignores extra bytes (Anchor does this). About `&mut &mut`: `serialize` writes into a `&mut [u8]` and needs a mutable reference to that slice so it can move forward. Copy the pattern.

**Zero-copy.** Copying is fine for small accounts. For big ones (an order book of 50 KB), copying costs compute units and doesn't fit the program's 4 KB stack frames. Zero-copy treats the bytes **as the struct, right where they are**. The CPU wants numbers at aligned addresses (a `u64` at a multiple of 8), so Rust inserts invisible **padding**, and `#[repr(C)]` makes that layout predictable:

```rust
#[repr(C)]
struct Example { a: u8, b: u64 }
```

| Format | Layout | Size |
| --- | --- | --- |
| Borsh | `a` (1) + `b` (8) | 9 bytes |
| `repr(C)` in memory | `a` (1) + **padding (7)** + `b` (8) | 16 bytes |

Same struct, two layouts, so **never mix the formats**. Libraries like `bytemuck` refuse structs with hidden padding; you add explicit fields (`_pad: [u8; 7]`) so every byte is accounted for. Without `#[repr(C)]`, Rust may reorder fields. You'll see zero-copy as `#[account(zero_copy)]` + `AccountLoader` in Anchor ([02 §6.4](./02-anchor-deep-dive.md#64-zero-copy-for-large-accounts)), `bytemuck::from_bytes` in native code, and pointer casts in Pinocchio.

**Why the discriminator exists.** Bytes carry no type, so these two structs look identical in an account:

```rust
struct Counter { authority: Pubkey, count: u64 }  // 40 bytes
struct Config  { admin: Pubkey,     fee: u64 }    // 40 bytes
```

Read a `Config` as a `Counter` and Borsh succeeds: `admin` becomes `authority`, `fee` becomes `count`. An attacker who is `admin` of some `Config` can pass it where a `Counter` is expected and act as its `authority`. That's the **type cosplay** bug ([04 §1.3](./04-advanced-security-and-exploits.md#13-account-confusion-and-type-cosplay)). The fix is a type tag at the start:

```text
bytes 0..8    discriminator   ← "this is a Counter"
bytes 8..40   authority
bytes 40..48  count
```

Anchor's `#[account]` writes `sha256("account:Counter")[..8]` there and `Account<'info, Counter>` rejects anything else; that's why Anchor space is always `8 + …`. In Part C you do it by hand with `VAULT_DISC`.

### A.6 `unsafe`, `no_std`, and Pinocchio (recognition only)

Programs run in a 4 KB-frame stack, 32 KB heap VM. High-performance frameworks (Pinocchio, p-token) avoid allocations and parse the input buffer with raw pointers inside `unsafe` blocks. You don't need to write `unsafe` yet; you need to recognize that **inside `unsafe`, the compiler stops protecting you**, so auditors read those blocks line by line (bounds, alignment, aliasing). The recognition checklist is [00 §12](./00-rust-fundamentals.md#12-unsafe-and-no_std-recognition-only) and [00 §13.9](./00-rust-fundamentals.md#139-unsafe-and-undefined-behaviour). \[Certain\]

### A.7 Rust security, not repeated

Six habits that prevent a large share of real Solana findings. Each is covered in depth in [00 §13](./00-rust-fundamentals.md#13-rust-security-for-on-chain-code), with program-level examples in [04 §1.16](./04-advanced-security-and-exploits.md#116-rust-level-bugs-panics-compute-memory-and-the-supply-chain):

1. `overflow-checks = true` in the workspace `[profile.release]`, **and** `checked_*` arithmetic on anything a user influences.
2. No `unwrap()`, `expect()`, `a[i]` or unchecked `/` in program code: a panic in an instruction others rely on is a denial of service.
3. `u64::try_from(x)` instead of `x as u64`.
4. Decide the rounding direction of every division (against the user).
5. Drop `RefCell` borrows (`try_borrow_mut_data`, zero-copy `load_mut`) before any CPI.
6. Bound every loop and every user-growable collection: compute is a budget an attacker can spend for you.

### A.8 Self-check

You're ready for Part B when you can answer, without looking:

1. Why is `AccountInfo::data` an `Rc<RefCell<&mut [u8]>>` and not a `&mut [u8]`?
2. What happens at runtime if you call `borrow_mut()` twice on the same `RefCell` without dropping the first? What should program code use instead?
3. You call `try_from_slice`, change a field, and return `Ok(())`. What's in the account afterwards?
4. Why would `try_from_slice` fail on a 48-byte Anchor account holding a 40-byte struct?
5. Two account types both start with a 32-byte pubkey. What stops one being passed as the other in Anchor? In Part C?

<details>
<summary>Answers</summary>

1. The same account can appear twice in one instruction; both `AccountInfo`s must share one buffer, so the "one mutable borrow" rule is enforced at runtime ([00 §8](./00-rust-fundamentals.md#8-smart-pointers-rc-and-refcell)).
2. It panics (`already mutably borrowed`). Use `try_borrow_mut` and return an error.
3. The old value: you only changed the copy.
4. The 8-byte discriminator comes first, so the bytes don't line up; strip it first, as Anchor does.
5. Anchor: the discriminator check in `Account<T>`. Part C: the owner check plus `VAULT_DISC` in `load_vault`.

</details>

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

**Authorities a defender records.** `mint_authority` and `freeze_authority` (and, on Token-2022, the extension list) are powers someone can use against holders and against a vault that custodians the mint. Part E shows you where those fields live. How to look them up on a *live* mint, without sending a transaction, is [03 §3.6](./03-solana-security-deep-dive.md#36-read-only-verification-explorers-and-the-cli). The bug-class view is [04 §1.14](./04-advanced-security-and-exploits.md#114-token-2022-extension-pitfalls) and [04 §1.15](./04-advanced-security-and-exploits.md#115-upgrade-authority-and-admin-key-risk).

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

<a id="exercises"></a>
## Exercises

A graded set in four levels. **Levels 1–3 are required** (twelve exercises); level 4 is stretch (three more). Hints and answers are in collapsible blocks. Rust-only drills stay in [00](./00-rust-fundamentals.md#exercises) — do not redo them here. Do not deploy deliberately broken snippets. Only test programs you wrote, or public educational repos the text names.

### Level 1: the account model, and spotting a missing check (Parts A, B)

These replace the Rust-only drills, which now live only in [guide 00](./00-rust-fundamentals.md#14-exercises). If share math, `RefCell`, or Borsh layouts still feel shaky, do 00's Level 3 first.

- [ ] **1.1 Spot the red flag.** The withdraw snippet below is deliberately incomplete. Name every missing check, the sentence the program never proved, and the one-line fix for each. Do not write an attack script; this is a reading exercise.

```rust
// SNIPPET for detection only. Deliberately incomplete. Do not deploy.
fn withdraw(accounts: &[AccountInfo], amount: u64) -> ProgramResult {
    let authority = &accounts[0];
    let vault     = &accounts[1];
    let recipient = &accounts[2];

    let mut data = Vault::try_from_slice(&vault.data.borrow())?;
    data.amount  = data.amount.checked_sub(amount).ok_or(ProgramError::InsufficientFunds)?;
    data.serialize(&mut &mut vault.data.borrow_mut()[..])?;

    **vault.try_borrow_mut_lamports()?     -= amount;
    **recipient.try_borrow_mut_lamports()? += amount;
    Ok(())
}
```

- [ ] **1.2 Account-model quiz.** Answer without looking, then open the hint if you are stuck:
  1. Who sets `is_signer` and `is_writable` — the program, or the client?
  2. Why can't a native program use `system_program::transfer` to move lamports out of a vault PDA that holds data?
  3. What is the difference between `AccountInfo.owner` and a token account's `owner` field?
  4. Why does `find_program_address` loop downward from bump 255?
  5. An attacker sends 1 lamport to a vault PDA before `initialize`. What happens, and why?

<a id="exercise-1-3"></a>
- [ ] **1.3 Read-only walkthrough: official programs.** Using only a block explorer and/or the Solana CLI, look up these **documented** program IDs (they are the official System and Token programs; do not substitute a random token):

  - System Program: `11111111111111111111111111111111`
  - SPL Token: `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA`

  Open each in [Solana Explorer](https://explorer.solana.com) (`/address/<ID>`) or [Solscan](https://solscan.io) (`/account/<ID>`), and/or run `solana program show <ID>` ([docs](https://solana.com/docs/programs/deploying)). Write down, for each: whether it is executable, which loader/owner the page reports, and what the CLI puts in the `Authority` field if it prints one. You are practising the habit, not memorizing a famous answer. The same habit, applied to a *protocol* program and a *mint*, is [03 §3.6](./03-solana-security-deep-dive.md#36-read-only-verification-explorers-and-the-cli).

- [ ] **1.4 Checklist: a described mint (hypothetical).** A write-up (not a real token) says: "HarborUSD is a stablecoin. The team keeps mint authority and freeze authority on the same wallet so they can 'react quickly'. Liquidity will be added on an AMM next week." No address is given, and you should not invent one. List: (a) what the holder of mint authority can do; (b) what freeze authority can do to a depositor's token account; (c) what you would still need to verify on-chain before treating this as safe collateral, in the vocabulary of Part B and Part E. Fuller verification steps are in [03 §3.6](./03-solana-security-deep-dive.md#36-read-only-verification-explorers-and-the-cli).

<details>
<summary>Level 1 hints</summary>

- **1.1** Walk the checklist from Part C.3: owner, signer, destination, PDA/identity, arithmetic, aliasing (`vault == recipient`). The snippet also never writes a discriminator check.
- **1.2** (1) Part B.3–B.4. (2) Part B.7 and C — System `Transfer` requires a System-owned account with no data. (3) Part E and B. (4) Part B.5. (5) Exercise 2.3 and C.4.
- **1.3** Official explorers and `solana program show` are read-only. If the CLI errors because the account is a native builtin, record that fact: not every program is an upgradeable-loader program.
- **1.4** Mint authority can increase supply. Freeze authority can freeze token accounts so they cannot transfer. "The team said they will be responsible" is not a check.

</details>

<details>
<summary>Level 1 answers</summary>

**1.1** Missing sentences and fixes (any equivalent wording is fine):

- "This vault is owned by our program" — check `vault.owner == program_id` (and, in Part C, the discriminator).
- "The authority is the one stored in the vault, and they signed" — require `authority.is_signer` and `authority.key == data.authority` (or the equivalent PDA derivation).
- "This is the vault we derived, not a substitute" — re-derive the PDA / check identity.
- "The recipient is a real destination, and is not the vault itself" — reject `vault.key == recipient.key`; confirm recipient is writable and is the intended account.
- "The lamport debit cannot underflow and the credit cannot overflow" — the `checked_sub` on `data.amount` is good; the raw `-=` / `+=` on lamports is not.

**1.2** (1) The client, via `AccountMeta`; the program can only check. (2) System `Transfer` requires `from` to be System-owned with no data; a vault PDA that holds state must be debited by its owner program. (3) `AccountInfo.owner` is the program that owns the account (the Token program for a token account); the token account's `owner` field is the wallet or PDA that controls the tokens. (4) It searches for the canonical (first off-curve) bump, starting at 255. (5) `CreateAccount` fails because the address already holds lamports: the victim can never initialize (a pre-funding DoS).

**1.3** There is no single required paragraph. A complete answer records executable / owner / authority (or "CLI cannot show Authority because this is a builtin") for both IDs, and says which tool produced each fact.

**1.4** (a) Mint more HarborUSD, diluting holders and any vault that prices the token as if supply were fixed. (b) Freeze a user's (or a protocol vault's) token account, which blocks transfers, liquidations and withdrawals. (c) On-chain you still need the mint's actual `mint_authority` and `freeze_authority` (present or `None`), the token program that owns the mint, any Token-2022 extensions, who holds LP tokens if a pool exists, and whether the program that will custody the token is upgradeable. Marketing text is not a source.

</details>

### Level 2: Native programs (Parts B, C, F)

- [ ] **2.1 Build and test Part C.** Build the native vault and get the F.2 test passing. Then make it fail by removing check (3).
- [ ] **2.2 Add `Close`.** Add an instruction that sends all lamports to the authority and closes the vault. Requirements: authority signs, zero the data, assign the account back to the System Program, and the recipient must not be the vault itself. Write a test that a second `Withdraw` on the closed vault fails.
- [ ] **2.3 Fix the pre-funding DoS.** Write a test where an attacker sends 1 lamport to the victim's future vault PDA before `initialize`, and show `initialize` fails. Then fix `initialize` (transfer the shortfall, then `allocate` and `assign` with `invoke_signed`) and show the test passes.
- [ ] **2.4 Duplicate accounts.** Write a test passing the vault as both `vault` and `recipient` in `Withdraw`. Explain what happens to the lamports, and add a check if you think one is needed.

<details>
<summary>Level 2 hints</summary>

- **2.1** Get the F.2 test green first. Removing check (3) should make the 'attack must fail' test itself fail.
- **2.2** Close requirements are listed in the exercise; C.4.5 is the checklist.
- **2.3** `CreateAccount` fails on a pre-funded address; the fix is transfer-shortfall + `allocate` + `assign` with `invoke_signed`.
- **2.4** Pass the same pubkey in two roles and look at the lamport change. Aliasing is the lesson; 04 §1.9 is the full class.

</details>

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

<details>
<summary>Level 3 hints</summary>

- **3.1** `close = authority` is the Anchor form of 2.2. `cargo expand` is the point of the exercise.
- **3.2** Remove *one* constraint at a time. `seeds` can still bind an account after you remove `has_one` — see D.5.
- **3.3** Per-user PDA seeds must include the user. Reuse the share math from 00, not a new formula.
- **3.4** Four (actually five) missing proofs: signer, scoped PDA, relationships, token program pinned, and `mut` on a PDA that doesn't need it.

</details>

### Level 4: Stretch

- [ ] **4.1** Write PoCs for sealevel-attacks 0–3 (signer authorization, account data matching, owner checks, type cosplay) using LiteSVM.
- [ ] **4.2** Port the Part C vault to [Pinocchio](https://github.com/anza-xyz/pinocchio) and compare compute units per instruction.
- [ ] **4.3** Create a Token-2022 mint with a transfer fee in a LiteSVM test and show that crediting `amount` instead of `received` in Part E makes the vault insolvent.

### Answers (read only after trying)

**Part A self-check.** Answers are now under [A.8](#a8-self-check), next to the questions. The older five-question Rust recap lives in [00, Self-check](./00-rust-fundamentals.md#self-check).

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
| 2 Anchor, as an auditor | D, exercise 3.1–3.4 | Do deep-dive exercise 2.3 in under 10 minutes |
| 3.6 Read-only verification | B.1, E, exercise 1.3–1.4 | Fill an upgrade-authority and mint-authority sheet from an explorer without sending a transaction |
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
