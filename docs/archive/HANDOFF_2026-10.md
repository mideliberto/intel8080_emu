# Alignment Handoff — 2026-10-02

One-time document. Delete or move to `docs/archive/` once applied.

## Why this exists
Last code commit: 2026-01-30. Project Knowledge on the web side had drifted from the repo: stale counts, mojibake from repeated copy/paste (`â€”`, `Ã¢â€â€`), and six files that existed only in PK. This package resets both sides to one truth: the repo.

## Verified baseline (GitHub HEAD, 2026-10-02)
- `cargo test`: 14 unit + 180 CPU + 10 monitor = **204 passing**. Docs said 191; that number was from before the January cleanup.
- `rom/monitor.bin`: ~2,472 of 4,096 bytes used.
- `rom/monitor.lst` and `rom/monitor.p` are tracked despite `.gitignore` (committed before the ignore rule).

## What this package changes
| File | Change |
|------|--------|
| `CLAUDE.md` | Rewritten: roles, guardrails, end-of-session protocol, source-of-truth rule |
| `TODO.md` | Stale "Current" items cleared; Open Decisions added |
| `docs/COLLABORATION_LOG.md` | Encoding repaired; 3 Key Decisions, 3 sessions, Current State, Review Metadata |
| `docs/ARCHITECTURE.md` | Encoding repaired only |
| `docs/DEVICE_SPECS.md` | Encoding repaired only |
| `docs/archive/PHASE4_STORAGE_PLAN.md` | Encoding repaired only |
| `docs/COLLABORATION_LOG_SPEC.md` | New to repo (was PK-only) |
| `docs/archive/session_summary_rom_overlay.md` | New to repo (was PK-only) |
| `docs/reference/*.txt` | New to repo (were PK-only) |

Dropped: `COLLABORATION_LOG_UPDATE_PROMPT.md`. Its job (excavating chats to rebuild history) is replaced by Claude Code writing the log as it goes.

## Open decisions found during audit
1. **`H` collision.** Roadmap assigns `H` to the HEX loader. `H` is Hex Math.
2. **`:` collision.** Phase 10 claims `:` for emulator commands. Intel HEX records start with `:`.

Both are cheap to fix now and expensive after Phase 5 code exists. Decide in the web Project, record in COLLABORATION_LOG Key Decisions, then code.

## Apply checklist (Claude Code)
1. Copy package files over the repo, preserving paths.
2. `git diff --stat` and confirm only the files above changed.
3. `git rm --cached rom/monitor.lst rom/monitor.p`
4. `cargo test` — expect 204.
5. Commit: `docs: align repo with web project, repair encoding, add session protocol`. Push.
6. Move this file to `docs/archive/`.

## Web side (Mike, manual)
Replace Project Knowledge with: `CLAUDE.md`, `TODO.md`, everything in `docs/` except `archive/`, `rom/monitor.asm`, `src/main.rs`, `Cargo.toml`. Delete the rest. The repo is public, so the web Project can also clone it directly when it needs code beyond that.
