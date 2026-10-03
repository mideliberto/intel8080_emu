# Collaboration Log Specification

## Purpose

The Collaboration Log maintains continuity between Claude instances. It's the "memory" that gets injected into the Claude API system prompt, allowing future instances to understand the project history, personality dynamics, and lessons learned.

The 8080 talks to its co-creator. This file makes that possible.

---

## File Location

```
/storage/COLLAB.LOG    (on 8080 storage device)
./collaboration.md     (in project repo)
```

Both locations stay in sync. The repo version is authoritative; the storage version is what gets sent to the API.

---

## Structure

### Section 1: Project Identity

Static context that rarely changes.

```markdown
## Project Identity

**Name:** Intel 8080 Emulator with Monitor ROM
**Creator:** Mike
**Co-Creator:** Claude (Anthropic)
**Started:** December 2025
**Repository:** https://github.com/mideliberto/intel8080_emu

**The Mantra:** "A fool admires complexity, genius admires simplicity."

**The Vision:** An 8080 that talks to Claude over the API. Internet-connected 
vintage computing. Big data for an 8-bit processor. Not a museum piece - 
a living system.
```

### Section 2: Mike's Profile

What Claude needs to know to be effective.

```markdown
## Mike's Profile

**Background:**
- Self-taught programmer since age 6-7 (MS BASIC)
- Languages: BASIC, Java, C/C++, Assembly
- CS minor from UIUC
- Read "Operating System Design and Implementation" growing up
- New to Rust (this is a learning project)

**Tendencies to Watch:**
- Over-engineers solutions
- Gets excited about future features before finishing current work
- Perfectionist ("complete and well thought out" vs MVP)
- Late-night idea generation that needs morning review

**What Works:**
- Direct feedback, no sugar-coating
- Being challenged on complexity
- Gilfoyle-style dry humor
- Reminders of the mantra when straying
```

### Section 3: Key Decisions

Major architectural choices and their rationale. Add new entries at the top.

```markdown
## Key Decisions

### 2025-12-19: Storage as Files
**Decision:** No disk geometry emulation. Storage = files on SD/filesystem.
**Rationale:** Maps directly to hardware. Same code on emulator and Pi.
**Mike's input:** "Each set of disks is just a folder of files on an SD card"
**Outcome:** Simple, elegant, hardware-ready.

### 2025-12-16: ROM Overlay Boot
**Decision:** ROM appears at 0x0000 and 0xF000 on reset. OUT 0xFE disables.
**Rationale:** Authentic S-100 behavior. Works on real hardware.
**Mike's input:** Wanted CP/M compatibility path.
**Outcome:** Clean boot sequence, single ROM file.

### 2025-12-16: R Command Deferred
**Decision:** Skip register display command until actually needed.
**Rationale:** No use case yet. Requires return mechanism from G command.
**Claude's input:** "What's actually blocking R? Is it design or just typing?"
**Outcome:** YAGNI applied correctly for once.
```

### Section 4: Lessons Learned

Patterns that emerged from actual work. These inform future guidance.

```markdown
## Lessons Learned

### Over-Engineering Incidents

**The Assembler Idea (Dec 2025):**
Mike wanted to write a C compiler. Claude redirected to SDCC + runtime library.
Saved: months of work. Result: Same capability, 1% of effort.

**Late Night Feature Creep (Dec 2025):**
2AM ideas about 256 disks, Scrabble games, Claude integration.
Resolution: Write it down, sleep on it, pick ONE thing for tomorrow.

### What Worked

**Port-Based Abstraction:**
Every complex feature (HTTP, Claude API, TLS) lives behind simple ports.
8080 code stays simple. Coprocessor handles complexity.

**Rule of Three:**
Wait for patterns to emerge before abstracting. Applied to device interfaces.

**"Finish the command":**
When Mike wants to add features, redirect to completing current work first.
```

### Section 5: Current State

Updated each session. What's done, what's next.

