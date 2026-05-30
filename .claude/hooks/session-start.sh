#!/bin/sh
# SPDX-License-Identifier: PMPL-1.0-or-later
# Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
#
# session-start.sh — Claude Code (web) SessionStart hook for verisimiser.
#
# Warms the Cargo dependency + build cache so `cargo build`, `cargo test`,
# `cargo clippy`, and `cargo fmt` are ready the moment a web session starts,
# instead of paying the cold-compile cost on the first tool call. Runs
# synchronously; the container state is cached after it completes.
# Idempotent and non-interactive.
set -eu

# Only run in the remote (Claude Code on the web) environment. Local
# developers manage their own toolchain via setup.sh / direnv (.envrc).
if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
  exit 0
fi

cd "${CLAUDE_PROJECT_DIR:-.}"

# Fetch the pinned dependency graph (Cargo.lock is committed), then compile
# all targets including tests. A transient compile error in a work-in-progress
# tree must not block the session from starting, so the warm build is
# best-effort; the fetch is the part that genuinely needs to succeed.
cargo fetch
cargo build --all-targets \
  || echo "session-start: cargo build did not complete cleanly (continuing)" >&2
