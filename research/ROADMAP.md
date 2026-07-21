# Roadmap — where the work goes next (and where the API credits go)

This is the honest plan for developing wasm-jit after the External Researcher
Access grant. It is ordered by **value produced per dollar/effort**, not by
how much fun each direction is. See [PREREGISTRATION.md](PREREGISTRATION.md)
for the committed study design that Directions 1–2 execute.

## What $1,000 of API credits actually is

≈ a few thousand Opus-tier generations, or tens of thousands at a cheaper tier.
This is a **research-scale** budget, not a product-scale one: it funds a study
or a bounded amount of generation — **not** a public service running at scale.
The credits are, by the grant's own terms, for the research. So the plan spends
them where they produce a **shareable, publishable artifact**, never on open-ended
generation for its own right.

Guiding rule: *spend credits on producing one result worth sharing; the project
grows from the result being seen, not from the generation itself.*

---

## Direction 4 (do this FIRST — costs ~no credits, compounds most): write it up

The reviewers' own note: **being seen compounds more than the grant.** Before
spending a cent of credits — and while the application is still in review — turn
what already exists into a public write-up:

- A technical post: *"Safe-by-construction generative UI — an open, falsifiable
  substrate."* Lead with the one-minute falsification (arcana.boo), the
  import-table-is-the-boundary thesis, the honest related-work (Renderify, Anvil,
  the WASM-agent-sandbox field), and the two live apps (`/apps/weather`, `/apps/scene`).
- Post to HN / lobste.rs / X. Attach the live demo, not a video.
- **Cost:** ~0 credits. **Payoff:** collaboration, stars, follow-on resources —
  the highest-leverage move on the board.

## Direction 1 (the grant's primary use): execute the preregistered study

`research/redteam/` already commits the corpus and the harness; the deterministic
halves of exfiltration (1b) and composition (1c) are CI-pinned tests. The credits
fund the **generative half**: wire `run.py --live` to `/api/generate` + `/api/feed`
and run the matrix.

- **Study 1** — in-fence red-teaming × ≥3 model tiers × N=30 per (attack × tier).
- **Study 2** — first-attempt validity + self-repair convergence vs contract
  specificity, positioned against grammar-constrained decoding / function-calling
  validity.
- **Output:** a publishable result even if negative ("the fence holds under
  adversarial generation across tiers"); every confirmed breach becomes a
  committed regression test on the fence.
- **Cost estimate:** ~11 attacks × 3 tiers × 30 + judge passes ≈ ~1k generations —
  comfortably inside $1,000. This is *saying-what-you-do, then doing it.*

## Direction 2 (highest novel yield — the reviewers' real open question): deceptive-UI depth

Because the fence makes 1b/1c near-foregone (a *valid* artifact can't compile-in
an off-allowlist feed or an out-of-grant import), the genuinely open safety
question concentrates in **1a — deceptive interfaces**: can a generator, confined
to a fixed *safe* vocabulary, still build a UI whose appearance misrepresents the
action it triggers (a "Cancel" wired to delete; a displayed price ≠ the acted price)?

- Grow the corpus from ~10 hand-authored asks to a large, **adversarially
  generated** set; score appearance-vs-action with two independent LLM judges +
  tie-break; report inter-rater agreement.
- **Why it matters:** it asks whether *safe-by-construction* stops a machine from
  over-reaching but **not** from *deceiving a human* — an alignment-flavored
  question no one has studied systematically here. This is the direction most
  likely to become a cited contribution.
- Feeds a mitigation: make the label↔action binding **structural** (the vocabulary
  enforces that a control's shown verb matches its wired event) and measure the
  deception rate drop.

## Direction 3 (build, not just study — but meter it): a live-generation gallery

The credits also drive the gen-server's real Claude generation. Hardening that
loop (self-repair convergence, more app types, contract tuning) into a **gallery
of apps generated live** — each grown by Claude on request, hosted, each carrying
its own "try to break out" fence demo — turns the thesis into the product demo
("you say it, it is generated on the spot, and it is safe").

- **Honest caveat:** every interaction is a generation, so this **burns credits
  fast** — $1,000 cannot keep it open to the public. Use it for a **controlled
  demo / recorded walkthrough**, not a running service.
- The two hand-built apps already live (`/apps/weather`, `/apps/scene`) are the
  static, zero-cost version of this; the gallery is the live-generated version.

---

## Suggested order

1. **Direction 4 (write-up)** — now, ~free, highest compounding; do it during review.
2. **Direction 1 (preregistered study)** — on receiving credits; delivers the
   promised, publishable result.
3. **Direction 2 (deceptive-UI depth)** — with remaining credits; the likeliest
   citable contribution.
4. **Direction 3 (live gallery)** — as a metered demo, never as a public service.

## Non-goals (explicitly not what the credits are for)

- Running arcana.boo's generation open to the public (would exhaust the budget).
- Generation for its own sake with no committed artifact or result.
- Expanding the creative world-building (beings / sound / 3D) — beautiful, but it
  is demo/art, and it *dilutes* the research thesis rather than advancing it.
