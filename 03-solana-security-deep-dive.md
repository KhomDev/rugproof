# Solana Security Deep-Dive: From Account Model to Agave Runtime

> **Rugproof** · ← [02 · Anchor Deep-Dive](./02-anchor-deep-dive.md) · [Series overview](./README.md) · [04 · Advanced Security & Exploits](./04-advanced-security-and-exploits.md) →

## Learning objectives

By the end of this guide you can:

- Reason about any instruction's account list from first principles: who owns each account, who must sign, who may write
- State the invariants the System, Token, ATA, Token-2022 and upgradeable-loader programs enforce for you, and the ones they don't
- Trace signer privileges through CPIs
- Read an AMM, a lending market and a perp DEX with a repeatable method
- Apply the full bug-class catalogue as a per-instruction checklist
- Follow a transaction through the Agave runtime down to your entrypoint

## Prerequisites

[01 · Solana Foundations](./01-solana-foundations.md) (Parts A–F) and [02 · Anchor Deep-Dive](./02-anchor-deep-dive.md).

> **About the code.** All code here is **snippets** that illustrate one check or bug. File paths in Agave and protocol repos refer to their `master`/`main` branches at the time of writing and can move.

## How to use this guide

**The uncomfortable truth first:** this guide is a map, not the territory. Reading it gets you vocabulary; you only become a Solana auditor by writing exploits against deliberately broken programs and by reading real protocol code until account validation is a reflex. Budget 8–12 weeks of daily work for the seven parts, and expect part 5 (reading three protocols) to take as long as parts 1–4 combined. \[Likely\]

**Why Solana is different from EVM, in one sentence:** on EVM a contract owns its storage and the danger is *what code runs*; on Solana programs are stateless and every piece of state arrives as an untrusted argument, so the danger is *which accounts you were handed*. Almost every Solana bug class is a variant of "the program trusted an account it did not verify". \[Certain\]

