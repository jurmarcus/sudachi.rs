# Branch Maintenance

This is a fork of [WorksApplications/sudachi.rs](https://github.com/WorksApplications/sudachi.rs)
maintained at [github.com/jurmarcus/sudachi.rs](https://github.com/jurmarcus/sudachi.rs).
The fork's `main` branch carries our performance + WASM additions on top of
upstream's `develop`.

## Branch map

| Branch | Source | Purpose |
|---|---|---|
| `main` | jurmarcus | The dependency line for downstream consumers (`~/code/sudachi`, `~/code/jisho`). All perf + WASM commits live here. |
| `develop` | upstream WorksApplications | Mirrored from upstream. Update with `sl pull`. **Never commit to it.** |
| `develop-v0.7` | upstream | Mirrored from upstream. Future major version (milestone 2, due 2026-06-30 with likely slip). Breaking dict format. Tracked but unused for our daily-driver work. |
| `feature/*`, `fix/*`, `refactor/*`, `pre/*` | upstream | Upstream in-flight branches. Never touched directly; the merges land in `develop` or `develop-v0.7` when ready. |
| `wasm-compat` | jurmarcus (deleted 2026-05-04) | Retired — its useful contents were ported to `main` in commits `fd458f99581c` (from_system_bytes), `e343b9d6c100` (build-dictionary feature), `46306a90d620` (target_family wasm gating). |

## When upstream lands a commit on `develop`

```bash
sl pull
sl log -r 'remote/develop % main' -T '{node|short} {desc|firstline}\n'  # commits since last sync
sl rebase -s 'main % remote/develop' -d 'remote/develop'                # rebase our perf stack
sl push --to main                                                        # update fork
```

After rebase, re-run benches to confirm no upstream change regressed our gains:

```bash
cargo bench --bench tokenize -p sudachi -- --quick
```

If any bench regresses by >2%, investigate which upstream commit caused it before pushing.

## When upstream releases a new minor version (0.6.13, etc.)

Same as above — `develop` advances, we rebase. The dict format is stable
within 0.6.x, so no dictionary churn for downstream consumers.

## When upstream releases 0.7.0 (BREAKING)

This is a planned migration project, not a routine sync. Steps:

1. **Wait for SudachiDict to ship a v0.7-format release.** As of 2026-05-04
   there is none — only v0.6 dicts exist. Latest: `v20260428` (Apr 30,
   2026, 0.6 format). Even WorksApplications can only test v0.7 internally
   with self-built dicts.
2. **Verify downstream consumers can absorb the API churn.** Open issues
   in milestone 2 include `update word_infos to V1`, `config system rework`,
   `align analyzer factory method names`. These will require code changes
   in `~/code/sudachi/crates/*` and `~/code/jisho/packages/rs/jisho-core`.
3. **Decide migration target:**
   - If our consumers can absorb the churn AND a v0.7 dict is available:
     rebase the perf stack onto `remote/develop-v0.7`. Several of our
     commits will conflict with v0.7's changes to dict reading code; some
     (Tier 2 work, if done) may become redundant with v0.7's
     `modify utf16 reader` / `impl CompactedStrings` commits.
   - Otherwise: keep `main` on develop indefinitely. Downstream pins to a
     0.6 tip; 0.7 work continues in a separate branch like `main-0.7` if
     needed for upstream PRs.

## Upstream PR strategy

Our perf work is upstream-PR-able. Tracker for individual PRs lives in
`~/notes/projects/sudachi-rs/2026-05-04-tier-plan.md` Phase 5 section.
Cadence: open one PR per logical change, target `develop-v0.7`,
let upstream CI verify against their internal v0.7 dict.

## What was retired and why

### `wasm-compat` branch

Deleted 2026-05-04. Three contributions ported to `main`:

| wasm-compat commit | Replacement on `main` |
|---|---|
| `from_system_bytes` + `from_system_static_bytes` constructors | `fd458f99581c` |
| `build-dictionary` cargo feature gate | `e343b9d6c100` |
| `target_family = "wasm"` cfg style | `46306a90d620` |

The `libloading` dependency gating was already done as our Task 2 commit
`9d3da0fe9eff` (using `target_arch = "wasm32"`, then widened to
`target_family = "wasm"` in `46306a90d620`).

Anyone with a `[patch."https://github.com/WorksApplications/sudachi.rs"]`
block pointing at `a7c50e44...` (the old wasm-compat tip) will see fetch
errors on next `cargo update` and should switch to:

```toml
sudachi = { git = "https://github.com/jurmarcus/sudachi.rs", rev = "<main-tip>" }
# Drop the [patch] block — nothing to patch since wasm-compat is gone.
```

## Snapshot of fork divergence (2026-05-04)

The fork's `main` is N commits ahead of `remote/develop` (the upstream
0.6 tip). Run `sl log -r 'main % remote/develop'` to see the current
diff stack. Each commit is a self-contained perf or feature change
suitable for individual review.

To see the divergence size: `sl log -r 'main % remote/develop' -T '{rev}\n' | wc -l`
