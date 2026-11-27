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
3. For exercises (`exercises/00-rust-fundamentals`):
   - Put new tests in `shared/exercises.rs` so they run against both `starter` and `solutions`.
   - Add the reference answer to `solutions/` and a `todo!()` stub to `starter/`.
   - Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings` and `cargo test -p solutions`.
4. Open a pull request describing what changed and why. CI checks links and runs the exercise tests.

## Reporting problems

Open an issue with the guide and section, what's wrong, and (if you can) a source for the correct version. For outdated APIs, include the crate version you tried.

## Security

This repo contains deliberately vulnerable **example** code. If you believe you've found a vulnerability in a real deployed program, report it to that project's security contact or bug-bounty program, not here.

## License

By contributing, you agree that your code contributions are licensed under MIT and your text contributions under CC BY 4.0, as described in the README.