**Confidence tags.** Claims carry \[Certain\] (spec-level or read in source), \[Likely\] (strong evidence, may drift with versions) or \[Guessing\] (the author's inference). Version-sensitive details (Anchor syntax, Agave module names, feature gates) drift every few months; verify against the tag you are auditing, not against this doc.

**Study order and time budget**

| Part | Goal | Time | Done when you can… |
| --- | --- | --- | --- |
| 1 Account model | Reason about any instruction's account list from first principles | 1 week | Explain why every account in a Raydium swap is passed, and who can write to each |
| 2 Anchor | Know the exact check behind each attribute | 1 week | Predict, without running it, whether a given `#[derive(Accounts)]` struct is exploitable |
| 3 Core programs | Know the invariants the token and loader programs enforce for you | 1 week | Name every check `spl_token::transfer` does and does not do |
| 4 CPIs | Trace privilege flow across program boundaries | 3–4 days | Spot a signer-privilege escalation and an arbitrary-CPI bug on sight |
| 5 Protocols | Read an AMM, a lending market, a perp DEX end to end | 3–5 weeks | Write a one-page threat model for each, listing every trust assumption |
| 6 Bug classes | Turn patterns into a checklist | 1 week + ongoing | Find a seeded bug in an unfamiliar program in under an hour |
| 7 Runtime | Understand what the validator actually enforces | 1–2 weeks | Explain what happens between `sendTransaction` and your entrypoint, with file names |

Read parts 1–4 in order. Part 6 can run in parallel with part 5: each time you read a protocol, try to apply every bug class to it. Part 7 matters most for the edge cases (realloc, duplicate accounts, CPI depth, compute limits) that appear in high-severity findings.

## 1. The account model

Everything on Solana is an account: a flat key→value store where the key is a 32-byte address and the value is a small fixed record. Programs are accounts. Token balances are accounts. Config is accounts. Programs hold no state of their own; they receive accounts as inputs and mutate them in place. \[Certain\]

### 1.1 What an account is

Every account has exactly these fields (Agave's `AccountSharedData`, mirrored by `AccountInfo` inside a program): \[Certain\]

| Field | Type | Meaning | Who can change it |
| --- | --- | --- | --- |
| `lamports` | u64 | Balance in lamports (1 SOL = 10^9) | Owner can debit; anyone can credit |
| `data` | `Vec<u8>` | Opaque bytes, max 10 MiB | Only the owner program, only if writable |
| `owner` | Pubkey | Program allowed to modify `data` and debit lamports | Only the current owner, only if data is zeroed and account writable |
| `executable` | bool | Account holds a loadable program | Loader only; one-way |
| `rent_epoch` | u64 | Legacy rent bookkeeping | Effectively fixed at `u64::MAX` for exempt accounts today |

The address is **not** stored in the account; it is the key that indexes it. Two consequences: an address alone tells you nothing about what is behind it, and the same bytes can live at any address. That is why "check the owner and the discriminator" is the first rule of Solana security.

### 1.2 The runtime's ownership rules

Agave enforces these rules at the moment of every write: each mutation goes through \`BorrowedAccount\` methods in \`transaction-context\` (\`set\_data\`, \`set\_lamports\`, \`set\_owner\`), and after an sBPF program returns, \`deserialize\_parameters\` replays its changes through those same checks. Older validators diffed pre- and post-state; the rules are the same. These are the only guarantees you get for free: \[Certain\]

1. Only the **owner** program may change `data`.
2. Only the **owner** may *decrease* `lamports`. Any program may *increase* lamports of a writable account.
3. Total lamports across all accounts in the instruction must be conserved.
4. `owner` can only be reassigned by the current owner, and only when the data is all zeros (or empty).
5. Read-only accounts cannot change at all (lamports, data, owner).
6. `executable` accounts are immutable to everyone but their loader.
7. Data length can only grow by `MAX_PERMITTED_DATA_INCREASE` (10 KiB) per instruction via `realloc`, and only by the owner.

What the runtime does **not** check: whether the accounts you received are the ones your logic expects. If your program reads a `Config` from an account it never verified, the runtime will happily let an attacker pass in a forged one that they own. Ownership rules protect *writes*; they do nothing for *reads*. \[Certain\]

### 1.3 Signers and writable: per-transaction flags, not account properties

`is_signer` and `is_writable` are not stored in the account. They come from the transaction message header, which partitions `account_keys` into four ranges: signed+writable, signed+readonly, unsigned+writable, unsigned+readonly. \[Certain\]

- **Signer** means a valid ed25519 signature over the message exists for that pubkey (verified before execution), or, inside a CPI, that a PDA's seeds were supplied via `invoke_signed`, or the caller passed through a signer it already had.
- **Writable** is a *request for a write lock*. The scheduler uses it to run non-conflicting transactions in parallel (Sealevel). Writable does not mean anyone may write; ownership rules still apply.
- Writable is also demoted by the runtime for some keys: reserved program IDs, sysvars, and a program ID that is invoked in the transaction. \[Likely\]

Auditor reflex: for every account in an instruction ask three questions. *Who owns it? Must it sign? Must it be writable?* Then ask the fourth, which is where bugs live: *what proves this is the specific account I think it is?*

### 1.4 Rent

Storage costs lamports. Since rent collection was disabled for new accounts, every new account must be **rent-exempt**: hold at least `Rent::minimum_balance(data_len)` lamports, roughly 0.00089 SOL base + \~0.00000696 SOL per byte at historical parameters. \[Likely; check `Rent::default()` and any SIMD changing rent price\]

Security-relevant consequences:

- **Closing an account** = moving all its lamports out. At the end of the transaction, an account with 0 lamports is garbage-collected. *Within* the same transaction it still exists with its old data, which is the root of the revival attack (part 6). \[Certain\]
- An account that would drop below rent-exempt without going to zero fails the transaction (`InsufficientFundsForRent`). Programs that do partial withdrawals of lamports from PDAs must keep the minimum. \[Certain\]
- Anyone can send lamports to any account. Never use an account's lamport balance as an accounting invariant (e.g. "vault lamports == total deposits"); it can be inflated by a donation. \[Certain\]

### 1.5 Program Derived Addresses (PDAs)

A PDA is an address derived as `sha256(seeds || [bump] || program_id || "ProgramDerivedAddress")`, rejected if it lies on the ed25519 curve. Being off-curve means **no private key exists**, so only the deriving program can sign for it (via `invoke_signed`). \[Certain\]

- `find_program_address` tries bump 255 downward and returns the first off-curve result: the **canonical bump**. `create_program_address` takes a bump you supply and does not check canonicality.
- Up to 16 seeds, each ≤ 32 bytes. \[Certain\]
- A PDA is a *naming scheme*, not a type. Holding the address proves nothing about the data inside; you still check owner and discriminator, unless you created it yourself at that address.

What PDAs are used for, and the matching bug:

| Use | Example seeds | Bug when done wrong |
| --- | --- | --- |
| Program-owned authority (vault signer) | `[b"vault_authority", pool.key()]` | Seeds too generic: one authority signs for every pool, so a user of pool A drains pool B |
| Deterministic per-user state | `[b"position", market.key(), user.key()]` | Missing `user` in seeds lets one user act on another's position |
| Uniqueness / replay protection | `[b"nonce", id.to_le_bytes()]` | Using non-canonical bump lets an attacker create a *second* valid PDA for the same logical key |
| Program config singleton | `[b"config"]` | Init not gated: first caller sets themselves admin (front-running init) |

**Seed collisions.** Seeds are concatenated without separators, so `["ab", "c"]` and `["a", "bc"]` derive the same address. Variable-length user-controlled seeds next to each other are a real bug class; prefix every seed set with a fixed literal and prefer fixed-length seeds (pubkeys, little-endian integers). \[Certain\]

### 1.6 Transactions, instructions, and the account list

A transaction carries: signatures, a message header, `account_keys`, a recent blockhash (or durable nonce), and a list of instructions. Each instruction names a `program_id_index` and a list of indexes into `account_keys`, plus opaque `data`. \[Certain\]

- The **same account can appear more than once** in an instruction's account list. Programs that take `from` and `to` without checking they differ get self-transfer bugs, and anything that holds two mutable references to the same data must handle aliasing. \[Certain\]
- **Address Lookup Tables** (v0 transactions) let a transaction reference up to 256 accounts by index into an on-chain table. Lookup-table accounts can never be signers. \[Certain\]
- Transaction size limit is 1232 bytes. This is why protocols push complexity into `remaining_accounts` and multi-instruction flows, which creates its own bugs. \[Certain\]
- **Instruction introspection**: the Instructions sysvar lets a program read the other instructions in its transaction. Used for flash-loan repayment checks and ed25519/secp256k1 signature verification; misused in signature-verification bugs (part 6). \[Certain\]

### 1.7 Mental model to keep

A Solana instruction is a function whose *every* argument is attacker-chosen except: the program ID being invoked, the signer flags (which are cryptographically backed), and the runtime's ownership rules. Everything else, including "this is the pool", "this is the oracle", "this is the token program", is a claim your code must verify.

## 2. Anchor: what each check does, and what it does not

Anchor turns a `#[derive(Accounts)]` struct into a `try_accounts` function that runs before your handler. Every guarantee Anchor gives you comes from (a) the **account wrapper type** and (b) the **constraint attributes**. Anything not expressed in one of those two places is unchecked. The fastest way to audit an Anchor program is to read each Accounts struct and, for every field, write down what is *proven* about it. \[Certain\]

**Version note.** Anchor 1.0 is the stable line (1.x is current as of late 2026), stewarded by OtterSec; Anchor v2 is a rewrite in release-candidate stage. Most production code you audit is still 0.29–0.32. Differences that matter for security are flagged below. \[Likely\] Source of truth: `lang/syn/src/codegen/accounts/constraints.rs` in the Anchor repo at the exact version in the target's `Cargo.lock`. Run `cargo expand` on any struct you are unsure about.

### 2.1 Account wrapper types

| Type | Checks | Does NOT check |
| --- | --- | --- |
| `Account<'info, T>` | Owner == `T::owner()` (the program declaring `T`); first 8 bytes == `T`'s discriminator; Borsh-deserializes; re-serializes on exit if `mut` | That it is the *specific* `T` you meant (any account of type `T` passes); relationships to other accounts |
| `AccountLoader<'info, T>` (zero-copy) | Owner + discriminator, lazily on `load()` / `load_mut()` | Same as above; plus `load_init` only checks discriminator is zero |
| `Signer<'info>` | `is_signer` | Ownership, type, anything about the data. A signer can be any keypair, including a PDA signed via CPI |
| `SystemAccount<'info>` | Owner == System Program | That it is unfunded, or not a signer, or not a specific wallet |
| `Program<'info, P>` | Key == `P::id()` and executable | Nothing else needed: this is the fix for arbitrary CPI |
| `Interface<'info, I>` | Key ∈ allowed set (e.g. Token or Token-2022) | Which of the set it is; your code must handle both |
| `InterfaceAccount<'info, T>` | Owner ∈ allowed programs; deserializes `T` | Extensions (Token-2022) unless you check them |
| `Sysvar<'info, S>` | Key == sysvar ID | Prefer `S::get()` syscalls; passing sysvars as accounts is legacy |
| `UncheckedAccount` / `AccountInfo` | **Nothing.** Requires a `/// CHECK:` doc comment | Everything. Read every `/// CHECK` comment and try to falsify it |
| `Box<Account<..>>` | Same as `Account`; heap-allocated to save stack | — |
| `Option<Account<..>>` | Passing the program ID as the account means `None` | Callers can opt out of the account. Check logic that treats `None` as "skip a check" |

Key insight: `Account<T>` proves *"some account of type T owned by this program"*. It does not prove *"the pool this user's position belongs to"*. That relationship needs `has_one`, `seeds`, `address` or `constraint`. Missing relationship constraints are the single most common Anchor finding. \[Likely\]

### 2.2 Constraints, one by one

| Constraint | What it checks | What it does NOT check / gotchas |
| --- | --- | --- |
| `mut` | `is_writable`; marks for serialize-on-exit | Ownership (comes from the type). Without `mut`, changes to an `Account<T>` are silently dropped |
| `dup` (1.x) | Opts in to the same account appearing in two `mut` slots | In 1.x duplicates of serializing `mut` accounts are rejected by default. **In 0.x there is no such check**: two `mut` fields can alias and the later `exit()` overwrites the earlier one |
| `signer` | `is_signer` | Same as `Signer` type |
| `has_one = x` | `self.x == accounts.x.key()` (field name must match) | Only equality of keys. If `x` is itself unchecked, you proved nothing. Classic: `has_one = authority` where `authority` is not a `Signer` |
| `address = expr` | `key == expr` | Correct choice for known singletons (oracles, programs) |
| `owner = expr` | `owner == expr` | Cannot override typed owner checks. On `UncheckedAccount` it is the only check you get |
| `executable` | Account is a program | Which program |
| `seeds = [..], bump` | Recomputes PDA from `program_id` + seeds and compares key. `bump` alone: `find_program_address` (canonical, costs CU); `bump = x`: uses `create_program_address` with stored bump | That the PDA is initialized, unless the type is `Account`. With `bump = stored_bump`, canonicality depends on how it was stored at init |
| `seeds::program = p` | Derives with another program's ID | That `p` is the right program, if `p` is user-supplied |
| `init` | CPI to System `create_account` with `payer`, `space`, owner = this program; writes discriminator. Also verifies seeds if given | Fails if the account already has lamports (prior to fix, attacker could pre-fund an address so `create_account` fails = DoS; Anchor now uses transfer+allocate+assign path when pre-funded) \[Likely\] |
| `init_if_needed` | Like `init` if account is uninitialized; otherwise validates as the declared type and runs other constraints | **Reinitialization risk:** your handler runs on an *existing* account too. Any handler logic that "sets initial state" will reset it. Feature-gated precisely because it is dangerous |
| `zero` | Discriminator is all zeros; then writes discriminator | That the account is rent-exempt or of correct size (check `space`); used for >10 KiB accounts created client-side |
| `close = target` | On exit: move all lamports to `target`, zero data, assign to System Program (since 0.30 / 1.x behaviour; older versions wrote `CLOSED_ACCOUNT_DISCRIMINATOR`) | That `target` is the right recipient (add `has_one`). Revival within the same tx is mitigated by reassigning owner, but check custom manual closes |
| `realloc = n, realloc::payer, realloc::zero` | Resizes, pays or refunds rent difference | `realloc::zero = false` leaves stale bytes when growing after a shrink in the same tx. Shrinking refunds **all** lamports above the new minimum, including intentional vault balances |
| `constraint = expr` | Arbitrary boolean | Only what you wrote. Read the expression carefully: `>=` vs `>`, wrong field |
| `token::mint`, `token::authority`, `token::token_program` | Deserialized token account's `mint` and `owner` fields match | Delegate, close authority, frozen state, Token-2022 extensions |
| `mint::authority`, `mint::decimals`, `mint::freeze_authority` | Mint fields match | Supply, extensions (transfer fee, hooks, permanent delegate) |
| `associated_token::mint/authority` | Key == ATA derived for (authority, mint, token\_program) | Nothing more; an ATA's address is also its validation |
| `extensions::*` (Token-2022) | Sets or validates the named mint extension | Every extension it does not name |
| `discriminator = ..` | Custom discriminator (1.x) | All-zero discriminators rejected in 1.x; in 0.x a zeroed discriminator could be hijacked via IDL instructions |

### 2.3 What Anchor never checks for you

1. **Relationships** between accounts (that this vault belongs to this pool). You must add `has_one` / `seeds` / `constraint`.
2. **`remaining_accounts`**. Raw `AccountInfo`s with zero validation (part 6).
3. **Accounts you load manually** inside the handler via `Account::try_from` or raw deserialization: only what that call checks.
4. **State after a CPI.** `Account<T>` is deserialized once. After a CPI that modifies it (e.g. a token transfer), the in-memory copy is stale until you call `.reload()`. Stale-data bugs in AMMs and vaults come from this. \[Certain\]
5. **Arithmetic.** Overflow checks only if `overflow-checks = true` in the release profile (Anchor templates set it; verify `Cargo.toml`). Rounding direction, never. \[Certain\]
6. **Order of instructions in the transaction**, flash-loan style composition, and whether the same instruction appears twice.
7. **`/// CHECK:` comments are documentation, not code.** The compiler only checks the comment exists.
8. **Constraint evaluation order.** Constraints run in field order, so a `constraint` that reads a field validated later in the struct sees unvalidated data. \[Likely\]

### 2.4 Exercise

Take the struct below and write, for each field, what is proven. Then find the bug.

```rust
#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut, has_one = vault)]
    pub pool: Account<'info, Pool>,
    #[account(mut)]
    pub vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_token: Account<'info, TokenAccount>,
    pub authority: AccountInfo<'info>, // CHECK: pool authority
    pub owner: Signer<'info>,
    pub token_program: Program<'info, Token>,
}
```

Answer: nothing ties `owner` to anything (anyone can withdraw), `authority` is never derived (if it is used as the CPI signer via `invoke_signed` with pool seeds that are generic, other pools' vaults can be drained), and `user_token.mint` is never checked against the vault mint.

## 3. The core programs

These five programs are the trusted base every protocol builds on. Know their invariants cold: when a protocol bug exists, it is usually because the author assumed one of these programs checks something it does not. Read the source, not the docs: they are short. \[Certain\]

### 3.1 System Program (`11111111111111111111111111111111`)

Owns every wallet and every uninitialized account. Instructions that matter: `CreateAccount`, `Transfer`, `Allocate`, `Assign`, `CreateAccountWithSeed` and nonce-account instructions. \[Certain\]

| Instruction | Requires | Security notes |
| --- | --- | --- |
| `CreateAccount` | `from` signs, `to` signs (keypair or PDA via `invoke_signed`), `to` has 0 lamports and no data | Fails if `to` already holds lamports: the pre-funding DoS. Robust code does `transfer` (shortfall) + `allocate` + `assign` instead |
| `Transfer` | `from` signs, `from` owned by System, `from` has **no data** | You cannot `system_program::transfer` out of a PDA that holds data; the owning program must debit lamports directly |
| `Allocate` / `Assign` | Account signs, owned by System | `Assign` is how ownership moves to your program; once moved, only your program can move it back (and only with zeroed data) |

Why you care: any account a user passes that is owned by the System Program is "just a wallet" and carries no data guarantees. A program that expects an initialized PDA but accepts a System-owned account is reading zeros or nothing.

Source: `solana-program/system` repo (`program/src/`), and the builtin in Agave under `programs/system/src/system_processor.rs`. \[Likely\]

### 3.2 SPL Token (`TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA`)

**2026 change:** the program at this ID is now **p-token**, a Pinocchio rewrite swapped in by feature gate (SIMD-0266); layouts and instruction semantics are unchanged, it adds `batch`, `withdraw_excess_lamports` and `unwrap_lamports`, and it no longer logs instruction names. \[Certain\] Audit against `solana-program/token` (`p-token/` and `program/`).

Two account types:

- **Mint** (82 bytes): `mint_authority: COption<Pubkey>`, `supply`, `decimals`, `is_initialized`, `freeze_authority`.
- **Token account** (165 bytes): `mint`, `owner`, `amount`, `delegate`, `state` (Uninitialized/Initialized/Frozen), `is_native`, `delegated_amount`, `close_authority`.

What `Transfer` checks: source and destination are initialized token accounts owned by the Token program; same `mint`; neither frozen; authority signed and is either `owner` or `delegate` (with sufficient `delegated_amount`); sufficient balance. Multisig owners are supported. \[Certain\]

What it does **not** check, and you must:

- That the destination is the account *your* protocol intended (e.g. the pool vault, not an attacker's account of the same mint).
- That `source != destination` is meaningful for your accounting (self-transfer succeeds and changes nothing).
- Decimals: `Transfer` ignores them; `TransferChecked` takes `mint` and `decimals` and is the only form that works with Token-2022 extensions.
- That an account's `owner` field (the token-level owner) relates to anything in your program. **`AccountInfo.owner` (= Token program) and `TokenAccount.owner` (= wallet or PDA) are different things**; confusing them is a classic bug.
- `CloseAccount` requires zero balance (except native SOL), and sends lamports where the close authority says. A protocol that lets users pass a token account it later expects to exist can be griefed by closing it.

Other invariants worth memorizing: `InitializeAccount` (v1) requires the Rent sysvar account, v2/v3 do not; `SetAuthority` with `None` on mint authority makes supply fixed forever; native mint (wrapped SOL) accounts track lamports via `SyncNative`, so `amount` can lag lamports until synced.

### 3.3 Associated Token Account program (`ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL`)

The ATA is a PDA of the ATA program with seeds `[wallet, token_program_id, mint]`. `Create` makes it; `CreateIdempotent` succeeds if it exists. \[Certain\]

- The address *is* the validation: if you derive the expected ATA and compare keys, you know mint, owner and token program.
- But **users can own non-ATA token accounts** and **an ATA's owner can be changed** via `SetAuthority(AccountOwner)` on legacy Token (blocked by the ATA program's "immutable owner" extension only on Token-2022). Never infer the current owner from the ATA address on legacy Token; read the `owner` field. \[Likely\]
- `token_program_id` is part of the seeds, so the Token and Token-2022 ATAs for the same (wallet, mint) are different addresses.

### 3.4 Token-2022 / Token Extensions (`TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb`)

A superset of Token with a TLV extension area after the base 165/82 bytes (padded so a token account's `AccountType` byte sits at offset 165). Protocols that accept "any SPL token" via `Interface<TokenInterface>` inherit every extension's semantics. This is where many 2024–2026 findings live. \[Likely\]

| Extension | What it changes | Protocol bug it causes |
| --- | --- | --- |
| Transfer fee | Recipient receives `amount - fee` | Vault credits `amount` but receives less: insolvency. Always measure balance before/after |
| Transfer hook | Every `TransferChecked` CPIs into an arbitrary program with extra accounts | Reentrancy-like callbacks, unexpected CU use, DoS, and extra accounts the protocol must pass through |
| Permanent delegate | A fixed authority can transfer or burn from *any* account of the mint | Collateral can be seized from your vault at any time |
| Confidential transfers | Balances move in encrypted form | `amount` field doesn't reflect total; accounting via `amount` is wrong |
| Non-transferable | Tokens cannot move | Liquidations/withdrawals revert: stuck positions |
| Default account state = Frozen | New accounts start frozen | Protocol-created vaults unusable unless thawed |
| Mint close authority | Mint can be closed at zero supply and re-created at same address with different params | Cached decimals/extension assumptions go stale |
| Interest-bearing | UI amount ≠ raw amount | Only cosmetic on-chain, but oracle/UI mismatch |
| Memo required / CPI guard | Incoming transfers need a memo; CPI guard blocks certain actions via CPI | Protocol transfers revert: DoS |
| Pausable, scaled UI amount | Mint can pause all transfers | Same DoS class |

Rule of thumb for auditing: list every extension, then decide for each whether the protocol (a) rejects mints with it, (b) handles it, or (c) silently breaks. Most protocols allowlist extensions; check the allowlist is enforced on every path that accepts a mint, including `init_if_needed`.

### 3.5 The upgradeable BPF loader (loader-v3, `BPFLoaderUpgradeab1e11111111111111111111111`)

Still the default for new deployments in late 2026; loader-v4 exists and sBPFv3 deployment requirements are scheduled with Agave 4.4. \[Likely\]

A deployed program is **three accounts**: \[Certain\]

1. **Program** account: executable, owned by the loader, data = `UpgradeableLoaderState::Program { programdata_address }`. Its address is the program ID. Immutable.
2. **ProgramData** account: PDA of the loader with seed = program ID; data = `ProgramData { slot, upgrade_authority_address: Option<Pubkey> }` followed by the ELF.
3. **Buffer** account (transient): where bytecode is written in chunks before `DeployWithMaxDataLen` / `Upgrade`.

Instructions: `InitializeBuffer`, `Write`, `DeployWithMaxDataLen`, `Upgrade`, `SetAuthority` / `SetAuthorityChecked`, `Close`, `ExtendProgram` (now permissionless-by-authority variants exist). \[Likely\]

Security points:

- **Upgrade authority = total control** of the program and all PDAs it owns. In audits, record who holds it (multisig? timelock?) and treat it as a trust assumption. `None` means immutable.
- **Programs verifying "admin = upgrade authority"** must load the ProgramData account, check its address is the loader-PDA of `program_id`, and read the authority. Anchor's pattern: `#[account(constraint = program.programdata_address()? == Some(program_data.key()))]` plus `program_data.upgrade_authority_address == Some(signer.key())`. Missing the address check lets an attacker pass a fake ProgramData. \[Certain\]
- **Same-slot visibility:** a program upgraded in slot N becomes executable with new code in slot N+1 (deployment cooldown / "delay visibility"). \[Likely\]
- **Buffer accounts** have their own authority; a leaked buffer authority lets an attacker swap the bytecode before deploy. Verify buffers with `solana-verify` and verifiable builds.
- **Closing a program** is permanent for that address; its program ID can never be redeployed.

Source: Agave `programs/bpf_loader/src/lib.rs` (the processor for all BPF loaders, including execution entry; see part 7) and `programs/loader-v4/src/lib.rs`.

## 4. CPIs and how privileges flow

A cross-program invocation (CPI) is one program calling another via the `sol_invoke_signed_rust` / `_c` syscall. The rule that governs all CPI security: **a callee can never receive a privilege its caller did not have, except signatures for the caller's own PDAs.** Everything else follows from that. \[Certain\]

### 4.1 The two calls

```rust
invoke(&ix, &account_infos)?;                    // pass through existing privileges
invoke_signed(&ix, &account_infos, &[seeds])?;   // + sign for PDAs of *this* program
```

For each `AccountMeta` in `ix`, the runtime checks (in `program-runtime`'s `prepare_next_instruction` and the CPI syscall code): \[Certain\]

1. The account is present in the caller's instruction context (you cannot conjure accounts).
2. If `is_writable` is requested, the caller had it writable.
3. If `is_signer` is requested, either the caller had it as a signer, **or** it equals `create_program_address(seeds, caller_program_id)` for one of the provided seed sets.
4. The callee program account is present and executable.

The `program_id` inside `ix` is whatever the caller put there. The runtime does not care if it is the program you *meant*.

### 4.2 Privilege flow, concretely

Consider: user signs tx → Vault program → CPI to Token `Transfer` from `vault_ata` (authority = vault PDA) to `user_ata`.

- User's signature reaches Vault as `is_signer` on the user account.
- Vault calls `invoke_signed` with seeds `[b"vault", pool.key(), &[bump]]`; the runtime derives the PDA from **Vault's** program ID and grants `is_signer` to that PDA in the callee.
- Token sees the vault PDA as signer and authority of `vault_ata`, and transfers.

If the vault PDA's seeds don't include the pool, any pool's vault is signable by any instruction path that reaches `invoke_signed`. That is the generic-seeds bug from part 1.

### 4.3 Data synchronization across a CPI

The caller and callee see the same account through serialized buffers. Around a CPI the runtime: \[Certain\]

1. **Before:** copies the caller's current `lamports`, `data` (and length) and `owner` for each passed account into the runtime's view (`update_callee_account`). Caller-side writes made before the CPI therefore become visible to the callee.
2. **After:** copies the callee's modifications back into the caller's `AccountInfo` (`update_caller_account`), including data length changes from `realloc`.

Consequences:

- Rust structs deserialized *before* the CPI (Anchor `Account<T>`, or your own `T::try_from_slice`) are **stale** after it. Re-read (`.reload()`) before using balances again. \[Certain\]
- With Anchor, mutated `Account<T>` fields are only serialized back on `exit()` at the end of the handler. If you modify `pool.reserve` and then CPI into a program that reads `pool`, it sees the *old* bytes. Call `pool.exit()` or write manually first. \[Likely\]
- Since direct-mapping work (account data mapped into the VM rather than copied), some realloc and pointer edge cases changed; verify behaviour for the feature set active on the cluster. \[Guessing\]

### 4.4 Limits

| Limit | Value | Why it matters |
| --- | --- | --- |
| CPI depth | Historically 4 nested CPIs below the top-level instruction; Agave has a \`raise\_cpi\_nesting\_limit\_to\_8\` feature (SIMD-0268) that raises nesting to 8. Exceeding it returns \`CallDepth\` | Deep composition (aggregator → AMM → Token-2022 → transfer hook) can hit the limit and revert \[Likely; verify current feature status\] |
| Reentrancy | Direct self-recursion (A→A) allowed; indirect (A→B→A) rejected with `ReentrancyNotAllowed` | Solana's built-in reentrancy guard. But it does *not* stop cross-instruction composition in one tx (part 6) \[Certain\] |
| Accounts per CPI | \`MAX\_CPI\_ACCOUNT\_INFOS\` = 255; at most 16 signer seed sets (\`MAX\_SIGNERS\`), each ≤ 16 seeds | Large batched CPIs fail \[Likely\] |
| Return data | 1024 bytes via `set_return_data` / `get_return_data`, tagged with the setting program ID | Always check the program ID returned matches the callee you expected \[Certain\] |
| Compute | Callee consumes from the same transaction budget | A hostile callee (e.g. a transfer hook) can burn CU to DoS your instruction |

### 4.5 The CPI bug classes

1. **Arbitrary CPI.** Program invokes `token_program.key()` without checking it equals the Token program ID. Attacker passes their own program, which "succeeds" without moving tokens, or receives the forwarded signer. Fix: `Program<'info, Token>` / `Interface`, or an explicit `require_keys_eq!`. \[Certain\]
2. **Signer forwarding to untrusted programs.** Any CPI into a user-specified program forwards every signer and writable account you pass. If you pass the user's signer, the attacker's program can use it to call *other* programs on the user's behalf. If you pass a PDA with `invoke_signed`, it gets your PDA's authority. Only ever `invoke_signed` into programs whose ID you have pinned. \[Certain\]
3. **Seed confusion.** One PDA used as authority for many resources (all pools, all users) means one compromised path signs for everything. Scope authorities by the resource they protect.
4. **Stale state after CPI.** Covered above. Look for balance reads after transfers without reload, especially in swap and deposit math.
5. **Callee-controlled accounts.** After a CPI to an external program (AMM, oracle, lending protocol), any writable account you passed may have changed arbitrarily *within that program's ownership rules*. Re-validate invariants post-call.
6. **Lamport ownership surprise.** A callee can credit lamports to any writable account; your post-CPI checks that compare lamports must account for that.
7. **Token-2022 CPI Guard.** If the user enabled CPI Guard on their token account, certain transfers/approvals via CPI fail. Protocols that assume they can always pull funds via delegate in a CPI get reverts.
8. **Return-data spoofing.** Reading `get_return_data()` without checking the returned program ID lets a nested program's return data be mistaken for the expected callee's.

## 5. Reading one AMM, one lending market, one perp DEX

Pick these three: **Raydium CP-Swap** (small, Anchor, Token-2022 aware), **Kamino Lend** (the largest Solana lending market, Anchor, flash loans via introspection), and **Drift v2** (the most complex open-source perp DEX on Solana, heavy `remaining_accounts` use). Together they cover nearly every pattern in parts 1–4 and 6. Directory layouts below were checked against each repo's default branch on 2026-11-26. \[Certain for paths listed; Likely for file roles\]

### 5.0 The reading method (use it on all three)

Do not read top-down. For each protocol, produce these five artifacts, in order:

1. **Account map.** Every `#[account]` struct: fields, seeds, who creates it, who can mutate it, size. Draw which accounts point to which (`pool.token_0_vault`, `obligation.lending_market`, `user.authority`).
2. **Instruction table.** Every instruction in `lib.rs`: signer, writable accounts, which state it changes, permissioned or not.
3. **Value flows.** For every instruction that moves tokens: from where, to where, who signs, what amount formula, rounding direction.
4. **Invariants.** Write down 5–10 statements that must always hold ("sum of user collateral ≤ vault balance", "`k` never decreases on swap", "a user with health < 1 can be liquidated, one with health ≥ 1 cannot"). Then try to break each.
5. **Trust assumptions.** Admin powers, oracles, upgrade authority, keepers, external programs CPI'd into.

Then apply every bug class from part 6 to every instruction. Keep notes in a single file per protocol; these become your reusable threat models.

### 5.1 AMM: Raydium CP-Swap

Repo: [raydium-io/raydium-cp-swap](https://github.com/raydium-io/raydium-cp-swap). Constant-product (x·y=k) AMM, no CLMM complexity, supports Token-2022 mints. About a week of reading. \[Likely\]

| Path (under `programs/cp-swap/src/`) | What to extract |
| --- | --- |
| `lib.rs` | Instruction list and dispatch |
| `states/` | `PoolState` and `AmmConfig` layouts; which vaults, mints, LP mint, fee accumulators the pool records |
| `instructions/initialize.rs`, `initialize_v2.rs`, `initialize_with_permission.rs` | How pools are created; PDA seeds for pool, vaults, LP mint, authority; who may create; initial liquidity and LP mint math (look for minimum-liquidity lock) |
| `instructions/swap_base_input.rs`, `swap_base_output.rs` | Full swap path: account validation, transfer-fee handling, curve call, fee split, vault transfers, reserve update |
| `instructions/deposit.rs`, `withdraw.rs` | LP mint/burn proportional math; rounding direction for each side |
| `instructions/collect_creator_fee*.rs`, `instructions/admin/` | Fee withdrawal authority; what admin can change on a live pool |
| `curve/` | Pure math: constant product, fee calculator, rounding helpers. Fuzz it |
| `utils/` | Token transfer helpers (Token vs Token-2022), transfer-fee calculations |

What to trace:

- **Reserves vs vault balances.** Does the pool use vault `amount` minus accumulated fees as reserves? Can a donation to the vault move the price? Can fees be counted as liquidity?
- **Transfer-fee mints.** On input, the vault receives less than `amount_in`; on output, the user receives less than computed. Check both sides use the *post-fee* amount in the invariant.
- **Rounding.** Every division must round in the pool's favour: down for amounts out, up for amounts in and LP burned. Write a 2-line test for each.
- **First depositor / inflation attack** on LP shares.
- **Vault and mint validation**: is `input_vault` tied to the pool by `has_one`/`address`? Is `input_token_mint` checked against the vault? Could input and output vault be the same account?
- **Oracle/TWAP** fields: who updates, can one tx manipulate them?

### 5.2 Lending: Kamino Lend (klend)

Repo: [Kamino-Finance/klend](https://github.com/Kamino-Finance/klend). Reserves (one per asset), obligations (one per user per market), elevation groups (e-mode), flash loans, farms, withdrawal tickets/queues, and newer fixed-term borrow orders. Expect 2–3 weeks. \[Likely\]

| Path (under `programs/klend/src/`) | What to extract |
| --- | --- |
| `lib.rs` | Instruction list; note the volume (≈55 handlers) and group them |
| `state/` | `LendingMarket`, `Reserve` (liquidity, collateral, config, last-update), `Obligation` (deposits, borrows, cached values) |
| `lending_market/lending_operations.rs` | The core math: refresh reserve (interest accrual), refresh obligation (valuation), borrow, repay, deposit, withdraw, liquidate. The heart of the protocol |
| `lending_market/lending_checks.rs` | Validation shared by handlers: staleness, reserve-obligation relationships |
| `lending_market/flash_ixs.rs` + `handlers/handler_flash_borrow_reserve_liquidity.rs`, `handler_flash_repay_reserve_liquidity.rs` | Flash loans via the Instructions sysvar: how borrow finds its matching repay, how CPI-invocation is blocked, how multiple borrows are prevented |
| `lending_market/withdrawal_cap_operations.rs` | Rate limits on outflows |
| `handlers/handler_refresh_reserve.rs`, `handler_refresh_obligation.rs` | Oracle reads (Pyth/Switchboard/Scope), staleness and confidence checks; which obligation reserves must be passed in `remaining_accounts` and in what order |
| `handlers/handler_liquidate_obligation_and_redeem_reserve_collateral.rs` | Liquidation: close factor, bonus, bad-debt path, `handler_socialize_loss.rs` |
| `utils/` | Fraction/fixed-point math, token transfer helpers |

What to trace:

- **The refresh pattern.** Most handlers require reserves and the obligation to have been refreshed *in the same slot* (often the same tx). Find exactly where staleness is checked and whether any path skips it.
- **Exchange rate (cToken).** How collateral tokens convert to liquidity; rounding on deposit vs redeem; can the rate be pushed by donation or by interest accrual edge cases?
- **Flash loans.** Read the introspection logic line by line: does it check the repay ix's program ID, the reserve, the amount, that it's not inside a CPI, and that there's exactly one borrow per repay? This is the canonical example of the instruction-introspection bug class.
- **Obligation accounting.** Deposits and borrows are arrays in the obligation. Look for index confusion, duplicates, and how `remaining_accounts` must align with them.
- **Elevation groups.** Changing group mid-life: can a user switch to better LTV with incompatible positions?
- **Oracle handling.** Price confidence, stale price, TWAP divergence checks, what happens when an oracle is down.

### 5.3 Perp DEX: Drift v2

Repo: [drift-labs/protocol-v2](https://github.com/drift-labs/protocol-v2). Cross-margined perps and spot, an AMM backstop, order book via keepers (JIT/auctions), insurance fund, isolated positions, LP pool. Large. Expect 3–4 weeks; don't try to read all of it. \[Likely\]

| Path (under `programs/drift/src/`) | What to extract |
| --- | --- |
| `lib.rs` | Instruction entry points; Drift keeps handlers thin |
| `instructions/user.rs` | User-facing instructions: deposit, withdraw, place/cancel orders, settle PnL |
| `instructions/keeper.rs` | Permissionless keeper instructions: fill, liquidate, settle funding, update AMMs. Highest-risk surface: anyone can call these with any accounts |
| `instructions/admin.rs`, `lp_admin.rs` | Admin powers; record each as a trust assumption |
| `instructions/optional_accounts.rs` | How `remaining_accounts` are parsed into oracle, spot-market and perp-market maps. **Read this first**: it is a textbook of `remaining_accounts` validation |
| `instructions/constraints.rs` | Reusable Anchor constraint functions |
| `controller/orders.rs` (+ `orders/`) | Order matching, fill logic, auctions. The longest and most bug-dense area |
| `controller/liquidation.rs` | Liquidation of perp, spot, borrow, PnL; partial liquidation, bankruptcy |
| `controller/pnl.rs`, `funding.rs`, `amm.rs`, `repeg.rs` | PnL settlement against the PnL pool, funding rate accrual, AMM peg adjustments |
| `controller/spot_balance.rs`, `spot_position.rs` | Interest-bearing spot balances (scaled balance accounting) |
| `controller/insurance.rs` | Insurance fund staking, revenue settlement |
| `math/` | Margin calculation, AMM math, safe arithmetic. Margin calc is where solvency lives |
| `state/` | `User`, `PerpMarket`, `SpotMarket`, `State`, oracle types and the market/oracle maps |
| `validation/` | Post-operation invariant checks (margin, positions). Note what is validated and what isn't |

What to trace:

- **Account maps from `remaining_accounts`.** How does Drift ensure that the oracle passed for market X is market X's oracle, that every market the user has a position in is included in margin calculation, and that no account is passed twice? Missing a market in the map would make margin calc ignore a losing position.
- **Margin calculation.** Initial vs maintenance margin, weight of unrealized PnL, oracle price vs mark price, how isolated positions are separated.
- **Keeper incentives and permissionless calls.** Can a keeper fill at a bad price, self-trade, or liquidate a healthy user by manipulating an input it controls?
- **Oracle validity.** Drift has explicit oracle validity states (stale, too volatile, insufficient data); map which actions are allowed in which state.
- **Funding and PnL pool solvency.** Can settled PnL exceed what the pool holds? What happens then?

### 5.4 Deliverable per protocol

One page: account diagram, the invariant list, the top 5 places you'd look for bugs, and every trust assumption. If you can't write it, you haven't finished reading.

## 6. The bug classes

Every class below reduces to the part-1 rule: *an account or input was trusted without proof*. For each, the table gives the check that's missing and the fix. Then work through the [sealevel-attacks](https://github.com/coral-xyz/sealevel-attacks) repo: each of its 11 folders has `insecure`, `secure` and `recommended` versions. Write an exploit test for every `insecure` one before reading the fix. \[Certain\]

### 6.1 The sealevel-attacks set

| # | Class | Missing check | Fix (Anchor) | Real-world shape |
| --- | --- | --- | --- | --- |
| 0 | Signer authorization | Authority never required to sign | `Signer` + `has_one = authority` | Admin instruction callable by anyone |
| 1 | Account data matching | Account's fields not tied to others (token account's owner/mint) | `has_one`, `token::authority`, `constraint` | Withdraw to an account whose `owner` isn't the user |
| 2 | Owner checks | `AccountInfo.owner` not verified; attacker forges data | `Account<T>` | Fake config/oracle account with attacker-chosen bytes |
| 3 | Type cosplay | Two account types same layout; no discriminator check | Anchor discriminators | Pass a `User` where a `Admin` is expected |
| 4 | Initialization | Re-init an existing account, overwriting authority | `init` (not `init_if_needed`), `is_initialized` flags | Attacker re-initializes a vault and becomes its authority |
| 5 | Arbitrary CPI | Callee program ID not checked | `Program<'info, Token>` | Fake token program "transfers" nothing |
| 6 | Duplicate mutable accounts | Same account passed twice for two roles | Key inequality constraint; 1.x default rejection | `from == to` double-counts rewards |
| 7 | Bump seed canonicalization | Non-canonical bump accepted from user | `bump` (canonical) or stored canonical bump | Two "unique" PDAs for one logical key |
| 8 | PDA sharing | One PDA authority across many users/pools | Seeds include the resource owner | User A's instruction signs for pool B's vault |
| 9 | Closing accounts | Lamports drained but data/owner left; revived in same tx | `close = target` | Reclaim rent, re-fund in the same tx, reuse a "closed" voucher |
| 10 | Sysvar address checking | Sysvar account passed but not key-checked | `Sysvar<'info, T>`, `address = sysvar::instructions::ID` | Fake Instructions sysvar: the Wormhole exploit (Feb 2022, \~120k ETH) |

### 6.2 Account lifecycle bugs

The lifecycle is: *address chosen → funded → allocated → assigned → initialized → used → (realloc) → closed → garbage-collected at tx end*. Bugs live in every transition. \[Certain\]

- **Pre-funding DoS.** An attacker sends lamports to a PDA your program will later `create_account`; the System Program rejects creation of a funded account. Fix: transfer + allocate + assign path (Anchor `init` handles it).
- **Front-running init.** A singleton (`[b"config"]`) with permissionless init: the first caller sets the admin. Fix: gate on upgrade authority (see 3.5) or deploy+init atomically.
- **Reinitialization.** `init_if_needed` or a hand-rolled init without an `is_initialized` flag lets a second call reset state (authority, balances, counters).
- **Revival.** After a manual close (lamports moved out), the account still exists with its data until the tx ends. A later instruction in the same tx can re-fund it to rent-exempt and it survives with old data. Fix: zero the data *and* assign to System Program, which Anchor's `close` does; check any manual close.
- **Closing with outstanding obligations.** Closing a position/obligation with non-zero debt, or a token account with balance that the protocol still counts.
- **Stale realloc data.** Growing an account after shrinking in the same tx exposes old bytes unless zeroed (`realloc::zero = true`). Shrinking refunds lamports, possibly vault funds (see 2.2).
- **Account resurrection by address reuse.** Closing a PDA and later re-creating it at the same address with fresh state can reset counters/nonces that were meant to be monotonic. Use a separate nonce or never close replay-protection accounts.
- **Rent-exemption breaks.** Withdrawing lamports from a program-owned account to below rent-exempt minimum (but above 0) fails the tx: DoS of withdrawals.

### 6.3 `remaining_accounts`

`ctx.remaining_accounts` is a slice of raw `AccountInfo`s passed after the declared accounts. Anchor validates **nothing** about them. They are used for variable-length inputs: market maps, oracle lists, route hops, obligation reserves, multisig signers. \[Certain\]

Checklist for every use:

1. **Owner** checked before deserializing? (Or `Account::try_from`, which checks owner + discriminator.)
2. **Identity**: is each account the *specific* one expected (key compared to a stored pubkey, or PDA re-derived)? Being *a* `PerpMarket` is not enough.
3. **Completeness**: if the program must see *all* of a user's positions (margin calc), what stops the caller from omitting one? Drift-style fix: iterate the user's positions and require each market to be present in the map.
4. **Duplicates**: same account passed twice, counted twice (double collateral).
5. **Ordering/alignment**: when accounts are matched to array entries by index (`obligation.deposits[i]` ↔ `remaining_accounts[i]`), is the alignment verified by key?
6. **Writability**: writable accounts from `remaining_accounts` are not auto-serialized by Anchor; changes to deserialized copies are lost unless written back.
7. **Signer lists**: multisig-style signer counts must deduplicate keys.
8. **CPI forwarding**: passing `remaining_accounts` wholesale into a CPI forwards whatever privileges they carry.

### 6.4 Signature verification

Solana programs don't verify signatures in-program (too expensive). Instead, the **Ed25519** and **Secp256k1** (and secp256r1) precompile programs verify signatures as separate instructions in the same transaction; the transaction fails if any signature is invalid. Your program then uses the Instructions sysvar to *find* that verification instruction and check it covered the right data. Everything about this is easy to get wrong. \[Certain\]

What a correct check verifies:

1. The Instructions sysvar account is the real one (`address = sysvar::instructions::ID`). Wormhole's bug: it used a deprecated loader function that didn't check the sysvar's address.
2. The instruction at the index you read has `program_id == ed25519_program::ID` (or secp256k1).
3. The precompile's **offsets struct** points into *that same instruction's* data. Each offset has an `instruction_index` field; `u16::MAX` means "this instruction". If the attacker sets it to another instruction's index, the precompile verifies a signature over bytes in *a different instruction*, while your program parses the pubkey/message from this one.
4. The **public key** in the verified data equals the expected signer (trusted oracle/guardian), and the **message** equals what you're authorizing (including domain separation, chain/program ID, nonce, expiry).
5. **Number of signatures** in the precompile ix is exactly what you expect; you check all of them, not just the first.
6. **Replay**: nonces or used-message PDAs so the same signed message can't be reused, and across clusters/programs.
7. **Relative position**: reading `current_index - 1` assumes ordering; check the CPI context (introspection only sees top-level instructions; a CPI'd call sees the outer transaction's instruction list).

Related introspection bugs (flash loans, "must be followed by" checks): checking only the *next* instruction instead of scanning; not checking `program_id`; not preventing the instruction from being called via CPI (where `get_stack_height()` > 1 and the sysvar describes the outer tx); allowing two borrows to share one repay.

### 6.5 Other high-frequency classes

- **Arithmetic**: overflow (check `overflow-checks`), `as` casts truncating u128→u64 silently, rounding towards the user, division before multiplication, precision loss in fixed-point.
- **Oracle**: missing staleness/confidence checks, wrong feed account (no `address`/owner check), using spot price for liquidation, Pyth pull-oracle `PriceUpdateV2` accounts not tied to a feed ID / verification level.
- **Token-2022 extensions**: see table in 3.4.
- **Compute/DoS**: unbounded loops over user-growable arrays, CU exhaustion via transfer hooks, griefing by closing accounts the protocol expects.
- **Admin/upgrade risk**: unchecked config updates, missing timelocks, admin can drain via a config value (fee = 100%).
- **Cross-instruction composition**: Solana has no global reentrancy problem, but multiple instructions in one tx can put state in an intermediate condition (refresh → manipulate → act). Ask: what if this instruction runs between two others the user controls?

## 7. The runtime: how Agave executes a program

A transaction reaches your entrypoint through four layers: the **bank** batches and locks accounts, the **SVM** (`svm/`) loads accounts and programs and runs each transaction, the **program runtime** (`program-runtime/`) manages the instruction stack and the sBPF VM, and the **transaction context** (`transaction-context/`) is the single source of truth for account state and enforces ownership rules on every write. Paths below were read on the `master` branch of [anza-xyz/agave](https://github.com/anza-xyz/agave) on 2026-11-26. \[Certain for names quoted; module boundaries move between releases, so pin a tag when you study\]

### 7.1 The crates to read, in order

| Crate / path | Role | Start with |
| --- | --- | --- |
| `runtime/src/bank.rs` | Slot state; batches txs, takes account locks, calls the SVM, commits results | `load_and_execute_transactions`, `commit_transactions` |
| `svm/src/transaction_processor.rs` | SVM entrypoint; per-tx fee/nonce validation, account loading, program cache, execution, rollback | `load_and_execute_sanitized_transactions` → `execute_loaded_transaction` |
| `svm/src/account_loader.rs` | Loads accounts, enforces loaded-data-size limits, fee-payer checks | `load_transaction`, `validate_fee_payer` |
| `svm/src/program_loader.rs` + `program-runtime/src/program_cache/` | Loads/verifies ELFs, JIT-compiles, caches by slot (handles deploy-delay visibility) | `load_program_with_pubkey` |
| `transaction-context/src/` | `TransactionContext`, instruction frames, `BorrowedAccount` with the ownership checks | `can_data_be_changed`, `set_lamports`, `set_owner`, `push` / `pop` |
| `program-runtime/src/invoke_context.rs` | Instruction stack, reentrancy and depth checks, privilege-escalation checks, builtin dispatch | `process_message`, `process_instruction`, `push`, `verify_instruction_accounts` |
| `programs/bpf_loader/src/lib.rs` | The loader builtin: dispatches loader management ixs or executes the user program | `process_instruction_inner` |
| `program-runtime/src/vm.rs` | Creates the sBPF VM, maps memory, runs JIT/interpreter, maps errors | `execute`, `create_vm`, `configure_program_regions` |
| `program-runtime/src/serialization.rs` | Builds the input buffer your entrypoint parses; writes changes back with checks | `serialize_parameters`, `deserialize_parameters` |
| `program-runtime/src/cpi.rs` + `syscalls/` | CPI syscall and all other syscalls (logging, PDA, sysvars, crypto, memory ops) | `cpi_common`, `translate_signers`, `check_authorized_program` |
| `compute-budget/`, `cost-model/`, `feature-set/` | CU limits and pricing, block cost tracking, feature gates | Grep every feature named in this section |
| [anza-xyz/sbpf](https://github.com/anza-xyz/sbpf) (separate repo) | The VM: ELF loader, verifier, interpreter, x86 JIT | `elf.rs`, `verifier.rs`, `interpreter.rs`, `jit.rs` |

### 7.2 The execution path, step by step

1. **Banking/replay.** The leader's banking stage (or replay on other validators) builds a batch and acquires **account locks**: writable accounts exclusively, read-only shared. This is where `is_writable` matters for parallelism, and why two txs writing the same pool are serialized. \[Certain\]
2. **`load_and_execute_sanitized_transactions`** processes the batch's transactions **sequentially** (later ones can depend on earlier writes in the batch). \[Certain\]
3. **Validate fee payer and nonce** (`validate_transaction_nonce_and_fee_payer`): fee payer must be a writable, System-owned account that can pay the fee and stay rent-exempt; durable nonce is advanced here. If this fails the tx is unprocessable; if it passes but later execution fails, **fees are still charged and the nonce still advanced** (`RollbackAccounts`). \[Certain\]
4. **Load accounts** (`load_transaction`): fetch every account; a tx that fails to load is `FeesOnly`. Enforces the loaded-accounts-data-size limit. \[Likely\]
5. **Load programs** (`filter_executable_program_accounts` → `replenish_program_cache`): look up each invoked program in the per-batch cache; load, verify and JIT the ELF if missing. Entries can be `Loaded`, `Builtin`, `FailedVerification`, `Closed` or `DelayVisibility`; the last three make the program "not deployed". This is the deploy-cooldown mechanism. \[Certain\]
6. **`execute_loaded_transaction`**: builds a `TransactionContext` (accounts + compute meter), snapshots rent state (`TransactionAccountStateInfo::new_pre_exec`), creates `InvokeContext`, and calls `process_message`. \[Certain\]
7. **`process_message`** loops over top-level instructions: precompiles (ed25519, secp256k1, secp256r1) go to `process_precompile`; everything else to `process_instruction`. \[Certain\]
8. **`process_instruction`** → `push()` checks **reentrancy** (program already on the stack and not the immediate caller → `ReentrancyNotAllowed`) and **depth** (`CallDepth`); then `process_executable_chain` picks the builtin by the program's owner: native loader → the builtin itself; `bpf_loader*` / `loader_v4` owner → that loader's entrypoint. \[Certain\]
9. **The loader** (`process_instruction_inner`): if the invoked program is the loader itself, it runs a management instruction (deploy/upgrade/close). Otherwise it finds the program's cache entry and calls `vm::execute` on the `Loaded` executable. \[Certain\]
10. **`execute`**: charges heap cost; takes stack/heap from a thread-local pool; **serializes** accounts and instruction data into the input region (`serialize_parameters`); maps read-only ELF, stack (with frame gaps), heap and input regions; sets `r1 = MM_INPUT_START`, `r2` = instruction-data offset (newer features also pass lengths in `r3`–`r5`); runs `execute_program` (JIT preferred). \[Certain\]
11. **Your program runs.** Every syscall charges CUs; memory access outside mapped regions or writes to read-only regions trap as access violations, which the runtime maps to `InstructionError`s such as `ReadonlyDataModified` / `InvalidRealloc`. With `deplete_cu_meter_on_vm_failure`, a VM fault burns the remaining compute budget. \[Certain\]
12. **CPI** (`cpi_common`): translates the `Instruction` and `AccountInfo`s from VM memory with pointer checks, derives PDA signers from the **caller's** program ID (`translate_signers`), rejects calls to loaders/precompiles except a few upgradeable-loader ixs (`check_authorized_program`), syncs caller → callee (`update_callee_account`), recurses into `process_instruction` (step 8, with `verify_instruction_accounts` raising `PrivilegeEscalation` on any widened writable/signer flag), then syncs callee → caller (`update_caller_account`, enforcing the 10 KiB realloc limit). \[Certain\]
13. **Return.** A non-zero return code becomes an `InstructionError`. On success **`deserialize_parameters`** walks the input buffer and applies lamports, data length/data, then owner (owner last, so the program can modify data before giving ownership away) through `BorrowedAccount`, which enforces the ownership rules from 1.2. \[Certain\]
14. **Post-transaction.** `TransactionAccountStateInfo::new_post_exec` + `verify_changes` reject transitions into a rent-paying state (`InsufficientFundsForRent`); lamport sum must balance (`UnbalancedTransaction`). On failure, only fee payer and nonce changes are kept. \[Certain\]
15. **Commit.** The bank stores modified accounts; 0-lamport accounts are dropped. This is the moment a "closed" account actually disappears. \[Likely\]

### 7.3 What the input buffer looks like

Your entrypoint (`solana_program::entrypoint!`, Anchor, or Pinocchio) parses this layout from `r1` (aligned ABI, loaders v2/v3/v4): \[Certain\]

```
u64 num_accounts
for each account:
  u8  dup_marker            // 0xFF = not a duplicate; otherwise index of the original
  -- if not dup --
  u8  is_signer
  u8  is_writable
  u8  executable
  [u8;4] padding            // original_data_len slot, host keeps the real value
  [u8;32] key
  [u8;32] owner
  u64 lamports
  u64 data_len
  [u8; data_len] data       // omitted under direct mapping: mapped directly instead
  [u8; 10240] realloc space + align to 16
  u64 rent_epoch            // u64::MAX
  -- if dup --
  [u8;7] padding
u64 instruction_data_len
[u8] instruction_data
[u8;32] program_id
(optionally) u64[] account vm addresses   // SIMD-0449
```

Security takeaways from the layout:

- **Duplicates are explicit.** The same account passed twice is one buffer entry plus a dup marker; both `AccountInfo`s point to the same `RefCell` data. That's the runtime basis for duplicate-mutable-account bugs. \[Certain\]
- **Realloc is bounded by the pre-reserved 10 KiB** per instruction (and per CPI level); total size capped at 10 MiB (`MAX_PERMITTED_DATA_LENGTH`). \[Certain\]
- **Direct mapping / virtual address space adjustments** change how account data is mapped (no copy), so reads past `data_len` into the realloc area and pointer tricks behave differently across feature sets. Check which flags are active on mainnet before claiming a memory-level exploit. \[Likely\]

### 7.4 Execution pipeline at a glance

&#91;embedded content: Agave execution path · 8 steps, CPI loop\]

A CPI from your program re-enters at step 3 with the privilege check in front of it; everything after step 6 is the runtime re-validating what your program wrote.

### 7.5 Exercises for this part

1. Clone Agave at the current mainnet tag. Put a breakpoint (or `log::info!`) in `InvokeContext::push` and `cpi_common`; run a `program-test` / LiteSVM test of a CPI and watch the stack.
2. Write a test that triggers each of: `ReentrancyNotAllowed`, `PrivilegeEscalation` (writable and signer), `CallDepth`, `InvalidRealloc`, `ExternalAccountDataModified`, `UnbalancedInstruction`. Read the exact code that raised each.
3. Read `check_authorized_program` and explain why a program cannot CPI into `bpf_loader` to deploy code, but can CPI into the upgradeable loader to `Upgrade`.
4. Read `deserialize_parameters` and explain why setting owner last matters for `assign` inside your program.
5. Read the program cache's handling of `DelayVisibility` and explain what happens to a tx that invokes a program upgraded in the same slot.
6. Use [LiteSVM](https://github.com/LiteSVM/litesvm) or Mollusk for fast tests: both embed the Agave SVM crates, so failures match the real validator.

## 8. Checklist, exercises, reading list

### 8.1 Per-instruction audit checklist

Run this on every instruction of every program you audit. Every "no" is a lead.

- [ ] Every account: owner verified (type or explicit)?
- [ ] Every account: discriminator/type verified?
- [ ] Every account: tied to the others by `has_one` / seeds / address / key comparison?
- [ ] Every authority: is a `Signer` *and* is the right signer for this resource?
- [ ] Every PDA: seeds scoped to the resource, fixed-prefix, canonical bump?
- [ ] Every program account used for CPI: ID pinned?
- [ ] Every `invoke_signed`: callee trusted, PDA signs only for what this instruction should move?
- [ ] Any two `mut` accounts that could be the same account?
- [ ] Any account read after a CPI without `reload()`?
- [ ] `remaining_accounts`: owner, identity, completeness, duplicates, alignment?
- [ ] Init: can it run twice? Can someone front-run it? Can it be pre-funded into failure?
- [ ] Close: correct recipient, data zeroed, no outstanding obligations, no revival?
- [ ] Realloc: payer, zeroing, refund of vault lamports?
- [ ] Token-2022: which extensions accepted, and is each handled?
- [ ] Math: overflow checks on, casts safe, rounding against the user?
- [ ] Oracles: account pinned, staleness and confidence checked?
- [ ] Instruction introspection: sysvar address, program ID, offsets, CPI context, uniqueness?
- [ ] Admin/upgrade authority: who holds it, what can it do instantly?

### 8.2 Exercises, in order

1. **sealevel-attacks**: an exploit test for each of the 11 insecure programs. [coral-xyz/sealevel-attacks](https://github.com/coral-xyz/sealevel-attacks)
2. **Write a vulnerable vault in native Rust (no Anchor)**, then port it to Anchor and to Pinocchio. Writing raw account parsing once is the fastest way to internalize part 1.
3. **CTFs**: OtterSec's and Neodyme's Solana CTF challenges (search their GitHub orgs), plus the Solana "security workshop" repos. \[Likely names may have moved; search before relying on them\]
4. **Reproduce real exploits**: Wormhole (sysvar spoofing, Feb 2022), Cashio (missing validation chain on collateral, Mar 2022), Crema (fake tick account / owner check, Jul 2022), Mango Markets (oracle manipulation, Oct 2022). Write a one-paragraph root cause for each in part-1 vocabulary, then compare with the sourced write-ups in [guide 04](./04-advanced-security-and-exploits.md).
5. **Audit reports**: read 10 public reports on the three protocols in part 5 (OtterSec, Neodyme, Zellic, Sec3, Offside, Certora publish many). For each finding, map it to a part-6 class.
6. **Fuzzing**: Trident (Ackee) for Anchor programs, and Crucible, which ships built into Anchor v2. Fuzz one invariant from each protocol in part 5. \[Likely\]

### 8.3 Reading list

| Source | What it is for |
| --- | --- |
| [Solana docs: core concepts](https://solana.com/docs/core) | Accounts, transactions, PDAs, CPIs: the official spec-level reference |
| [Anchor account constraints](https://www.anchor-lang.com/docs/references/account-constraints) | Authoritative constraint list for the current version |
| [Anchor release notes](https://www.anchor-lang.com/docs/updates/changelog) | Security-relevant behaviour changes between versions (dup accounts, discriminators, close) |
| [anza-xyz/agave](https://github.com/anza-xyz/agave) | Runtime source, part 7 |
| [anza-xyz/sbpf](https://github.com/anza-xyz/sbpf) | The VM: verifier, interpreter, JIT |
| [solana-program/token](https://github.com/solana-program/token) | Token program, including p-token |
| [Solana Improvement Documents](https://github.com/solana-foundation/solana-improvement-documents) | Every runtime change (search SIMD numbers cited in this guide: 0266, 0268, 0433, 0449) |
| [p-token upgrade page](https://solana.com/upgrades/p-token) | What changed in the Token program in 2026 |
| [sealevel-attacks](https://github.com/coral-xyz/sealevel-attacks) | Canonical bug-class examples |

### Sources

Checked while writing (2026-11-26): Agave `svm/src/transaction_processor.rs`, `program-runtime/src/invoke_context.rs`, `cpi.rs`, `vm.rs`, `serialization.rs`, `programs/bpf_loader/src/lib.rs` on `master`; repo listings for raydium-cp-swap, klend and protocol-v2; the Anchor constraints reference; the Solana p-token upgrade page. Everything else is from general knowledge and tagged accordingly.

---

**Previous:** [← 02 · Anchor Deep-Dive](./02-anchor-deep-dive.md) · **Next:** [04 · Advanced Security & Real-World Exploits →](./04-advanced-security-and-exploits.md)
