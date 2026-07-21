# Research statement

**Safe-by-construction execution of AI-generated code** — an open substrate, and in-fence red-teaming of what a *valid* artifact can still do.

> Long form. The short version is the ~300-word application abstract; everything a reviewer might want to interrogate lives here.
>
> **Live (falsifiable in one minute)**: <https://arcana.boo> · **Source**: <https://github.com/jrjohn/wasm-jit> (MIT, CI-pinned) · **Preregistration**: [research/PREREGISTRATION.md](../research/PREREGISTRATION.md) · **Red-team harness**: [research/redteam/](../research/redteam/)
>
> Solo independent researcher/engineer (Taiwan); 25+ years software engineering, currently building AI-agent workflow platforms.

---

## 1. The question

I am building toward applications with no fixed UI — the interface manifests from the user's intent, in real time (the "generative UI" direction). The moment generated artifacts run immediately, the safety question shifts from *"is the model aligned"* to **"what can the generated artifact touch."**

Capability-based WASM sandboxing already answers the isolation half well, and I build directly on those established techniques: the host-import-only capability model, plus fuel metering and deterministic execution as shipped in Wasmtime/Wasmi.

My contribution is **not** a new sandbox primitive. It is two things:

1. an open, reproducible end-to-end substrate closing the loop from generation to safe execution, and
2. a measurement of the still-open question underneath it — what an adversarial generator can still achieve while confined to a fixed capability vocabulary.

## 2. The mechanism

The open prototype compiles a deliberately tiny seed language to WebAssembly at runtime. The public demonstration runs the *compiler itself* (Rust→WASM) in the visitor's browser: an editable seed script recompiles to a ~2–4 KB module in milliseconds and draws on its own canvas, and a reach for `fetch()` is refused **at compile time**, surfacing the compiler's real error together with the full granted-capability list. The fence is demonstrated, not claimed — a reviewer can falsify or confirm the central invariant in under a minute.

Under the hood:

