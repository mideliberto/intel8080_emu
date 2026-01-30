# Vision: Era-Inappropriate Computing

**The 8080 doesn't know it's connected to the internet. It just reads bytes from ports.**

---

## The Crown Jewel: Talking to Claude

An 8080 that talks to its co-creator.

This emulator was built in collaboration with Claude. The monitor ROM, the storage system, the architecture decisions - all developed through conversation. The logical endpoint: the 8080 itself can continue that conversation.

**How it works:**

```
> A How do I implement the R command?

You deferred it on December 16th. No use case yet.

The blocker is return mechanism - G does PCHL and never
comes back. Options:

1. RST 7 as breakpoint - saves regs, returns to monitor
2. CALL wrapper - user code CALLs instead of being JMPed
3. Skip it until you're actually debugging something

I'd pick option 3. YAGNI.

What are you trying to debug?
```

**What makes this special:**

The Claude instance has *context*. It knows:
- The project history (from `COLLABORATION_LOG.md`)
- Mike's tendencies (over-engineering, late-night feature creep)
- The mantra ("A fool admires complexity...")
- Recent decisions and their rationale
- What's actually implemented vs. what's aspirational

It's not a generic chatbot. It's the same collaborator, reconstructed.

**The 8080 sees:**
- Prompt bytes going out port 0x38
- Status on port 0x3A
- Response bytes coming in from port 0x3B

It has no idea it's talking to an AI. It's just I/O.

**Required phases:**
- Phase 4: Storage (DONE) - for collaboration log
- Phase 8: Network device - for HTTPS
- Phase 9: Claude device wrapper
- System prompt with project context

**Files:**
- `COLLABORATION_LOG_SPEC.md` - How to maintain context
- `CLAUDE_8080_SYSTEM_PROMPT.md` - The actual API prompt

---

## The Gutenberg Reader

An 8080 e-reader. Download books from Project Gutenberg over HTTP, cache them in storage, read them one screen at a time.

**How it works:**
1. HTTP device fetches text from gutenberg.org
2. Stream directly to storage (16MB per book, plenty)
3. `B` command opens book viewer
4. Display 20-24 lines at a time
5. Page up/down through the text
6. When buffer runs low, swap in next chunk from storage

**The 8080 sees:**
- Bytes coming in from HTTP port
- Bytes going out to storage port
- Bytes coming back from storage to display

It has no idea it's reading Tolstoy.

**Classic technique:** Memory overlays. When RAM was 64KB and books were longer than that, you paged data in and out. We're doing it again, but the "disk" is a 16MB file and the "remote terminal" is the internet.

**Required phases:**
- Phase 4: Storage (DONE)
- Phase 8: HTTP client
- New: Book viewer command

---

## Other Ideas

Things that fit the "era-inappropriate" philosophy:

### Weather Station
Fetch weather data from an API, display current conditions. The 8080 doesn't know about JSON - the coprocessor parses it and sends simple text.

### Stock Ticker
Stream stock prices. Update every few seconds. The 8080 just displays what arrives on the port.

### Chat Client
Send/receive messages through Claude API or a simple chat server. The 8080 is a dumb terminal - exactly what it was designed to be.

### Time Server
NTP sync through the coprocessor. Display accurate time. Set timers. The 8080 doesn't know about UDP - it just reads the clock ports.

### BBS Gateway
Connect to telnet BBSes through the coprocessor. The 8080 thinks it's talking to a serial port.

### Scrabble / Word Games
Dictionary stored in 16MB storage. Game state in RAM. The 8080 can absolutely handle turn-based word games with that much reference data.

### Email Client
POP3/SMTP aren't complex protocols. Fetch mail, display it, compose replies. Store drafts to storage. Send through the network device.

---

## The Philosophy

The 8080 is simple. The coprocessor handles complexity.

Every "impossible" feature becomes possible when you realize:
1. The 8080 only needs to move bytes
2. The coprocessor can do anything
3. Ports are the abstraction boundary

The same ROM runs on the emulator today and real hardware tomorrow. The 8080 doesn't know the difference. That's the point.

**Not a museum piece. A living system.**

---

## Hardware Path

The vision extends to real silicon:

```
â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”     Ports      â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
â”‚  Real 8080  â”‚â—„â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â–ºâ”‚  Pi Zero W      â”‚
â”‚  (vintage   â”‚   directly or  â”‚                 â”‚
â”‚   chip)     â”‚   via SPI/I2C  â”‚  - WiFi         â”‚
â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜                â”‚  - SD Card      â”‚
                               â”‚  - TLS          â”‚
                               â”‚  - Claude API   â”‚
                               â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜
```

Same ports. Same ROM. Same code. Different substrate.

The Rust emulator isn't throwaway scaffolding. It's a prototype of the coprocessor firmware. When we move to Pi, we're porting the device implementations, not rewriting the 8080 interface.

---

## The End State

An 8080 that:
1. Boots from ROM overlay (like real S-100 systems)
2. Stores data to 16MB files (like modern SD cards)
3. Fetches data from the internet (via coprocessor)
4. Talks to Claude for assistance (maintaining project context)
5. Runs on real hardware (with Pi coprocessor)

A 1975 processor with 2025 capabilities. The ports are the portal.
