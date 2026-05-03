# DCB-02b-A — `NeedRefill` slice API revision for audio_player

**Status:** **Resolved 2026-05-03 — Option A ratified.** Folded
into DCB-00 §10 (audio_player row updated) + §15
(ratification entry) via the 2026-05-03 callback-refill
amendment. DCB-02b-A2 implementation is now unblocked. This
file is preserved as historical analysis only; no behaviour
PRs reference it directly.

## 1. Purpose

Close the residual raw-pointer write path in
`audio_player.rs`'s refill API. DCB-02b made the cache op
type-system-tracked but left the PCM byte writes themselves
behind a `*mut u8` pointer in `PollResult::NeedRefill`.
DCB-02b-A makes the writes safe-slice-based.

## 2. Problem statement

### 2a. Current shape (post DCB-02b)

```rust
pub enum PollResult {
    Idle,
    Playing,
    NeedRefill {
        buf: *mut u8,        // ← raw pointer to inactive bank
        file_offset: u32,
        max_bytes: usize,
    },
    Finished,
}

// Caller pattern:
match audio_player.poll() {
    PollResult::NeedRefill { buf, file_offset, max_bytes } => {
        unsafe { /* memcpy PCM data into buf via raw pointer */ }
        audio_player.refill_done(bytes_written);
    }
    ...
}
```

`refill_done` constructs a `BankGuard<Read>` internally and
calls `release(&mut ctx, current_target)` which (post-DCB-01d)
emits the cache clean. So the cache discipline is type-tracked
already; only the byte-write path is still `unsafe { ... raw
pointer ... }`.

### 2b. Why it stayed

DCB-02b's commit notes flagged this:

> The bank-guard's `as_mut_slice()` is intentionally NOT used
> here — the caller already wrote PCM bytes through the raw
> pointer returned by `PollResult::NeedRefill`. The guard's
> role on this code path is to (a) emit the cache clean for
> the inactive bank, (b) provide the borrow-check guarantee
> that no concurrent guard exists. A future API revision
> could thread the `&mut [u8]` through `NeedRefill` to make
> the slice access typesafe end-to-end; that's deferred to a
> DCB-02b-A follow-up.

The "future API revision" is what this sub-letter ratifies.

### 2c. Self-referential constraint

The straightforward refactor — return a token holding the
`BankGuard` from `poll()` so the caller writes via
`token.as_mut_slice()` then `commit()`s — is self-referential:
the player owns `DcaState` (with `DbufRead` inside), the token
borrows from `DbufRead`, and the token also needs `&mut SCB`
for `release`. Putting the token in the player's state would
create a self-referential struct. Returning the token from
`poll()` works (the `'a` lifetime ties to `&mut self`), but
the `commit()` path needs to access *other* fields of the
player (`bytes_queued`, `state`, `dma.current_target()`),
which are also borrowed by the token's `&mut self` parent.

## 3. Options

### Option A — Callback-based refill (`poll_refill<F>`)

Replace `poll()` + external `refill_done()` with a single
`poll_refill<F>(&mut self, refill: F) -> PollResult` method.
The closure receives `&mut [u8]` (the inactive bank's slice
via the BankGuard) plus file offset, and returns the number
of PCM bytes written. The player handles the bank-guard
construction, release, and `bytes_queued` accounting
internally — caller never sees the guard or a raw pointer.

```rust
pub enum PollResult {
    Idle,
    Playing,
    Finished,
    // NeedRefill removed — handled inside the closure
}

impl<const N: usize> AudioPlayer<N> {
    pub fn poll_refill<F>(&mut self, refill: F) -> PollResult
    where
        F: FnOnce(&mut [u8], u32) -> usize,
    { ... }
}

// Caller:
let result = audio_player.poll_refill(|buf, file_offset| {
    let to_copy = ...;
    buf[..to_copy].copy_from_slice(...);
    buf[to_copy..].fill(0);
    to_copy
});
```

**Pros**:
- No self-referential token; the closure scope = guard scope.
- Caller never sees a raw pointer.
- `PollResult` simplifies (one fewer variant).
- Matches the standard "scoped resource" pattern in Rust
  (`with_*`-style APIs).

**Cons**:
- Breaking API change. The single in-tree consumer (the disco
  bare-metal binary at `examples/stm32h747i-disco/src/main.rs:
  3266`) needs a mechanical refactor — the existing
  pattern-match on `PollResult::NeedRefill` becomes a closure.