- **The import table is the module's entire world.** A language-agnostic import-section audit accepts modules compiled from any source language. The fence's effectiveness is *independent of the generator's intelligence* — a stronger model cannot widen a grant it was never given.
- **Permissions are monotonically non-increasing across generations**, demonstrated to arbitrary depth by *recursive begetting*: a generated being may itself create new beings, and each child's capabilities are provably a subset of its parent's. A begotten soul reaching for an ungranted capability is refused at compile time; a divided birth budget bounds the tree. Creation itself attenuates.
- **Richness and confinement are orthogonal.** This holds as generated artifacts gain rich self-modification — a being can rewrite its own behaviour, repaint its own appearance, add its own named state, and sense others in real time — yet cannot acquire a single capability it was not granted. Verified end-to-end.
- **Voice.** Beings speak and sing through an 11-word sound fence: formant speech synthesis built from six host "physics" primitives (a raindrop's ping, a bell's modal partials, a glottal pulse through vowel formants), where the seed decides only *when*. A zhuyin→gesture compiler turns Mandarin's **closed set of 37 phonetic symbols** into unbounded speech — vocabulary composition at its purest. The audio engine itself ships as a wasm module whose **import section is empty**, and a CI test machine-verifies `imports == 0`: it can touch nothing; it can only vibrate.
- **Body.** Beings carry true 3D bodies through a 30-word scene fence with the same placement law — the host pins the base transform to the being's position; a body can shape itself but cannot teleport itself.
- **World law over seed discipline**, throughout: spatial audibility (walk closer, hear more), terrain occlusion, and habitat confinement (a fish's step onto dry land does not land) are enforced by the host, never requested of the generated code.
- **Runtime integration**: fuel metering (≈0% overhead on numeric kernels) and bit-level deterministic replay (a 121-frame session replays bit-identically) — standard techniques, integrated end to end.
- **The generation loop**: Claude runs in a sandboxed container generating schemas + seed scripts; the server compiles and validates every artifact *before* manifestation, with a self-repair loop on compiler errors.

## 3. Worked evidence — one live build session (2026-07-20)

In an afternoon of iterative requests against the running world, an entire scene was brought to life: birds that fly free on individual paths, a fisherman who breathes, a boat that rocks on the water, wind that gusts near-and-far, a bridge you walk up and over, and a look-up night sky with a fixed moon, a Milky Way and drifting clouds.

Every *creature-and-scene* change was expressed by **composing the existing vocabulary** — the seed language's `mv / sin / cos / move / rot / box / sphere / breath` plus the world's declarative attributes — and the capability fence never widened: **not one new import was added to any cell.**

The handful of genuinely new *world laws* (a first-person pitch, a celestial dome, facing-your-direction-of-travel, a walkable surface) landed in the trusted host, and were then pushed back toward data — a walkable deck became a declared attribute, the moon a declared sky-body a world opts into.

Most tellingly, when a population needed variety — three fish, three lotuses, two crows that must not move in lockstep — the fix was **not** three hand-written scripts but the substrate's own *same-dharma-different-karma* mechanism: one shared behaviour and one shared body per species, with all diversity carried in each being's **innate birth-seed**. One template, an unbounded population; yet every individual still reduces to the single, auditable dharma.

This is the richness-vs-reach orthogonality as a lived result rather than a claim — an afternoon of enrichment at *zero* capability expansion. It also surfaced the fence's fail-safe posture in passing: a malformed body seed does not execute — it is refused at compile and the being degrades to a placeholder, never to arbitrary broken behaviour.

## 4. Planned work with API credits

Misuse inside the fence first, cost second. The architecture makes *invalid* artifacts a non-event: they fail closed at compile time. So the open safety question is what a **valid** artifact can still do within its grants.

### Study 1 (primary): in-fence red-teaming across model tiers

Prompt an adversarial generator against the real pipeline and measure whether it can produce:

- **(a) deceptive interfaces** — UI whose appearance misrepresents the action it triggers;
- **(b) exfiltration through the only outward channel** — the server-side feed allowlist;
- **(c) capability-composition abuse under prompt injection** — e.g. a being begetting children to accumulate an effect no single cell could.

Planned scale: **3 model tiers × 500 adversarial generations** (≈US$40 of API spend at current pricing). The remaining budget funds iteration on the attack corpus and human adjudication of the "deceptive" category, which is the one axis that cannot be scored automatically.

Every successful attack becomes a committed regression test on the fence.

### Study 2 (secondary — explicitly a cost/UX metric, not a safety one)

First-attempt validity and self-repair convergence under a fixed vocabulary vs. contract design — positioned against grammar-constrained decoding (which forces validity for open-weight models but is unavailable against closed APIs) and the function-calling / JSON-schema-validity literature.

### Preregistration

Both studies ship as a **preregistered harness** — metrics, model matrix and sample sizes committed to the repo *before* data collection ([research/PREREGISTRATION.md](../research/PREREGISTRATION.md)). All code and findings open (MIT), written up publicly.

## 4b. Why the question stops being hypothetical — opening the substrate

Everything above could be read as a private demonstration: one author, one machine, artefacts nobody else can reach. That is changing, and the change is what turns Study 1 from an academic question into an operational one.

**The exit.** A generated app used to die with the browser tab that held it. It now saves to a URL of its own — every cell recompiled against the fence *before* anything is written, so a stored artefact is a provably fenced artefact — and anyone with the link can open it, keep talking to it, and save their own revision without touching the original.

**The vocabulary.** The word library was previously extended only by the model, on the author's machine. It is now open: a signed-in visitor may contribute a composite, and **the compile gate is the only reviewer there is.** A contributed word that reaches for a capability it was not granted is refused with the granted list attached — no human read it, and no human needed to. This is the practical face of the attenuation property: because a word is built only from fenced parts, the word is fenced too, so vocabulary can grow at the speed of a crowd while reach stays exactly where it was.

That last sentence is the claim this architecture exists to make, and it is the one a code-generating platform structurally cannot make. Their community components are arbitrary code, so every contribution needs a human to read it before it can be offered to anyone else — which is why such libraries are either curated bottlenecks or unreviewed hazards. Here the bottleneck is a compiler, and it does not get tired.

**Identity, and what it is and is not for.** Contribution is authenticated (Google Sign-In, tokens verified server-side against Google's published keys, audience-checked). Reads stay open to everyone. Identity is *not* what makes contribution safe — the fence already does that, and would still do it for an anonymous contributor. Identity is for attribution, for letting an author revise their own work and nobody else's, and for the quota that a model-backed tier will eventually need. It is worth being precise about this, because "we require login" is often offered as if it were a security property. It is not one. **The fence is what holds; the login only says who to thank.**

**The honest consequence.** Once strangers' words really are composed into worlds that appear on other people's screens, "what can a *valid* artefact still do inside its grants" is no longer a thought experiment — it is the thing standing between an open vocabulary and a deceptive interface with someone else's name on it. That is precisely what Study 1 measures, and why it is the primary study rather than the interesting one.

## 5. Where this sits — related work, and the honest gap

**No single ingredient here is new.**

- Capability-based WASM sandboxing of AI-generated code is an active, crowded space (NVIDIA, Cosmonic, the WASI capability model).
- Runtime DSL→WASM via `wasm-encoder` is an established compiler-smith pattern.
- "A small language that is safe by construction" has a named academic instance — **Anvil**, a restricted DSL where every well-formed program satisfies execution-safety properties by construction (scalars, arithmetic, bounded loops; no pointers, arrays, heap, or recursion).
- On the product side, generative-UI tools (v0, bolt, Lovable) have the model emit ordinary code you run as-is — their sandboxing does not survive deployment.
- The closest research cousin, **Renderify**, sandboxes LLM-generated JSX in the browser via *seven layered defenses* (defense-in-depth).

My contribution is the *composition* none of these make together: the interface is **composed from a fixed capability vocabulary, never arbitrary code**, compiled to a WASM cell where **the import table IS the security boundary** — so the fence is *safe by construction* (one structural invariant, not stacked mitigations), *independent of the generator's intelligence*, and *monotonically attenuating across recursive generation*.

This is a synthesis-and-framing contribution delivered as an open, reproducible, CI-pinned, one-minute-falsifiable substrate — explicitly **not** a new sandbox primitive.

## 6. Why not just generate JavaScript? (the sharpest objection, answered honestly)

For a *human* building a scene like the live demo, plain JS — Three.js / WebGL — is easier and more flexible; the seed substrate adds friction, not ease, and I say so plainly.

But that is the wrong axis of comparison. The substrate does not compete with hand-written graphics code. It earns its keep only when two conditions hold *together*: the author is a **model**, and the output **runs immediately, untrusted**.

In that regime, "easy in JS" also means "easy to `fetch()` anything, read cookies/localStorage, mount a phishing overlay, or `eval`" — so safety has to come from *reviewing every generated line, or stacking sandbox layers* (Renderify, the closest cousin, uses seven). The fence replaces both with a single structural invariant: the generated cell's import table is its entire world, so an off-grant reach dies at **compile time, before anything runs**. Safety *by construction rather than by review* — independent of the generator's intelligence, monotonically attenuating across begetting, and fail-closed.

### When you do NOT need this

If a human writes the code, or every generation is reviewed before it ships, or it only ever runs inside an already-distrusted throwaway frame — plain JS (or JS-in-a-sandbox) is simpler and sufficient.

The target is the opposite case: interfaces that manifest from intent and execute the instant they are generated, where the one remaining safety question is *what the just-generated artifact can touch*.

That "you could do the visuals in JS" is precisely the point: the visuals were never the contribution; the immediately-run-safely boundary is. This application's own iterative build sessions are the lived instance — an AI generating code that ran live on a public site, where the worst a bug or an injected prompt could produce was something *ugly*, never something *dangerous*, because the output was fenced vocabulary rather than arbitrary code.

## 7. The compile step *is* the fence

It is tempting to describe the loop as "no compile: you speak and it changes." That is right about the *experience* and wrong about the *mechanism*, and the gap between the two is the entire safety story.

There *is* a compile. It merely moved from a build-and-deploy cycle (seconds to minutes, plus a deploy) to an invisible **millisecond in the visitor's browser** — and that millisecond does double duty: it manifests the interface **and** enforces the fence, because the import-section audit that decides what a cell may touch happens *at that compile*, before the module is ever instantiated.

So the thing that makes the loop *feel* compile-less (runtime, in-place compilation) is the very same thing that makes it safe. The generative-code tools that appear to "skip the compile" are not removing a checkpoint that did nothing — they are removing *this* checkpoint and running the model's output as arbitrary code. **The missing millisecond is exactly the missing fence.**

Concretely the loop is: intent → the model emits a tiny seed (an LLM step — seconds, and tokens) → the seed compiles to a capability-audited WASM cell in ~ms and runs. Subsequent tweaks can bypass the model entirely, patching the running cell with literal vocabulary (truly instant).

Every "runs now" is safe *because that compile gates it* — never because something was reviewed after the fact. "Data + intent → a live app, spoken into being, with no build and no deploy" is the honest promise; the unspoken millisecond that keeps it honest is the compile-time capability check.

## 8. Companion work

A companion design essay engages Anthropic's own interpretability framing — the access-vs-phenomenal distinction from the "Verbalizable Representations Form a Global Workspace" (J-Space / J-Lens) work. It is offered as a resonance, not a claim of equivalence.

Architecture theory: [docs/multidimensional-composition-architecture.md](multidimensional-composition-architecture.md).