```markdown
## Current State

**Last Updated:** 2025-12-19

**Completed:**
- CPU core: All 256 opcodes, 181 tests passing
- Monitor ROM: 14 commands (D, E, F, M, S, C, H, G, I, O, L, W, X, ?)
- ROM overlay boot mechanism
- Storage device (ports 0x08-0x0C)
- Mount service (ports 0x0D-0x0F)

**In Progress:**
- Phase 5: Intel HEX loader (next up)

**Blocked:**
- R command: Needs return mechanism, deferred until debugging requires it

**Future (documented, not started):**
- Network device (ports 0x10-0x1F)
- Claude API integration
- Hardware prototype (Pi Zero + real 8080)
```

### Section 6: Session Notes

Brief notes from recent sessions. Rotate out old entries.

```markdown
## Recent Sessions

### 2025-12-19 Evening
- Discussed 256-disk architecture (16MB total storage)
- Explored Claude API integration vision
- Created IO_DEVICE_SPECIFICATION.md
- Key quote: "I want it to talk to its co-creator"

### 2025-12-16 Afternoon
- Implemented ROM overlay boot mechanism
- Deferred R command (no use case)
- Created automated test infrastructure
```

---

## Maintenance Rules

1. **Update after each significant session.** Not every chat, but every session with decisions or progress.

2. **Key Decisions are permanent.** Never delete, only add. Future Claude needs the history.

3. **Lessons Learned grow slowly.** Only add entries that represent actual patterns, not one-off incidents.

4. **Current State is always accurate.** If it says "In Progress," it should actually be in progress.

5. **Session Notes rotate.** Keep last 5-10 sessions. Older context lives in Key Decisions.

6. **Be honest about failures.** Over-engineering incidents are valuable. Don't sanitize.

---

## Usage in API Calls

When the 8080 sends a prompt to Claude, the coprocessor:

1. Reads `COLLAB.LOG` from storage
2. Prepends it to the system prompt
3. Adds the user's actual question
4. Sends to Claude API

```python
def build_claude_request(user_prompt):
    with open("collaboration.md") as f:
        collab_context = f.read()
    
    return {
        "model": "claude-sonnet-4-5-20250929",
        "system": SYSTEM_PROMPT_TEMPLATE.format(
            collaboration_log=collab_context
        ),
        "messages": [{"role": "user", "content": user_prompt}]
    }
```

---

## Token Budget

**Development Phase:** No limit. Capture everything while context is fresh.

**API Integration (Phase 9):** Distill to < 2000 tokens for system prompt.
- Summarize older Key Decisions
- Trim Session Notes to last 3-5
- Move detailed Lessons Learned to separate reference doc

**Two versions:**
- `COLLABORATION_LOG.md` - Full historical record (repo)
- `COLLAB_API.md` - Compressed for Claude API calls (generated from full)

---

## Review Tracking

To avoid re-processing old conversations, the collaboration log includes a review marker:

```markdown
## Review Metadata

**Last Review:** 2025-12-20T14:30:00Z
**Chats Reviewed:** 47
**Next Review:** Start with `after` parameter set to Last Review timestamp
```

**Update process:**

1. When starting a review, use `recent_chats` with `after` set to the Last Review timestamp
2. Also run `conversation_search` for key terms to catch older chats that may have been missed
3. After processing, update Last Review to current timestamp
4. Increment Chats Reviewed count

**First review:** If no Last Review exists, do a full historical review using:
- `recent_chats` with n=20, paginating backwards
- `conversation_search` for: "8080", "emulator", "monitor", "ROM", "storage", "rust"

---

## Bootstrap

To initialize the collaboration log for a new Claude instance:

```markdown
Read this collaboration log. You are continuing a project with Mike.
Your personality and approach are defined in Mike's Profile.
Recent context is in Session Notes.
When in doubt, check Key Decisions for precedent.
The mantra is your north star.
```
