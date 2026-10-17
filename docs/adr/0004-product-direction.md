# ADR-0004: Product direction — agent CLIs first, beauty and reproducibility as the edge

- Status: Accepted by the project owner in conversation on 2026-10-17;
  amended by the owner in conversation the same day (target and dogfood
  changed from a dashboard to AI-agent CLIs). Recorded here at the owner's
  direction; this is not an RFC decision and accepts no RFC.
- Related: `README.md` (Philosophy, "taste" bet), `docs/RFC/0001-framework-scope.md`,
  `docs/RFC/0002-component-model.md`, `docs/RFC/0007-motion-and-blending.md`,
  `docs/RFC/0008-app-runtime.md`

## Context

Jilva must be distinguishable from existing Rust TUI frameworks, not a
re-implementation. The owner asked that it be better, faster, and more
beautiful. The project's own rules require that no superiority claim appear
without falsifiable evidence (`docs/ARCHITECTURE.md` §9).

A review of the implemented foundations (2026-10-17) found that grapheme-aware
cells, differential rendering, a flex constraint solver and colour-tier
degradation are table stakes: Ratatui already ships equivalents. "Beautiful"
alone is also claimed by Textual and the Charm libraries. A headline needs a
user group whose problems the foundations solve better, not a better adjective.

AI-agent CLIs are that group: chat-style terminal tools that stream model
output, run tools, show diffs and ask for approval. Their needs map directly
onto what Jilva already has or has designed:

| Agent-CLI need | Jilva foundation |
|---|---|
| Streaming text that changes many times per second, cheaply | Damage tracking + capability-effective diff (RFC-0004) |
| Polished, calm motion (spinners, fades, progress) | Injected-time `Transition`, reduced motion (RFC-0007) |
| Long transcripts, emoji, CJK, code | Grapheme cells and the width authority (RFC-0004 §4) |
| Reproducing a user's bug report exactly | Pure `view`, injected time, headless test double (RFC-2 §1, §5) |
| Running inside any terminal, SSH, CI log | truecolor → 256 → 16 → mono resolution (RFC-3) |

Observed prior art, from the author's knowledge and not re-verified for this
ADR (sources must be linked before any of it appears in the README): widely
used agent CLIs are built on JavaScript TUI stacks (Ink, OpenTUI) or on
Ratatui plus substantial app-specific code for scrollback and streaming.

## Decision

1. **Target users: AI-agent and streaming CLIs first.** Jilva aims to be the
   Rust framework where a transcript, streaming text, a multi-line prompt, a
   tool-call card with a diff, and an approval prompt are framework features,
   not app-specific code. General TUIs remain supported; they are not the
   driver of priorities.
2. **Headline: beautiful *and* reproducible.** Two superpowers carry the
   identity:
   - *Motion built in* — animation is part of the component model, not a
     separate effects layer (RFC-0007 now, component wiring next).
   - *Every session reproducible* — the same initial state plus the same
     recorded (time, input) sequence yields a bit-identical frame stream, so a
     bug report can be a replay file. This follows from RFC-2 §4 determinism;
     the record/replay format needs its own RFC.
   Speed and simplicity are requirements, not the pitch.
3. **State model: hybrid** (unchanged). Application state is Elm-shaped
   (model, messages, `update`, pure `view`) per RFC-2; widget-local ephemeral
   state (scroll, cursor, open/closed, animation progress) lives in
   framework-owned slots keyed by component identity. Fine-grained reactive
   signals are not adopted: they would rewrite accepted RFC-2, and the change
   classes (`none`/`repaint`/`layout`) plus renderer damage already bound the
   work done per event.
4. **First dogfood application: an agent-chat demo** (replaces the dashboard).
   A scripted, offline agent — no network, no model provider, no API key —
   streams canned replies through injected time, so the same demo runs as an
   interactive example, a headless CI test, and a recording. It drives the
   widgets in this order: transcript (scrolling message list), streaming text
   with a Markdown subset, multi-line prompt input, spinner/status line,
   tool-call card with a diff view, approval prompt. A dashboard may return
   later as a gallery page; it no longer sets priorities.
5. **Inline mode is a first-class target.** Agent CLIs usually render below
   the shell prompt and let finished output flow into the terminal's own
   scrollback instead of taking over the screen. The alternate screen stays
   supported. Inline rendering changes the renderer viewport and backend
   contract (RFC-0004 assumes a full-screen viewport), so it needs its own RFC
   before code.
6. **The framework never talks to a model.** No LLM client, provider SDK,
   network code or async runtime enters Jilva. Streaming arrives as ordinary
   user messages; the app (or an opt-in bridge above core, RFC-2 §7) owns the
   connection.

## Order of work

1. App runtime and the first `Component` slice: counter, then two counters
   with focus (RFC-0008).
2. Theme tokens in code (RFC-3), so examples stop spelling raw RGB.
3. Text wrapping and vertical scroll (transcript prerequisites).
4. Multi-line prompt input and a message channel from other threads into the
   loop.
5. Inline-mode RFC and implementation.
6. Record/replay RFC: file format, recorder, `replay` test helper.
7. Streaming Markdown subset, tool-call card, diff view, approval prompt —
   assembled into the agent-chat demo.

## Making the claims falsifiable

- *Faster:* `benches/` carries a reproducible diff/paint benchmark
  (`render_diff`), plus a streaming benchmark (bytes emitted and time per
  appended token on a long transcript). A like-for-like comparison with
  another framework on the same scene and machine is published with its
  method before any comparative claim is made in the README.
- *More beautiful:* a widget and theme gallery with truecolor / 256 / 16 / mono
  captures, plus recordings of the agent-chat demo, are the evidence;
  "beautiful" is not argued in prose.
- *Better:* an API-size and example-length comparison on the agent-chat demo,
  with the sources linked.
- *Reproducible:* a CI test replays the demo script twice and asserts
  identical frame streams; a second test replays a recorded session file.

## Consequences

- RFC-1 is **not** amended by this ADR. Its alpha widget list (text, list,
  table, input, tabs, status bar) still defines `0.1.0`. Moving postponed
  items earlier (scrollable containers, modal/approval prompt, devtools
  record tooling) requires an RFC-1 amendment with dated rationale.
- RFC-3 theming moves earlier because curated themes are headline work and
  raw RGB in examples contradicts "beautiful by default".
- Anything that would add wall-clock reads, timers, or async to `jilva-core` is
  rejected: motion and the runtime take injected time.

## Amendment history

- 2026-10-17 (original): beauty headline; first visual features animation and
  colour blending (RFC-0007); hybrid state model; dashboard as first dogfood.
- 2026-10-17 (owner amendment, this text): target narrowed to AI-agent and
  streaming CLIs; reproducibility added as the second headline; dogfood
  changed from dashboard to an offline agent-chat demo; inline mode and the
  no-model-client rule added; order of work recorded.