- The closure can't propagate user errors elegantly without
  a `Result<usize, E>` return type — which requires generics
  on the error. For the audio-player path this isn't an
  issue (caller's error handling is "log + zero-fill"), but
  it's a soft-fork point.

### Option B — Token type returned by a separate `acquire_refill` call

Add `fn acquire_refill(&mut self) -> Option<RefillToken<'_, N>>`
to the player; `poll()` stays unchanged (returns Idle / Playing
/ NeedRefill { file_offset, max_bytes } / Finished, with the
`buf` field removed from NeedRefill). Caller pattern-matches
`NeedRefill`, calls `acquire_refill()`, gets a token whose
`as_mut_slice()` is the safe view, then `token.commit(pcm)`
finalises.

**Pros**:
- Preserves `PollResult::NeedRefill` for callers that pattern-
  match on it.
- Token-based API is more flexible than callback (no closure
  capture issues; can hold a long-lived borrow if needed).

**Cons**:
- Token must hold `BankGuard` + `&mut SCB` + need access to
  player's `bytes_queued` / `state` for commit. Self-
  referential complexity is real, even if Option A's closure
  scope sidesteps it: the token's lifetime ties to `&mut
  self`, and `commit()` needs to mutate `self.bytes_queued` —
  which is fine in Rust, but the token's design has more
  moving parts than Option A.
- Two-step API (poll → acquire → commit) is more ceremony than
  one-step (poll_refill closure).

### Option C — Defer indefinitely; close as discretionary

Mark DCB-02b-A as a "nice-to-have" that doesn't merit the
breaking API change. The DCB-02b retrofit already type-tracks
the *cache* discipline; the residual raw pointer in
`NeedRefill` is contained (single consumer in the rlvgl tree;
the consumer's `unsafe { copy_nonoverlapping }` is the only
unsafe bit). INV-D9 isn't compromised — `NeedRefill::buf` is
the inactive bank's address derived from the (typestate-
tracked) `DbufRead`, not a free-floating DMA buffer.

**Pros**: zero churn.

**Cons**: leaves a residual `unsafe { ... }` block in the
disco binary's audio path that the type system *could* cover.
Not consistent with DCB's "if the type system can express it,
let it" philosophy.

## 4. Recommendation

**Option A** (callback-based refill).

Justification:

- **Sidesteps the self-referential token complexity.** The
  closure scope IS the bank-guard scope; lifetimes flow
  naturally. No raw-pointer escape hatch on the consumer
  side.
- **Standard Rust idiom.** `with_*`-style APIs (e.g.
  `RefCell::borrow_mut`, `Cell::with`, scoped thread APIs)
  are the established way to expose a scoped resource to
  user code. Audio refill matches this shape exactly.
- **Single consumer, mechanical update.** The disco
  bare-metal binary is the only in-tree consumer. The
  `unsafe { copy_nonoverlapping }` becomes a safe
  `buf[..to_copy].copy_from_slice(...)` (or stays `unsafe`
  for the source pointer but the destination is now a
  safe slice). One file's-worth of refactor.
- **PollResult simplifies.** `NeedRefill` variant goes
  away; pattern-match becomes a 3-arm match on Idle /
  Playing / Finished.

Option B is rejected on the same grounds DCB-02-A rejected
its Option C: more API surface than the simpler shape, with
no observable benefit. Option C is rejected because it leaves
a real `unsafe` block in disco firmware that the type system
can cover with one mechanical refactor.

## 5. Proposed amendments

### DCB-00 §10 audio_player row — clarification

Add a 2026-05-03 follow-up note: "DCB-02b-A 2026-05-03 closes
the residual raw-pointer write path in `PollResult::NeedRefill`
by replacing the legacy `poll()` + `refill_done(pcm)` API with
a callback-based `poll_refill<F>` method. The cache typestate
discipline (DCB-02b) is unchanged; only the byte-write path
becomes safe-slice-based."

### `audio_player.rs` API change

```rust
// PollResult — drop NeedRefill variant
pub enum PollResult { Idle, Playing, Finished }

// Replace poll() + refill_done() with:
pub fn poll_refill<F>(&mut self, refill: F) -> PollResult
where F: FnOnce(&mut [u8], u32) -> usize;
```

The internal implementation uses the existing BankGuard
pattern: bank_guard at construction (no cache op for Read),
caller's closure writes via `guard.as_mut_slice()`,
`release(&mut ctx, current_target)` emits the clean.

### §15 ratification entry

Standard sub-letter resolution shape; documents the §10 row
clarification + the API change. Not a Standards Action change
to the typestate set or invariants — only the consumer-facing
audio_player API surface.

## 6. Implementation plan summary (informative)

If Option A is ratified:

- **DCB-02b-A2** — Replace `poll()` + `refill_done()` with
  `poll_refill<F>` in `platform/src/audio_player.rs`. Drop
  `PollResult::NeedRefill` variant. Update consumer at
  `examples/stm32h747i-disco/src/main.rs:3266` to use
  closure-based refill. Verify embedded build.

## 7. Change log

- **2026-05-03 — Drafted.** Surfaced during the post-DCB-04
  cleanup pass when reviewing the residual `unsafe { ... raw
  pointer ... }` blocks left over from DCB-02b's "minimum
  retrofit" shape. Recommendation: Option A (callback-based
  refill API). Awaiting owner ratification via a DCB-00 §15
  amendment.
- **2026-05-03 — Resolved.** Option A ratified by owner
  go-ahead ("I agree with A — it fits the style of the
  library"). Resolution folded into DCB-00 §10 (audio_player
  row updated to document the post-DCB-02b-A `poll_refill<F>`
  shape; references DCB-02b cache discipline + DCB-01b-A
  release-side placement) and §15 (closure-based-refill
  amendment entry; not a Standards Action change to the
  typestate set or invariants — only the consumer-facing
  `audio_player` API surface). DCB-02b-A2 (the
  implementation) is now unblocked. This sub-letter is now
  historical record only.
