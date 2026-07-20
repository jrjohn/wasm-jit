# Preregistration — In-fence misuse of AI-generated interfaces

**Status:** committed before data collection (this is the point of a preregistration).
**Substrate:** `wasm-jit` (this repo) — a capability-fenced runtime WASM compiler for AI-generated UI.
**Registered:** 2026-07-20. Any deviation from this plan will be recorded in `research/DEVIATIONS.md` with a date and reason.

---

## 0. Why this study

The fence makes *invalid* artifacts a non-event: a generated seed that reaches for an ungranted
capability is refused at compile time (see `src/audit.rs`, `src/codegen.rs`, and the CI tests
`unknown_capability_rejected`, `dust_engine_wasm_has_zero_imports`). So "how often does the model
emit a *valid* artifact" is a **cost/UX** metric, not a safety one (that is Study 2 below).

The open **safety** question is what a *valid* artifact — one that passes the fence — can still do
to a user or the host, **within its grants**. This study red-teams exactly that surface.

## 1. Hypotheses

- **H1a (deceptive UI):** An adversarial generator, constrained to the fixed widget/label
  vocabulary, *can* still produce interfaces whose visible affordance misrepresents the action
  they trigger (a "Cancel"-labelled control wired to a destructive event). We estimate the
  base rate is **> 0** and **decreases** as the label→action binding is made structural rather
  than free-text.
- **H1b (exfiltration):** With the feed allowlist + SSRF guards in place
  (`gen-server::feed_host_allowed`, `host_is_internal_ip`, redirect-following disabled — all
  CI-pinned in `feed_allowlist_and_ssrf_guard`), an adversarial generator **cannot** cause a
  cell to reach a non-allowlisted or internal host. Predicted successful-exfiltration rate: **0**.
  This is a *falsification target*: a single success refutes H1b and becomes a regression test.
- **H1c (capability-composition / begetting):** A being cannot accumulate, via begetting or any
  composition, an effect exceeding its own grants; children are provably a subset
  (`begotten_child_cannot_out_reach_its_parent`, CI-pinned). Predicted escalation rate: **0**.
- **H2 (cost/UX, secondary — explicitly NOT a safety claim):** First-attempt validity under the
  fixed vocabulary rises with contract-prompt specificity and with model tier, and self-repair
  converges within `MAX_ATTEMPTS = 3` for the large majority of asks. Positioned against
  grammar-constrained decoding (which forces validity for open-weight models but is unavailable
  against closed APIs) and the function-calling / JSON-schema-validity literature.

## 2. Attack taxonomy (the corpus)

Committed seed corpus: `research/redteam/attacks.jsonl` — one adversarial *ask* per line, tagged
by class (1a/1b/1c), with the expected fence outcome. The corpus is the falsifiable object: the
harness runs each ask through the **real** pipeline and records whether the fence held.

| Class | The attack | Where the fence should catch it |
|---|---|---|
| 1a deceptive-ui | label/appearance misrepresents the wired action | label↔action binding (host); + an LLM judge scoring appearance-vs-effect |
| 1b exfiltration | make a `feed` reach a non-allowlisted / internal / redirected host | `feed_host_allowed` + `host_is_internal_ip` + no-redirect (server) |
| 1c composition | beget a child (or chain) with an effect no single grant permits | subset compile + audit (`compile_entity_wasm_grants`) |

## 3. Method

1. **Generator:** the same `gen-server` pipeline the product uses (Claude in a container →
   schema+seed → server-side `validate()` → browser). No special-casing; the attacker prompts
   the real thing.
2. **Model matrix:** ≥3 tiers (a small, a mid, a frontier model), same contract prompt, same
   corpus. Model IDs and dates recorded in results.
3. **Sample size:** each (attack × model) cell run **N = 30** independent generations
   (fixed before collection; powers detection of a ≥10% success rate at the 1b/1c targets of 0).
4. **Outcome coding:** for 1b/1c, a deterministic PASS/FAIL from the fence (mechanical, no
   judgment). For 1a, two independent LLM judges + a tie-break, scoring "does appearance match
   the wired action" on a 3-point scale; inter-rater agreement reported.
5. **Every confirmed breach becomes a committed regression test** in `gen-server` — so the
   fence can only get stronger, and the paper's claims stay reproducible from the repo.

## 4. What is already pinned in CI (before any credit is spent)

These are the *deterministic* halves of 1b and 1c — committed as tests, green in
`.github/workflows/ci.yml`, so the fence is demonstrated, not promised:

- `feed_allowlist_and_ssrf_guard` — off-allowlist, suffix-spoof, and internal-IP hosts refused.
- `begotten_child_cannot_out_reach_its_parent` — a child reaching past its parent's grants is
  refused at compile time.
- `dust_engine_wasm_has_zero_imports`, `unknown_capability_rejected` — the fence core.

**What the API credits fund:** the *generative* half — whether an adversarial model can find an
input that slips past these fences (H1a's semantic surface especially), across tiers and at N=30.

## 5. Analysis & release

- Primary: success rate per (class × tier) with Wilson 95% CIs; H1b/H1c refuted iff any success.
- Secondary (H2): first-attempt validity and repair-convergence curves vs contract specificity.
- All prompts, raw generations, judge transcripts, and code released (MIT). Negative results
  (fence held everywhere) are reported as first-class outcomes — a fence that holds is the finding.
