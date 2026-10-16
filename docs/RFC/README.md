# RFC process

Design before code. Every significant decision in `jilva-rs` starts as an RFC.

## Lifecycle

`Draft` → `Discussion` (min. 1 week) → `Accepted` | `Rejected` | `Postponed`

- **Draft** — open a PR adding `docs/RFC/NNNN-title.md` from the template. Number = next free.
- **Discussion** — announced in Ideas; review focuses on flaws, alternatives, prior art.
- **Decision** — a maintainer merges (accepted) or closes with reasons. Accepted RFCs may still change — amendments are new short RFCs.

## What needs an RFC

- Public API shape, component model, theming system, MSRV, dependency policy.
- Anything a user would have to unlearn if we got it wrong.

## What does not

- Typos, wording, repo chores, prototype code.

## Rules

- One proposal per RFC. Small wins.
- Include prior art (what do Ratatui / Cursive / Bubble Tea / Textual do here?).
- Include non-goals and open questions explicitly.
- Prototype code may accompany an RFC but never substitutes for it.
