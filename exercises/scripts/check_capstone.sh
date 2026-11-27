#!/usr/bin/env bash
# Prove every capstone does what it claims, for one exercise workspace:
#   * capstone-solution passes its whole suite;
#   * the vulnerable capstone passes every honest_* test;
#   * every single exploit_* test FAILS on the vulnerable capstone
#     (so each one really demonstrates a bug).
# Usage: exercises/scripts/check_capstone.sh exercises/01-solana-foundations
set -euo pipefail
cd "$1"

cargo test -q -p capstone-solution
cargo test -q -p capstone honest_

mapfile -t exploits < <(cargo test -q -p capstone --test capstone -- --list 2>/dev/null \
  | sed -n 's/^\(exploit_[A-Za-z0-9_]*\): test$/\1/p')
if [ "${#exploits[@]}" -eq 0 ]; then
  echo "::error::no exploit_* tests found in $1"
  exit 1
fi

fail=0
for t in "${exploits[@]}"; do
  if cargo test -q -p capstone --test capstone -- --exact "$t" >/dev/null 2>&1; then
    echo "::error::$t PASSED on the vulnerable crate; it does not demonstrate the bug"
    fail=1
  else
    echo "ok: $t fails on the vulnerable crate (bug demonstrated)"
  fi
done
exit $fail
