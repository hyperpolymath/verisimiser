// SPDX-License-Identifier: PMPL-1.0-or-later
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
//
// Cross-process advisory write lock for the JSON sidecar (V-L2-F4, #150).
//
// The SQLite sidecar serialises concurrent writers through the database
// write lock; the JSON store has no such engine, so this provides the
// equivalent: an exclusive sibling lock file (`<path>.lock`) created with
// `O_EXCL` semantics (`create_new`). It is dependency-free (no new crate,
// no `unsafe`) and host-local.
//
// Crash resilience: because an `O_EXCL` lock file is not released by the OS
// when a process dies, a lock whose mtime is older than `stale_after` is
// treated as abandoned and stolen. The window is a deliberate trade-off —
// long enough that a slow-but-live writer is not robbed, short enough that
// a crash doesn't wedge the sidecar for long. Acquisition retries with a
// fixed interval until a timeout.
//
// Caveat: like all lock-file schemes this is host-local and not safe over
// network filesystems without working `O_EXCL` (e.g. some NFS configs).

use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result};

/// Default wait before giving up acquiring a contended lock.
const DEFAULT_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(10);
/// Default age past which a lock file is presumed abandoned and stolen.
const DEFAULT_STALE_AFTER: Duration = Duration::from_secs(30);
/// Poll interval while a lock is contended.
const RETRY_INTERVAL: Duration = Duration::from_millis(25);

/// An acquired advisory lock; the lock file is removed on drop.
#[derive(Debug)]
pub struct FileLock {
    path: PathBuf,
}

impl FileLock {
    /// The lock-file path for a target sidecar (`<target>.lock`).
    fn lock_path(target: &Path) -> PathBuf {
        let mut name = target.as_os_str().to_owned();
        name.push(".lock");
        PathBuf::from(name)
    }

    /// Acquire the lock for `target`, blocking (with retry) up to the
    /// default timeout and stealing a stale lock if necessary.
    pub fn acquire(target: &Path) -> Result<Self> {
        Self::acquire_with(target, DEFAULT_ACQUIRE_TIMEOUT, DEFAULT_STALE_AFTER)
    }

    /// Like [`acquire`](FileLock::acquire) but with explicit `timeout` and
    /// `stale_after` (used by tests to exercise contention/staleness fast).
    pub fn acquire_with(target: &Path, timeout: Duration, stale_after: Duration) -> Result<Self> {
        let path = Self::lock_path(target);
        let deadline = Instant::now() + timeout;
        loop {
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut file) => {
                    // Record holder (pid + epoch secs) for diagnostics; the
                    // file's mere existence is the lock, so ignore write errs.
                    let _ = writeln!(file, "{} {}", std::process::id(), epoch_secs());
                    return Ok(FileLock { path });
                }
                Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                    if is_stale(&path, stale_after) {
                        // Abandoned by a dead writer: steal and retry.
                        let _ = std::fs::remove_file(&path);
                        continue;
                    }
                    if Instant::now() >= deadline {
                        anyhow::bail!(
                            "timed out after {:?} acquiring sidecar lock {} \
                             (another writer holds it)",
                            timeout,
                            path.display()
                        );
                    }
                    sleep(RETRY_INTERVAL);
                }
                Err(e) => {
                    return Err(e)
                        .with_context(|| format!("creating sidecar lock file {}", path.display()));
                }
            }
        }
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        // Best-effort release; a leftover lock will be reclaimed as stale.
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Seconds since the Unix epoch (0 if the clock is before it).
fn epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// `true` if `path` exists and its mtime is older than `stale_after`.
fn is_stale(path: &Path, stale_after: Duration) -> bool {
    let Ok(modified) = std::fs::metadata(path).and_then(|m| m.modified()) else {
        return false;
    };
    SystemTime::now()
        .duration_since(modified)
        .map(|age| age > stale_after)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acquire_and_release_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("sidecar.json");
        let lock_file = FileLock::lock_path(&target);
        {
            let _guard = FileLock::acquire(&target).unwrap();
            assert!(lock_file.exists(), "lock file exists while held");
        }
        assert!(!lock_file.exists(), "lock file removed on drop");
        // Re-acquire after release succeeds.
        let _again = FileLock::acquire(&target).unwrap();
    }

    #[test]
    fn contended_lock_times_out_quickly() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("sidecar.json");
        let _held = FileLock::acquire(&target).unwrap();
        // A second acquisition with a tiny timeout and a long stale window
        // must fail rather than steal a live lock.
        let err = FileLock::acquire_with(
            &target,
            Duration::from_millis(80),
            Duration::from_secs(3600),
        )
        .unwrap_err();
        assert!(err.to_string().contains("timed out"), "got: {err}");
    }

    #[test]
    fn stale_lock_is_stolen() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("sidecar.json");
        // Leave a lock behind (simulating a crashed writer).
        std::mem::forget(FileLock::acquire(&target).unwrap());
        assert!(FileLock::lock_path(&target).exists());
        // With a tiny stale window, the next acquire steals it.
        std::thread::sleep(Duration::from_millis(40));
        let _stolen =
            FileLock::acquire_with(&target, Duration::from_secs(2), Duration::from_millis(20))
                .expect("stale lock should be stolen");
    }
}
