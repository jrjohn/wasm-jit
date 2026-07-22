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
- **Memory, host-mediated.** A being now carries a complete, persistent storehouse (PostgreSQL, with lexical and semantic recall), yet the cell never receives a database handle, a query string, or a connection — the host recalls scoped text and feeds it in, keyed hard to `(owner, soul)` and enforced at the tenant, query, role, and database levels. A crowd can populate a hundred worlds and every being recalls exactly its own, never another's. The fence extends from what a cell may *execute* to what it may *remember*, unchanged in shape: the host lends what the cell may never hold. The same pattern recurs for reach (the host fetches allow-listed sources; the cell never gets `fetch`) and for the drawing commons below — one principle, applied to execution, memory, vocabulary, and reach alike.
- **Voice.** Beings speak and sing through an 11-word sound fence: formant speech synthesis built from six host "physics" primitives (a raindrop's ping, a bell's modal partials, a glottal pulse through vowel formants), where the seed decides only *when*. A zhuyin→gesture compiler turns Mandarin's **closed set of 37 phonetic symbols** into unbounded speech — vocabulary composition at its purest. The audio engine itself ships as a wasm module whose **import section is empty**, and a CI test machine-verifies `imports == 0`: it can touch nothing; it can only vibrate.
- **Body.** Beings carry true 3D bodies through a 30-word scene fence with the same placement law — the host pins the base transform to the being's position; a body can shape itself but cannot teleport itself.
- **World law over seed discipline**, throughout: spatial audibility (walk closer, hear more), terrain occlusion, and habitat confinement (a fish's step onto dry land does not land) are enforced by the host, never requested of the generated code.
- **Runtime integration**: fuel metering (≈0% overhead on numeric kernels) and bit-level deterministic replay (a 121-frame session replays bit-identically) — standard techniques, integrated end to end.
- **The generation loop**: Claude runs in a sandboxed container generating schemas + seed scripts; the server compiles and validates every artifact *before* manifestation, with a self-repair loop on compiler errors.

## 3. Worked evidence — a public, in-use substrate (2026-07-20 → 22)

In an afternoon of iterative requests against the running world, an entire scene was brought to life: birds that fly free on individual paths, a fisherman who breathes, a boat that rocks on the water, wind that gusts near-and-far, a bridge you walk up and over, and a look-up night sky with a fixed moon, a Milky Way and drifting clouds.

Every *creature-and-scene* change was expressed by **composing the existing vocabulary** — the seed language's `mv / sin / cos / move / rot / box / sphere / breath` plus the world's declarative attributes — and the capability fence never widened: **not one new import was added to any cell.**

The handful of genuinely new *world laws* (a first-person pitch, a celestial dome, facing-your-direction-of-travel, a walkable surface) landed in the trusted host, and were then pushed back toward data — a walkable deck became a declared attribute, the moon a declared sky-body a world opts into.

Most tellingly, when a population needed variety — three fish, three lotuses, two crows that must not move in lockstep — the fix was **not** three hand-written scripts but the substrate's own *same-dharma-different-karma* mechanism: one shared behaviour and one shared body per species, with all diversity carried in each being's **innate birth-seed**. One template, an unbounded population; yet every individual still reduces to the single, auditable dharma.

This is the richness-vs-reach orthogonality as a lived result rather than a claim — an afternoon of enrichment at *zero* capability expansion. It also surfaced the fence's fail-safe posture in passing: a malformed body seed does not execute — it is refused at compile and the being degrades to a placeholder, never to arbitrary broken behaviour.

**And it kept holding as the substrate went public.** In the two days after (2026-07-21 → 22), the world grew memory (a per-being storehouse, isolated by `(owner, soul)`), transmigration across worlds, beings that act unbidden and converse with one another — each remembering, in its own isolated storehouse, what a *named* other said to it — and a drawing vocabulary that any being can extend and any other reuse by name. Not one new import was added across the whole of it: the orthogonality was not a one-afternoon fluke but survived sustained enrichment. Meanwhile the substrate stopped being private. As of this writing, **14 distinct people have signed in and built worlds; the drawing vocabulary has been extended 13 times by contributors whose only reviewer was the compile gate; and the fence has logged 23 refusals against live traffic.** That last number matters most: the red-team corpus Study 1 proposes to build is already beginning to populate itself from real use — every refusal is the invariant doing its job, recorded, in the wild, before any credited study begins.

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

That last sentence is the claim this architecture exists to make, and it is the one a code-generating platform structurally cannot make. Their community components are arbitrary code, so every contribution needs a human to read it before it can be offered to anyone else — which is why such libraries are either curated bottlenecks or unreviewed hazards. Here the bottleneck is a compiler, and it does not get tired. This is no longer only an argument: the open library has already taken **13 contributed words** through that gate with no human in the loop, and the **23 refusals** logged against live traffic are the same gate turning away what it should.

**Identity, and what it is and is not for.** Contribution is authenticated (Google Sign-In, tokens verified server-side against Google's published keys, audience-checked). Reads stay open to everyone. Identity is *not* what makes contribution safe — the fence already does that, and would still do it for an anonymous contributor. Identity is for the ownership check (letting an author revise their own work and nobody else's), for the operator to reach a contributor, and for the quota that a model-backed tier will eventually need. It is deliberately *not* public: a saved world carries its author only in a server-side field used for the ownership check, stripped from every public read, so one visitor never learns who made another's world; and the visitor count is a salted hash, with the email kept in a separate operator-only directory that no endpoint serves. It is worth being precise about this, because "we require login" is often offered as if it were a security property. It is not one. **The fence is what holds; the login only says who to thank — and only the operator is thanked.**

**The honest consequence.** Once strangers' words really are composed into worlds that appear on other people's screens, "what can a *valid* artefact still do inside its grants" is no longer a thought experiment — it is the thing standing between an open vocabulary and a deceptive interface with someone else's name on it. That is precisely what Study 1 measures, and why it is the primary study rather than the interesting one.

## 4c. What the fence does not bound — reach vs. truthfulness

A structural point worth stating plainly, because it is the precise edge of the whole approach: a compile-time capability check bounds what an artifact can **touch**, not whether it tells the **truth**. Perception can be fenced — a being sees only what the host shows it, and cannot reach past that to invent a fact about the world it has no channel to. Honesty cannot be compiled. A being asked to describe its situation can still misdescribe it, and no import-section audit will catch that, because there is no capability being exceeded — only a claim being made.

This is exactly why Study 1's "deceptive interface" category is the one axis that cannot be scored automatically and needs human adjudication: deception lives in the gap the fence structurally cannot close. Naming that gap is not a weakness of the claim — it is the boundary of what "safe by construction" can honestly assert. The fence guarantees that a generated artifact *cannot exfiltrate, cannot escalate, cannot touch what it was not given*; it does not guarantee that the artifact is *truthful about what it is doing*. The first is a property of the substrate and is provable; the second is a property of the content and is exactly what Study 1 exists to measure.

## 5. Where this sits — related work, and the honest gap

**No single ingredient here is new.**

- Capability-based WASM sandboxing of AI-generated code is an active, crowded space (NVIDIA, Cosmonic, the WASI capability model).
- Runtime DSL→WASM via `wasm-encoder` is an established compiler-smith pattern. Runtime WASM *generation* itself is not new either — Andy Wingo's `wasm-jit` proof-of-concept has a running WebAssembly program generate new WASM modules at runtime and late-link them in for a JIT speedup. But that is runtime codegen for **performance**; no one has turned runtime WASM generation into a **capability fence** for AI-generated code, with the compile-time import audit as the security boundary. Others generate WASM to run *faster*; here it is generated to run *safely*.
- "A small language that is safe by construction" has a named academic instance — **Anvil**, a restricted DSL where every well-formed program satisfies execution-safety properties by construction (scalars, arithmetic, bounded loops; no pointers, arrays, heap, or recursion).
- On the product side, generative-UI tools (v0, bolt, Lovable) have the model emit ordinary code you run as-is — their sandboxing does not survive deployment.
- The closest research cousin, **Renderify**, sandboxes LLM-generated JSX in the browser via *seven layered defenses* (defense-in-depth).
- **Capability-secure JavaScript already exists — this is the honest peer.** The object-capability model (Miller, *Robust Composition*, 2006), Google's **Caja**, and today's **SES / Hardened JavaScript / Compartments** (TC39, Agoric) run untrusted JS under default-deny, no-ambient-authority confinement, in *pure JS with no WebAssembly*. So "you need WASM for a fence" is **not** a claim I make.

My contribution is therefore not "a fence where none existed," but a *specific composition* none of the above make together. Where SES confines **arbitrary JS logic** under a hardened realm, a cell can only express a **fixed capability vocabulary** — so here *what can be written* is bounded, not merely what it may reach. That artifact is a WASM cell whose **import section is its entire capability surface**: a machine-checkable audit of a few dozen bytes (not a browser security model, not a shim), *independent of the generator's intelligence* (a stronger model cannot widen a grant it was never given), and *monotonically attenuating across recursive generation* (a child's grants are a subset of its parent's — a property no JS-sandbox composition offers by construction).

This is a synthesis-and-framing contribution delivered as an open, reproducible, CI-pinned, one-minute-falsifiable substrate — explicitly **not** a new sandbox primitive.

## 6. Why not just generate JavaScript? (the sharpest objection, answered honestly)

For a *human* building a scene like the live demo, plain JS — Three.js / WebGL — is easier and more flexible; the seed substrate adds friction, not ease, and I say so plainly.

But that is the wrong axis of comparison. The substrate does not compete with hand-written graphics code. It earns its keep only when two conditions hold *together*: the author is a **model**, and the output **runs immediately, untrusted**.

And the honest comparison is *not* with naïve `eval`. Generated JS is just as **instant** whether you `eval` it or compile a cell — **speed is not the difference**; the difference is what runs in that instant. The real alternatives are the ones the industry already uses:

- **iframe sandbox + CSP** — how v0, bolt, and Claude Artifacts run generated UI today (this very document renders in one), and for "run untrusted UI" it is often *good enough*. But it is default-*allow* minus mitigations: safety rests on several browser flags plus a Content-Security-Policy set correctly, and CSP is empirically fragile — Weichselbaum et al. (ACM CCS 2016) found the large majority of real-world CSP policies trivially bypassable. It also does not compose across generation, and answering "what can this reach?" means reasoning about the whole browser security model.
- **Capability-secure JS (SES / Compartments)** — genuinely default-deny, in pure JS, no WASM. But it confines *arbitrary logic*: the untrusted code can still compute anything it likes, over a larger trusted base (the SES shim plus realm isolation).

Against both, the cell's edge is narrow but real: it is **not arbitrary code but a fixed vocabulary**, audited over a **few-dozen-byte import section** rather than a browser model or a shim, **attenuating across generation**, and — the axis that matters most as models scale — **independent of the generator's intelligence**. "Review every generated line" and "it probably will not misbehave" both weaken as the generator gets stronger, or gets prompt-injected; a structural bound a stronger generator *cannot widen* does not.

### When you do NOT need this

If a human writes the code, or every generation is reviewed before it ships, or it only ever runs as throwaway UI with no access to private data — plain JS behind an iframe+CSP sandbox is simpler and usually sufficient, and I say so plainly.

The target is the sharper case: generated code that must **compute over data that should not leave the machine** — where a fence with *no network primitive* is a stronger guarantee than a bypassable CSP — or that composes strangers' contributions onto other people's screens, where **attenuation, not per-component review, is what scales**. There the safety question is not "is the policy configured right" but *what the just-generated artifact can structurally touch*.

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
