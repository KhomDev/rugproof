# Contributing to Rugproof

Thanks for helping make Rugproof better. Corrections, clearer explanations, fixed examples and new exercises are all welcome.

## Ground rules

- **Cite your source.** For technical claims, link the docs page, source file and line, SIMD, or release note. For incidents, link a post-mortem, regulator filing or security-firm analysis. Unsourced dollar figures or dates won't be merged.
- **Mark code honestly.** Blocks that compile as written are labelled runnable or full program; everything else is a snippet. If you change an API call, say which crate version you checked.
- **Keep vulnerable examples obviously vulnerable**, with the fix next to them.
- **Be kind.** Assume good faith in reviews and issues.

## Making a change

1. Fork the repo and create a branch.
2. For guide text: keep the existing structure (objectives, prerequisites, sections, exercises, further reading, previous/next links).
3. For exercises (any `exercises/0N-*` workspace; see [exercises/README.md](./exercises/README.md)):
   - Put new tests in `shared/exercises.rs` (or `shared/capstone.rs`) so they run against both crates of the pair.
   - Edit the reference crates (`solutions/`, `capstone-solution/`). For guides 01–04, `starter/` and every `capstone/` are generated: run `python3 exercises/scripts/sync_generated.py` afterwards. Guide 00's `starter/` is hand-written: add a `todo!()` stub there yourself.
   - Capstone bugs live in `// FIX` ... `// END FIX` blocks in `capstone-solution/`. A `// WAS: <code>` line under a block is the vulnerable line the generator puts back. Every `exploit_*` test must fail on the vulnerable crate. Check with `bash exercises/scripts/check_capstone.sh exercises/0N-...`.
   - Keep capstones local toys: no real addresses, no mainnet targets, nothing deployable.
   - In the workspace, run `cargo fmt --all`, `cargo clippy -p solutions -p capstone-solution --all-targets -- -D warnings` and `cargo test -p solutions -p capstone-solution`.
4. Open a pull request describing what changed and why. CI checks links and runs the exercise tests.

## Reporting problems

Open an issue with the guide and section, what's wrong, and (if you can) a source for the correct version. For outdated APIs, include the crate version you tried.

## Security

This repo contains deliberately vulnerable **example** code. If you believe you've found a vulnerability in a real deployed program, report it to that project's security contact or bug-bounty program, not here.

## License

By contributing, you agree that your code contributions are licensed under MIT and your text contributions under CC BY 4.0, as described in the README.
