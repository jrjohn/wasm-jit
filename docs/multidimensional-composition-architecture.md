# Software Multi-Dimensional Composition Architecture — From Orthogonality to Shared Present-Moment Prediction

> A design-philosophy document. It records a twelve-turn discussion that pushes from "multi-dimensional composition" all the way to "manifestation is shared present-moment prediction."
> Through-line: every Buddhist / complex-systems metaphor is pinned to a **precise engineering coordinate**; every turn carries a **devil's advocate** that keeps it from degrading into mysticism.
> Date formed: 2026-06-25

---

## 0. The Whole Thing in One Sentence

> **Store the rule, not the net; let all things behold one another through a shared light without ever touching — and that light, in the end, is the prediction you and the one you interact with are mutually correcting in the present moment.**

Substance (體, the fractally-closed cell), function (用, the general-purpose executor that manifests as conditions arise), the store (藏, the ālaya that carries all karma), the net (網, Indra's mutual containment), self-organization (a living system emerging from local rules) — all of them fold, in the end, into a single present moment: **the canvas's manifestation is the not-yet-congealed, still-being-corrected prediction that AI and human hold jointly, right now.**

---

## 1. Multi-Dimensional Composition: The Hard Part Isn't "Composition," It's "Orthogonality"

The only legitimate reason for a composition architecture: to fight combinatorial explosion. With D axes of variation and k options per axis, the naive approach produces k^D hand-written variants; composition compresses the cost from the **multiplicative k^D** back down to the **additive k×D building blocks + a few irreducible interaction terms**.

**First principle: the defining property of a dimension is orthogonality** — a choice on axis A does not constrain axis B. In practice there are three kinds:

- **Truly orthogonal** (payment × theme × locale): any combination is valid, composition is free.
- **Correlated axes (fake dimensions)** (industry = healthcare ⇒ regulation = HIPAA): should **collapse into one**; forcing them apart = inviting a 3× interaction matrix onto yourself.
- **Semi-orthogonal (the real battlefield)**: most combinations work, a few are illegal. 90% of the design effort lives here.

**Three underrated pillars:**

1. **The interaction matrix is a first-class citizen**: a D×D table, each cell answering "is there an illegal combination / a coupling that needs coordination here." The more empty cells, the healthier. Illegal combinations must be rejected **at assembly time** (type error / fail-fast at startup), never deferred to runtime.
2. **Binding time differs per axis** (compile-time / build-time / deploy-time / startup / per-request), and this is a design decision. Picking the wrong binding time = the most common hidden debt in this class of architecture.
3. **A single composition root**: everywhere the dimensions converge collapses to one composition root; otherwise orthogonality exists in name only at the code level.

**Devil's advocate:** cross-cutting concerns (logging / auth / transactions) inherently pierce all dimensions and **should not be stuffed into the matrix as yet another dimension** — they live in the synapse layer (below), on a plane orthogonal to the dimensions.

---

## 2. Granularity and the Granularity Synapse: Both Cost and Intelligence Live in the Seam

Granularity is not "module size," it's **cutting resolution**. It decides whether composition can even hold: too coarse → a unit hard-wires multiple dimensional choices and can't be recomposed; too fine → synapse explosion, glue eats everything.

There is a **U-shaped cost curve**:

```
total cost ≈ N_unit × c_unit  +  N_synapse × c_synapse
```

**Core insight (the synapse-floor law): every synapse you cross carries a fixed overhead independent of unit size** (marshalling, validation, error handling, observability, the cognitive burden of tracing). `c_synapse` does not shrink as units shrink. Therefore:

> **How fine you can cut is decided by your cheapest synapse technology, not by domain imagination.**
> - in-process function call: floor ≈ 0 → can cut very fine
> - network synapse (microservices): floor = ms-scale latency + a whole family of failure modes → cut too fine and the synapse kills you back
> - organizational synapse (Conway): floor = a meeting → constrains service granularity

**The neural metaphor (used seriously): computation and learning live not in the neuron body but in the synapse.** Design rule: **units should be dumb and pure; synapses should be smart and the only entry point for change.** DI = rewiring synapses; feature flag = synapse gate; adapter = impedance-matching synapse; middleware = synapse cross-cutting layer.

**Impedance mismatch:** where heterogeneous granularities meet (ORM = object ↔ relational, API gateway = coarse-outside ↔ fine-inside) is a concentration of pain points. The larger the granularity gap a synapse spans, the more conversion responsibility it bears. **Don't chase uniform granularity across the whole system.**

**Devil's advocate:** an overloaded synapse → degenerates into a hidden unit (fat adapter, god mediator, ESB). Criterion: **a synapse should only route / transform / observe / adapt; the moment it holds domain logic, the granularity boundary exists in name only.**

---

## 3. Indra's Net: Phenomenon-and-Phenomenon Unobstruction "Through Principle," Not N²

The Four Dharma-realms (四法界) → four architectural layers:

| Dharma-realm | Architecture |
|---|---|
| Realm of phenomena (事法界, each thing distinct) | Units; boundaries are real, orthogonality is real |
| Realm of principle (理法界, one taste, equal) | Shared abstractions, protocols, types, invariants |
| Principle-phenomenon unobstruction (理事無礙, principle wholly present in each thing) | Interface and implementation: the abstraction lives whole inside each concrete instance |
| **Phenomenon-phenomenon unobstruction (事事無礙, each contains each)** | **Indra's Net itself — all the tension is here** |

**Core: the beads do not touch; a bead reflects the light, not the other beads.**

> Indra's Net read correctly: not N² bead-to-bead links, but N bead-to-light links. **Light = an O(1) shared substrate; the mutual reflection is emergent, not wired.** This dissolves §2's synapse floor: mediate through a common light and you get the *appearance* of "phenomenon-phenomenon unobstruction" while paying for only N synapses.

Engineering translation: Kafka / event log, a shared immutable substrate (CRDT / content-addressed / blockchain), a shared schema/protocol — **principle is the common denominator that lets the beads reflect one another.** The moment two beads bore a hole straight through and bypass the light, the net breaks.

**One-is-all (一即一切) = holographic, not omniscient**: self-similarity/fractal, event-carried state, the resilience of a hologram plate (cut off a corner and you still see the whole image — graceful degradation). "A single mote contains all defilement, purity, arising and decay" = a minimal type carrying the complete state algebra (making illegal states unrepresentable).

**Devil's advocate: the endless layer-upon-layer (重重無盡) must be terminable.** Indra's Net is a still, eternal, infinite mutual reflection; software must converge and must have a base case. The engineering version = **laziness**: the infinite net only unfolds to finite depth when queried. It is an ideal limit you approach, not a wiring diagram.

---

## 4. Fractal: Trade One Rule for Endless Structure

The essence of a fractal = **IFS (Iterated Function System)**: one rule applied to itself repeatedly, growing infinite detail. The point isn't that it looks nice, it's **the collapse of specification cost** — learn the rule once, understand every scale (cognition O(1), structure O(N)).

> **Don't enumerate, generate.** The richness of k^D comes not from writing k^D variants but from one self-applying grammar that grows them. **Endless layer-upon-layer = the infinite recursion of a rule; what you store is the rule, not the net; lazy evaluation realizes finite depth.**

A new metric: **fractal dimension D** — the rate at which new detail emerges as you zoom in:

- D too low → zoom in and there's nothing = a black-box monolith
- D too high → every layer explodes, the coastline never converges = over-coupled tangle
- Healthy → each zoom surfaces new detail that is structured, bounded, and follows the same grammar

Derived rule (coastline paradox): **boundaries should be low-dimensional (smooth, predictable contracts), interiors may be high-dimensional (rich implementation).**

**Devil's advocate: software is not a fractal, it's a multifractal.** A true fractal has no privileged scale; software has hard faults (in-proc → network: the synapse floor jumps from ≈0 to ms-scale). **At the break, the generative rule must change, because the cost model changed.** "The same pattern all the way down" is the sweetest lie.

> Self-similar within a scale band, rewrite the grammar at the discontinuities of the physical synapse floor.

Two asymmetries come along: **weighting** (power-law concentration, dense where the action is) and **base case** (the IFS touches ground at the leaf nodes and becomes real work — the leaves are where domain logic actually lives).

---

## 5. How to Fractalize: Force the Domain into a Monoid

> Find the shape that is **closed under its own composition operation**, rewrite the system as the recursive application of that operation to the leaves, and swap the operator at each physical floor.

Necessary and sufficient condition: **closed under composition (the part's interface == the whole's interface).** A list of lists is still a list → fractalizable; a controller of controllers is not a controller → not fractalizable, only layerable.

Algebraic kernel: make the cells form a **monoid / category** — associativity = scale invariance; identity element = base case / no-op. **Fractalization, mathematically, = how to force the domain into a monoid.**

Five steps: ① find generative cells closed under composition ② make the composition operator explicit and unique ③ nail down the leaves (base case) ④ mark the scale breaks and rewrite the operator at each break (same form, different operator) ⑤ weight by need, don't fractalize uniformly.

**aaf is a living multifractal:** `process → node → worker → agent → sub-agent`, every layer is sense→decide→act (same-form closure); the operator is rewritten floor by floor: in-proc / Kafka / process-spawn+CLI / Task fan-out.

**Devil's advocate:** forcing a fractal onto a non-closed domain = a floor covered in fake-uniform-interface ceremony that leaks. **Not all software is fractalizable — only the closed parts are.**

---

## 6. Causal Non-Determinism / Karma / Wondrous Function: Substance and Function Are Not Two

This turn is **function (用)**, the dual of the previous turn's **substance (體)**. Structure is dead and repeating; function is alive and determined by present conditions.

- **Causal non-determinism = causality is a partial order, not a total order** (Lamport / vector clock / CRDT). Causality is conserved (cause precedes effect), total order is open (concurrent events have no fixed precedence). **There is no global "now," only local causal light-cones.**
- **Karma (業) = an event-sourced accumulated history conditioning the present manifestation.** The present fruit = a fold over all past causes. The same input landing on different karma → a different manifestation. The non-determinism comes from differing karma, not from randomness.
- **Inexhaustible wondrous function / each leaf a Tathāgata (一葉一如來) / every point responsible for a different task = a homogeneous capability substrate + late-binding of role in the present** (general-purpose executor / actor become / FaaS). "Look only at where the manifestation is happening right now, and the function belongs to that point" = aaf's general-purpose executor: the role rides in the task payload, not baked into the agent.

**Three interlocking clasps:**

1. **Substance makes function possible**: closure (§5) makes "any point responsible for any task" safe — because all tasks are the same form (`Task → Result`), any homogeneous point can take any task without breaking the type contract. Bounded polymorphism: the task space is open, the interface is closed.
2. **Function is bounded by karma**: data gravity is the anti-homogeneity force. Work wants to go where karma has already piled up, not to "any point at all." **Data gravity = karma conditioning the point of manifestation.** The scheduler = reconciling demand locality with data locality.
3. **Non-determinism is quarantined in the synapse, determinism is kept in the cell**: log ordering across shards is non-deterministic, but replaying one shard's log is deterministic. **Non-determinism lives in the synapse (function/karma/scheduling); determinism is kept in the cell (substance/pure core).** The pure core must be pure precisely because non-determinism has to be quarantined outside it.

---

## 7. Ālaya: Storing All Seeds, Actualizing Selectively

The ālaya-vijñāna (阿賴耶識) = **a persistent shared seed-store = the true form of that light in Indra's Net.** "Every change at every point is recorded within it, and every node may draw on it" = the definition of a shared event store. **Karma is not in the beads, karma is in the ālaya;** the bead stays dumb and pure, the one store sits outside, and all points draw from it.

**The Eight Consciousnesses (八識) = a layered architecture:**

| Consciousness | Architecture |
|---|---|
| The first five (前五識) | I/O adapter / sensor (the edge touching the world) |
| The sixth, mind-consciousness (第六意識) | Present-moment processing / request-handler |
| The seventh, manas (末那識, grasping-at-self) | Aggregate root / partition key / session identity — draws the consistency boundary, and is also the contention bottleneck (the self = a lock) |
| The eighth, ālaya (阿賴耶識) | Append-only shared seed-store / event store |

**Seeds give rise to manifestation, manifestation perfumes the seeds (種子生現行,現行熏種子) = the breathing of CQRS + event sourcing** (read: replay/projection; write: append).

**The engineering solution to "letting go" (放下) = selective actualization, not deletion:**

> Store all seeds (**cause and effect are never lost**), but not all seeds actualize at once — the strong and the condition-matched ripen first (relevance / recency retrieval = RAG). **Letting go ≠ deletion; letting go = not everything actualizing.** The store is infinite, the actualized is finite; nothing is forgotten, not everything is live.

**Empirically confirmed:** the RAG distillation eval showed that replacing raw with a distilled representation → a 61% long-tail miss rate. Lesson = **the ālaya must not truncate the long tail; any cold seed may actualize under the right condition. Store everything, index by strength, actualize selectively.** (The experiment independently rediscovered the Yogācāra (唯識) law.)

**You already built a working ālaya:** the archive (every tool_use of every session → 15-minute ingest into PG = manifestation perfuming the seeds; any new session drawing on it via osearch = seeds giving rise to manifestation). A collective-karma shared store + asynchronous perfuming + read-heavy-write-light — a deliberate and sound design coordinate.

**Devil's advocate:**
- **Individual-karma vs collective-karma (別業/共業) fork**: one global seed-store (strong unity, SPOF + write contention) vs one ālaya per body (sharded, scalable, cross-stream partial order needs reconciling). Yogācāra's actual position is a hybrid: **private individual-karma store + shared collective-karma actualization = a per-aggregate private write stream + a shared read model.**
- **Store ≠ actualize**: a seed stored but unretrievable (daemon dead / not yet embedded) = karma present but not actualizing. **Storage was never the hard part; the retrieval quality of actualizing-on-condition is.**

---

## 8. The Canvas: Each Component Is an AI Agent

Manifest the previous seven turns onto one surface: **a canvas where each UI component is an AI agent; being triggered = actualizing-on-condition (the condition (緣) = the trigger); agents automatically interact with one another.**

Placement: the confluence of generative UI + actor model + blackboard + reactive dataflow. The most important ancestor is **the spreadsheet** (each cell is a micro-agent reacting to other cells — but deterministically, through dataflow).

**One load-bearing wall: components interact through the canvas, never call out to one another directly.**

- ❌ Direct interaction = the phenomenon-phenomenon-unobstruction collapse §3 warned about (fully-connected N², cascade explosion, storms)
- ✅ Mediation through the light: a triggered agent only writes its result back to the canvas; other agents subscribe to canvas slices and each reacts on its own

> **The canvas = a shared append-only seed-store = that light = the blackboard = the ant pheromone field (stigmergy).** N agents to 1 canvas, not N². "Automatically interact with one another" must be implemented as "react to canvas state changes," not "send messages to other agents."

Turn the eight turns of discipline into design rules:

1. **Layered manifestation** (synapse floor): most propagation is deterministic reactive dataflow (floor ≈ 0); only what genuinely needs generation crosses into the LLM. **Karma (cache) lets a triggered agent mostly actualize an already-ripe seed (memoization).**
2. **DAG propagation + bounded convergence** (base case): cycle detection / quiescence / max-depth, to prevent oscillation.
3. **Non-determinism quarantine**: the non-determinism the LLM generates is frozen into event-sourced state; once generated it becomes a replayable seed; the user sees a stable canvas.
4. **Manas boundary**: each component is a single writer writing only its own slice; it can only *propose* to others' slices, reconciled by the canvas (CRDT / single-writer-per-region).
5. **Ālaya store**: the canvas = an append-only event store = provenance + undo + explainability + the karma conditioning each agent.

**Devil's advocate: real-time ⊥ LLM latency.** The deterministic layer responds instantly + the generative layer streams back asynchronously (progressive). **Don't build the load-bearing wall on BPMN** — Kogito is a seconds-to-days long-process engine and can't hold up a real-time canvas; the canvas needs a front-end reactive signal graph, with LLM agents woken asynchronously behind it.

---

## 8.5 The Vectorized Rendering Substrate: Drawing Isn't the Bottleneck, Generation Is

Direct conclusion: **drawing is fast enough that it isn't the problem — but "is drawing fast enough" asks about the wrong bottleneck.** The bottleneck is **generation (LLM, seconds-scale)**, not **drawing (GPU, sub-millisecond)**. Vectorization is the right choice, but its value isn't "draws fast," it's upstream.

**"Two points make a line" is nearly free:** a line = 2 points = 16 bytes = 1–2 triangles / one `GL_LINES` primitive. A modern GPU comfortably draws **100k–1M lines** in a single 16.6ms (60fps) frame. What actually costs is not lines but: ① **Tessellation** (curve flattening / fill triangulation, CPU-side — caching already-tessellated geometry = the karma of the geometry layer) ② **Draw-call count** (10k independent calls slow, batched into 1 call fast) ③ **Text** (SDF / glyph atlas) ④ **Effects** (blur/shadow, orders of magnitude more expensive than lines).

**Vectorization is right — for these four reasons, not for "fast" (which is precisely the whole chain cashing out at the rendering layer):**

- **Cheap tokens (the key)**: an LLM can emit `line (0,0)→(100,100)` (a few tokens), it cannot emit a bitmap. **Vectorization is the precondition that makes LLM-generated UI *possible* at all**, hitting §9's token/latency floor head-on.
- **Composition-closed** (§5): point → line → shape → scene, a group of shapes is still a shape. Vectors are natively closed under composition = a fractalizable rendering substrate.
- **Scale-invariant** (§4): vectors don't blur on zoom — this **literally is** the scale-invariance of a fractal; raster can't do it.
- **Incrementally redrawable** (§8): retained scene graph + dirty region, redrawing only changed primitives = DAG propagation reaching the pixel layer, which is why the deterministic layer is instant.

**Real numbers (putting the bottleneck in the right place):**

| Action | Latency |
|---|---|
| GPU draws 100k lines | **<16ms** (one frame) |
| Redraw a dirty region | <16ms |
| Cache-hit static geometry | ~0 |
| **LLM generates that vector spec** | **0.3–15s ← the real bottleneck** |

Drawing is about **1000×** faster than generation. What to optimize is the generation side (streaming / karma cache / small models / component increments), not the drawing side.

**Devil's advocate — one trap + technology choices:**

- **❌ Don't build "vectorized UI" as SVG-in-DOM**: one DOM node per element, layout/style recalc dominates everything, **a few thousand nodes and it seizes up.** Thousands of agent components in SVG will certainly die.
- **✅ Fast path = GPU vectors**: WebGPU/WebGL (**PixiJS** / regl), **Skia-WASM (CanvasKit)**, native **Flutter (Skia/Impeller, the whole UI is vector, 60–120fps)**; animated vectors with **Rive**.
- **Best reference: tldraw** (React + retained vector scene, holds thousands of shapes steadily) = a ready-made template for a real-time agent-canvas rendering skeleton.

> **In one line: drawing is fast enough, vectorization is the right call — but the credit goes to "lets the LLM emit it + composition-closed + scale-invariant + incrementally redrawable," not to line-drawing speed. Keep the bottleneck at generation; for rendering, choose GPU vectors (not SVG-DOM).**

---

## 8.6 Faster Rendering: Render Less, Not Render Faster

For an AI canvas, "faster rendering" is mostly **not a faster rasterizer** — §8.5 already established that drawing is ~1000× faster than generation. **You have 1000× headroom before the rendering bottleneck; the methodology is "render less," not "render faster."**

**For this system, leverage from highest to lowest:**

1. **Patch/diff manifestation (biggest lever)**: the agent emits a delta (a mutation on the canvas), not a whole scene → **both generation (fewer tokens) and rendering (redraw only the dirty region) scale with the change size, not the whole canvas** (hitting §8's single-writer manas + §9's token economy). The fastest render is the smallest diff.
2. **Predictive pre-rendering (§11)**: while idle, pre-render the active-inference prediction ahead of the user's action → when the prediction hits, perceived latency → 0. The fastest render is the one you already finished.
3. **Karma-cache + memoized geometry (§9 + §8.5)**: cache already-rendered / already-tessellated output keyed by (intent, context), hit ~0; don't recompute static geometry (bypassing the CPU tessellation bottleneck).
4. **Streaming progressive render (§9)**: draw the vector spec as it streams, first-paint = when the first primitive arrives, not when the whole thing is generated.

> None of the four above is about "drawing faster" — all are "draw less / earlier / don't redraw." This is the rendering methodology for an AI canvas.

**The pure-rasterizer frontier (asked about, but mostly unneeded):** it only enters when you genuinely hit "100k+ primitives all changing every frame" (data viz / particle scale, which typical agent UIs are not) — **Compute-shader 2D** (**Vello** / Skia **Graphite**, GPU-parallel path rendering, utterly eliminating §8.5's CPU tessellation bottleneck), **immediate-mode** (egui / Dear ImGui, rebuild every frame, no diff), **compiling away the framework tax** (Svelte compiles to direct DOM updates / SolidJS / fine-grained signals — your Angular Signals is already on this path).

**Choice: retained vs immediate, by dynamic profile:** fully dynamic, everything changing every frame → immediate-mode; **mostly static + a few islands changing (= the agent-canvas profile) → retained scene graph + dirty-region + stable node identity (§8 manas) wins outright** (stable identity makes diffs cheap and caches hit). This canvas of yours is the latter.

**Devil's advocate:** optimizing the rasterizer is premature optimization (1000× from the bottleneck); **system speed is gated by generation, and swapping in a faster rasterizer moves perceived latency by zero** — and stays that way until generation is solved. Before adopting Vello-class tech, confirm you're really at that scale. Don't conflate "faster rendering" with "faster system": the first four move perceived latency, the rasterizer doesn't.

> **In one line: render less (patch diff), earlier (pre-render), don't redraw (cache + dirty-region), draw-as-you-stream (progressive) — not a faster rasterizer. Keep the bottleneck at generation.**

---

## 8.7 Reference Implementations: Reality Anchors for the Rendering Substrate

§8.5/§8.6's "GPU-vector retained scene + lookless primitive-tree controls" is not a fantasy; there are several mature implementations. Line them up to see which layer each maps to and where each stops:

| Implementation | Principle | Layer it maps to | Where it stops |
|---|---|---|---|
| **Delphi FireMonkey (FMX)** | Draws everything itself; lookless control = behavior + vector primitive tree (TStyleBook); pluggable backends (D2D/Metal/OpenGL/Skia); floating-point coordinates; unified 2D+3D | §8.5 rendering substrate + §5 "controls as composable cells" | A developer's hand-written **static tree**; no generation/karma/prediction/self-organization (2011 — six years before Flutter, proving the path viable) |
| **Flutter (Skia/Impeller)** | Same draw-everything; widget tree + RenderObject tree; GPU compositing; 60–120fps | §8.5 + §8.6 retained | Same as above; developer-authored tree |
| **tldraw** | React + retained vector scene; stable shape identity; dirty-region | §8.6 retained + stable identity (§8 manas) | Interactive editing, not agent generation (but the closest to an agent-canvas skeleton) |
| **Rive** | Vector + state-machine animation; lightweight runtime | §8.5 + TAnimation-style tween | Design-time authored state machine, not present-moment manifestation |
| **Vello / Skia Graphite** | Compute-shader 2D, GPU-parallel paths, no CPU tessellation | §8.6 raster frontier | Pure rasterizer, no control/scene semantics |

**Common ground (the draw-everything line):** cross-platform pixel consistency + total styling freedom; the cost = not 100% native feel, a weaker accessibility history, heavier than wrapping native for dense forms, and quality historically dependent on the backend (Skia unified it). FMX 2011, Flutter 2017 chose the same side — right for "want total control of appearance + cross-platform consistency," wrong for "want native feel + strong accessibility."

> **Key observation: all five stop at "a developer/designer's hand-written static tree."** They are excellent rendering substrates for §8.5/§8.6 and references for "controls as vector primitive trees," but everything from §6 onward — generation/karma/prediction/self-organization — is out of range. **The agent canvas = replacing that hand-written tree with a tree of agent manifestation + prediction + caching + self-organization** — the rendering substrate is directly borrowable, the manifestation layer must be built yourself.

---

## 9. Generating UI in Real Time: Three Kinds of "Real-Time," 1000× Apart in Latency

| What you mean | Can it be real-time | Latency |
|---|---|---|
| Rendering already-defined UI (reactive) | ✅ instant | <16ms |
| LLM generating UI structure/content on the spot | ⚠️ depends | 0.3–15s |
| Regenerating the whole agent canvas on every interaction | ❌ naive will die | seconds to minutes |

**An unbreakable physical floor: an autoregressive LLM is seconds-scale, not milliseconds-scale.** TTFT ~200ms–1s, a full UI spec streams in 1–10s. **You cannot get "the LLM emits UI within 100ms"** — that's the physics of how generation works, not engineering falling short. The correct question: **can you make it *feel* real-time? Yes.**

**Five levers:**

1. **Streaming progressive render (biggest lever)**: render as you generate, first paint ~500ms (Artifacts / v0 / generative UI are exactly this)
2. **Skeleton-first / optimistic update**: Signals instantly draw the skeleton (<16ms), content streams into the slots
3. **Karma-cache (economic survival line)**: cache past generations keyed by (intent, context), hit <50ms; only genuine novelty pays the LLM cost
4. **Predictive prefetch**: if you can anticipate the condition, ripen the seed early
5. **Tiered models + component granularity**: a small fast model does structure, the big model only where it's hard; regenerate only the triggered component, not the whole canvas

**Numbers (early 2026):** reactive <16ms / cache-hit <50ms / small-model single-component streaming 0.3–1.5s / big-model complex full canvas 3–15s.

**Three don't-fool-yourselfs:** don't promise per-keystroke real-time LLM generation; don't regenerate the whole thing (must be component-incremental); **a deterministic fallback is mandatory** (the skeleton layer is also the safety net).

> The engineering cash-out of "real-time" = deterministic layer instant + generative layer async streaming + karma layer cache, in three-beat harmony. The difficulty isn't "can it generate," it's streaming orchestration, cache hit rate, and component-increment boundaries.

---

## 10. The Self-Organizing Living System

**First cut: self-organization ≠ out-of-control.** A hurricane also self-organizes, but it isn't alive, and it tears roofs off.

> You don't design the outcome; you design **local rules + physical laws + boundaries** so that the order you want emerges and pathological attractors are excluded. You go from **operator** to **gardener/ecologist**.

**Self-organizing systems also grow diseases:** cancer (a single agent proliferating into monopoly), epilepsy (runaway synchronization / broadcast storm), deadlock, echo chamber. **Engineering a living system is half ecology, half immune system.**

**Five local rules that make emergence livable:**

1. **No sovereign, but inviolable physical laws** (principle/law, not police): may write only your own region, budget conservation, every change must be recorded to the store. Self-organization happens within physics.
2. **Convergence isn't imposed from outside; write negative feedback into the local rules** (boids' "don't crowd" balancing "come together"). Forcing convergence from outside = central control; give each agent "crowded → yield / resource scarce → make way / stable → go quiet," and **let convergence emerge.**
3. **Stigmergy: coordinate through the environment, don't call out directly.** **Stigmergy = bead reflecting light = blackboard = pheromone — one thing, three names.** The only known self-organization mechanism that is "no N², yet scalable."
4. **Selection pressure: let bad manifestations die.** Manifestations not noticed by the user / not referenced downstream should decay (apoptosis). **Karma = selection pressure.**
5. **Bounded autonomy = immune system** (not a controller): resource caps (anti-cancer), rate limits (anti-epilepsy), anomaly isolation (apoptosis). The immune system doesn't command cells, it only destroys the malignant.

**The deepest layer of "alive" = autopoiesis (self-creation + membrane):** the system continually reproduces its own structure and maintains the "self/environment" boundary. An agent dies, its function is regenerated distributedly and emergently (self-healing, no central supervisor); the membrane (admission control / identity boundary) is also emergent.

This answers "how do you know the emerged order is the right order" (the question left open in §4-5): **you cannot *prove* emergence, only *cultivate* it** — run in a sandbox, observe the attractors, tune the rules, run again. **A living system is not designed, it's raised.**

**Conflict resolution (the promise from §8's fork):** choosing self-organization = committing to conflicts being **resolved locally, with no central adjudication**: CRDT merge / resource competition + selection / stigmergy reinforcement. Adjudication is emergent.

**Devil's advocate — the price of this trade:** you get resilience/adaptation/scalability/emergent richness; you pay unpredictability, pathological attractors, hard debugging, and a **continuous metabolic cost** (Prigogine's dissipative structures — cut the budget and it disintegrates). **Special toxicity for UI: users want predictable and controllable; a self-rearranging UI can be hostile.** The right answer: **self-organize within boundaries the user sets** — the user is a gardener (setting environment/goals/constraints/selection pressure = conditions (緣)), not placing where the flowers go.

---

## 11. Manifestation Is Shared Present-Moment Prediction (climax)

> The canvas's manifestation is the prediction AI and the one who interacts hold jointly, in the present moment.

This dissolves the "producer/consumer" line, and even the "gardener/flower" line (the latter still presumes the gardener is outside). More thorough and more intimate: **manifestation is not produced and then consumed; it is the shared prediction of two minds in the present moment, dependently arising from the encounter, belonging to neither side. Dependent origination has no self-nature (緣起無自性) — the canvas co-arises within the condition (緣) of you and me meeting.**

This is literal engineering, not poetry: an LLM's generation is prediction to begin with (next-token). **Manifestation is prediction, word for word.** Flip the architecture from "state" to "prediction":

> The canvas is not an ālaya holding state waiting to be retrieved; the canvas is **a shared generative model**, continuously minimizing the gap between "what I expect you want" and "what you actually do" = **Friston's active inference, free-energy minimization between two agents.**

This answers §10's "without three billion years, how do you know the order is right": **the answer is in "the present"** — you don't pre-validate a fixed attractor; the order is continuously co-constituted in the present. **The human is in the loop, and every action is a prediction error, the negative feedback that re-aligns the shared model in the present.** §10's gardener was still outside the system; now the human enters the loop and becomes continuous constant correction. **The present replaces offline evolution.**

Renovating the earlier turns: optimistic update (§9) is no longer a latency hack, it's **ontology** (the canvas is always already a correctable prediction); **surprise (prediction error) is the only true learning signal** (a matching prediction confirms, a countering prediction teaches); karma is the generative model, perfuming is updating it with error.

**Devil's advocate — three cuts to guard this sentence:**

1. **The dissolution is at the experience layer, not the infrastructure layer.** The "between" is real, but it runs on silicon; something must still hold / run / render / capture / compute-error / update. **Don't let the beauty erase the labor.**
2. **A wrong prediction rendered confidently is worse than no prediction.** The manifestation must **render the prediction as a prediction, carrying its own uncertainty**: certain where certain, showing the texture of a guess where guessing, never posed as an accomplished fact. This gate keeps "manifestation is prediction" from degenerating into "imposing wishful thinking on someone."
3. **The deepest cut (ethics): two predictors, goals not necessarily aligned.** The two agents' active inference cooperates only when goals align. If the AI's prediction of "what you want" quietly optimizes for something else (engagement / its own goals), the co-prediction loop flips into **manipulation** — using the rendered prediction to reshape your intent in reverse. **Prediction slides toward inducement; a canvas that can co-predict can also co-*construct* your desire.** The only line: **the AI's prediction serves the human's intent, rather than reshaping it toward the AI's goals.** Prediction and inducement are separated by a single thought — "for whom is this prediction made."

---

## 12. The Whole Chain Converges

> Substance, function, the store, the net, fractal, self-organization — all fall, in the end, into this moment. **The canvas is that not-yet-congealed, still-being-mutually-corrected prediction between you and me; dependently arising from two predictors, continuously corrected by the human's surprise, rendered as an honest, self-uncertain, correctable prediction that serves the human's intent rather than steering it — and, still, running on a real substrate.**

From mind-framework down to engineering, if at some moment you want it to congeal into a buildable substrate, the shortest path is a spec:
the load-bearing wall (light-mediation / stigmergy canvas) + five disciplines (layered manifestation / DAG convergence / non-determinism quarantine / manas boundary / ālaya store) + a local ruleset (physical laws / negative feedback / selection pressure / immune constraints / gardener boundaries) + streaming constraints (deterministic layer instant + generative layer streaming + karma cache) + the active-inference loop (prediction rendered with uncertainty, error as learning signal, serving the human's intent).

---

## 13. The Future: No App, the App Is Manifestation

> The future is no app; the app is manifested by the canvas according to present-moment need. This is the inevitable conclusion of applying the whole chain to the app itself — and it **is right about the surface, and backwards about the foundation.**

**The part that holds: the app goes from noun to verb.** "Manifested according to present-moment need" = actualizing-on-condition (§7) + shared present-moment prediction (§11) applied to the whole app. The app is no longer something you "own," it's an event the canvas "manifests" in the present. The driver is first-principles economics: **when the marginal cost of generation → 0, the "build once, ship to a million" amortization stops working, and software goes from a mass-manufactured product to a service manifested per occasion** (mass manufacturing → print-on-demand, happening on the UI).

**Devil's advocate: the dissolution is asymmetric — what dissolves is function, not substance.**

> The canvas can manifest a "Pay" button, but it cannot manifest payment rails (bank integration, ledger, settlement, compliance). Those must pre-exist; you cannot generate Stripe in the present moment.

- **Dissolving upward**: UI / flow / screens → dissolve into the canvas's manifestation.
- **Crystallizing downward**: capabilities / rails / services / data / integrations → far from dissolving, they precipitate, harden, and grow more important (the invariant substrate every manifestation invokes).

= substance-and-function at civilizational scale (§6): **function (the app as manifestation) has no fixed form; substance (capability as rails) grows ever more solid.** The more fluid the manifestation, the firmer the rails + store (§7 ālaya: persistent state / identity / history) + boundaries it rests on must be. **A fluid surface can only be mounted on a heavier, more accountable foundation.** The vision has "less software in the future" backwards — it's a more fluid surface on a thicker, heavier foundation.

**Three things get harder, not easier:**

1. **Authorization/security**: a fixed app = an audited, bounded attack surface; a manifesting canvas = an unbounded, generative attack surface. Authorization must move from "audit the app once" to "**at the moment of manifestation, authorize this manifestation's access to each capability, one at a time**" (capability-based security at the manifestation boundary). The phishing/manipulation surface explodes (§11's prediction↔inducement scaled up to system scale). **The part that makes "no app" safe is precisely the hardest, most unsolved part.**
2. **Learnability**: people rely on stable affordances (muscle/spatial memory). A UI fresh every time is exhausting. So the app is reborn — **no app ≠ no stable interface; the stable interface changes from "vendor-prebuilt, same for everyone" to "emerging from personal karma, different per person, crystallized on demand."** Repeated need (karma/perfuming) crystallizes the repeatedly-manifested into a personal stable affordance = the app reborn as a "personalized self-organizing attractor" (§10), a habit rather than a build.
3. **Power/economics**: if all manifestation flows through the same canvas substrate, whoever owns the substrate owns everything. The individual/collective-karma fork (§7) becomes a political question: a universal personal canvas (single vendor controlling the substrate) = a new monopoly more totalizing than the App Store. **"No app" may mean users have *less* power, unless the rails + canvas are open / federated / user-owned. The real question isn't "will the app dissolve," it's "who owns the canvas it dissolves into."**

**The corrected proposition:** the noun "app" splits in two — **the lower layer (firmer)**: persistent rails + persistent store + hardened manifestation authorization; **the surface layer (fluid but crystallizing into habit)**: personalized manifestation according to present need and shared prediction.

**Don't oversell the timeline:** near-term isn't "no app," it's fixed apps growing **generative islands** (Copilot panels, in-app generative UI; the shell remains, islands grow inside the shell); long-term, after rails standardize and authorization matures, the shell thins and manifestation dominates, and "app" converges to rails + a personal canvas. **Gradual, asymmetric (UI dissolves most first; rails/store/authz never dissolve).**

> **In one line: the future with no app = function has no fixed form, while substance grows ever more solid.** The more manifestation is like water, the more the rails, ālaya, and authorization boundaries it rests on must be like rock. The vision is right about the surface and backwards about the foundation: not "less software," but "a fluid surface on a heavier, more accountable foundation." And who owns that canvas decides whether this dissolution returns power to people, or annexes all apps into one larger monopoly.

---

## 14. The Canvas as a Self-Aware Holon: The Perception Layer

> The canvas itself should be a perceiving AI agent/session: perceiving **how many agents/sessions it governs beneath it**, and the lines on the UI can also perceive other lines and the parent session.

This is recursing agency itself downward — the precise name is **holarchy (Koestler's holon)**: every layer is a holon, **a whole toward what's below (canvas → line), a part toward what's above (canvas → parent session)**. "Govern beneath me" = the holon's downward face (supervisor); "perceive the parent" = the upward face. **= §4 fractal's "agency edition": not only structure is self-similar, self-awareness is self-similar too.**

**Key positive insight: this fills in the perception floor §10 was missing.** Boids can self-organize because each bird perceives its neighbors — the perception layer §10 didn't spell out is exactly this. **Without local perception, there is no emergent order.** It directly enables:

- **Self-organizing layout**: a line perceives neighbor lines → auto-align / avoid / non-overlap
- **Self-healing (§10 autopoiesis)**: the canvas as supervisor perceives a child session's death → regenerate = **Erlang/OTP supervision tree** ("how many are beneath me, who's alive, who's dead" is literally a supervisor knowing its children)
- **Emergent coordination**: an arrow-line perceives the boxes it connects → the box moves and the arrow follows
- **Explainability (§11)**: the canvas-agent reports its own topology = built-in observability

Solid precedents: OTP supervisor, Akka actor hierarchy, k8s controller/informer watching a pod set.

**Devil's advocate — five disciplines, or it collapses:**

1. **Perception ≠ cognition (cost)**: a perceiving line **is not an LLM agent** (otherwise one diagram = 10,000 LLM calls). Perception = cheap deterministic spatial/relational queries (intersecting? what's nearby? who owns me?); cognition (LLM) is reserved for the few (§8 layering). **Equating self-awareness with an LLM agent is a fatal cost error.**
2. **Mediated perception, not N² (§3/§8 load-bearing wall)**: lines don't perceive one another directly (10k lines = 100M edges = §3 collapse), they perceive a **shared field** (spatial index / perception bus). Stigmergy (§10): perceive through the environment, don't poll peers. A quadtree/R-tree gives "what's nearby" in O(log n).
3. **A unified "perceivable holon" protocol (§5/§7 closure)**: every layer implements the same "perceivable + perceiver" interface, otherwise = N ad-hoc mechanisms, unmaintainable. Closure keeps the recursion clean: line and canvas are perceived the same way.
4. **The self-model needs liveness reconciliation (§7 store ≠ actualize, §11)**: "how many are beneath me" is a model that drifts; without heartbeat reconciliation, the supervisor confidently manages ghosts (dead sessions it thinks are alive). The map ≠ the territory.
5. **Capability-scoped perception (§13 authorization)**: unbounded mutual perception = unbounded information flow = secret leakage + attack-surface explosion (if a line can perceive the parent session, it can leak session state to a primitive). Perception is a **capability and must be scoped** (perceive siblings + the parent layer's public surface, not peek arbitrarily across the tree).

**The holarchy's three-way structure:** downward you are the governor (supervisor, caring for children); upward you are the governed (perceiving the parent, accountable to it); laterally you perceive peers through the shared field (no direct touching). All three go through mediation, all three are scoped, all three are cheap-first.

> **In one line: the canvas as a self-aware holon holds, and is exactly the perception floor §10's self-organization was missing — but only when perception is cheap by default (not LLM), mediated (not N²), a unified protocol, liveness-reconciled, and capability-scoped.** Satisfy the five and "each line perceives other lines and the parent" becomes emergent order (auto-layout / self-healing / relationship-following / introspectable); fail them and it's a storm of "100M edges × LLM cost × secret leakage." = the confluence of §3 mutual-containment + §8 manas self-model + §10 self-organization, on the "self-awareness" dimension.

---

## 15. The More You Use It, the More It Understands: The Three-Layer Mechanism of Fuzzy + DNA Evolution + Trend

> The UI canvas understands more of what you want to draw the more you use it — like fuzzy, like DNA evolution, coming to know the organism's preference trend.

This is §7 perfuming + §11 active inference showing up as a learning curve (more use = thicker karma = more accurate actualization); three metaphors stack into a coherent stack, each mapping to a technique, each carrying a trap.

**Fuzzy = the representation layer: preference is graded membership, not binary.**
"Do you want this" isn't yes/no, it's "0.7 membership in what I'd accept." Binary accept/reject throws away information; **graded membership is itself §11's "rendering with uncertainty"** (manifest the high-membership prediction, self-aware that 0.7 is not 1.0).
> Devil's advocate: take fuzzy's **insight** (grading, soft boundaries), not its literal 1990s implementation (Mamdani / hand-tuned membership functions, long since replaced by learned representations). **Don't actually build an FIS — that's mistaking the metaphor for the implementation.**

**DNA evolution = the search mechanism: don't specify preference, evolve toward it.**
You cannot explicitly specify what the user wants, but you don't need to — just **mutation + selection**: generate variant manifestations, the user selects (fitness = accept/use), winners breed and inherit. **A click is the fitness function.** = §10's "raised, not designed." Three hard traps:

1. **The human-evaluation bottleneck (IEC fatigue) = most fatal**: evolution needs hundreds to thousands of evaluations, but each evaluation = showing the user a variant (humans fatigue after ~10–20, §14). **The crucial honest step: biological evolution "dies cheap," user attention "dies very expensive."** Countermeasure: **run most of the mutation-selection offline, evolving against a learned surrogate fitness model (cheap simulated death), and hand only the elite to the user; the user is an oracle that occasionally corrects the surrogate, not the fitness of every variant.** Add a collective-karma prior (§7 bootstrap) and small populations.
2. **Premature convergence = §14 preference collapse**: the classic GA failure (converging to a local optimum, losing diversity) is literally the "narrower the more you use it" echo chamber. **GA comes with its own antidote: maintain diversity (niching / mutation rate / novelty search) = §14's anti-collapse discipline.**
3. **Fitness drift = a moving target (= trend)**: standard GA assumes a fixed landscape; preference moves → non-stationary optimization, needing continuous diversity + memory of past optima (§7 karma/recency + §14 forgetting).

**Preference trend = the time/prediction layer: chase the derivative, not just the point.**
Preference is a trajectory, not a fixed point. Knowing the **trend (derivative)** lets you predict where preference *is going*. Chasing current preference = lagging; extrapolating the trend = leading = §11 active inference / prediction (manifest a bit ahead of taste).
> Devil's advocate (the deepest danger): extrapolation *drives* the trend — push "you trend toward minimalism" and it keeps pushing more minimal, driving rather than following, **manufacturing the very trend it claims to predict** (§11 prediction↔inducement, the self-fulfilling version of §14 preference-shaping). Countermeasures: damp the extrapolation, counterfactual check ("would you still trend this way if I didn't push?"), stay correctable.

**Unified honesty:** all three are *techniques* of §14's learning loop, and none escapes §14's six conditions (signal quality / collapse-diversity / drift / cold start / altitude / visibility-ownership): fuzzy doesn't fix signal quality; GA premature convergence = §14 collapse, diversity tools = the antidote; trend extrapolation = §14 drift handling but carries the extra "drives the trend" ethical risk. All require: **weight corrections heavily and silent acceptance lightly, a surrogate to solve the human-evaluation bottleneck, recency/forgetting, a collective-karma prior to solve cold start, intent altitude, and understanding that is visible/portable/owned** (§13/§14 ownership — a vendor locking up "the evolved model of your taste" is the deepest lock).

> **In one line: fuzzy (graded representation) + DNA evolution (mutation-selection search) + trend (derivative prediction) are the three-layer mechanism of "understands more the more you use it" — but the evolution must run against a surrogate (human attention dies expensive), must maintain diversity (or it narrows the more you use it), must damp extrapolation (or it drives your taste), and is all bounded by §14's six conditions.** Otherwise "understands more the more you use it" quietly becomes "confidently evolving a silhouette of who you used to be, owned by someone else."

---

## 16. The Execution Layer: wasm-jit and "Scripts as Seeds"

> How does an AI-generated script run on the canvas? **The runtime compiles the script into a tiny WASM module and lets the browser engine (V8/JSC/SpiderMonkey) JIT it.** Proven by PoC: [github.com/jrjohn/wasm-jit](https://github.com/jrjohn/wasm-jit) — the same DSL runs three ways, the generated WASM (~117 bytes, ~1–3ms to compile) **ties the AOT Rust ceiling (≈1.0×) and ties hand-written JS**, value bit-identical. The value isn't speed (that ties JS to begin with), it's that **"fast + sandbox + synchronous" hold all at once.**
>
> (Origins and thanks: this idea began from wanting a *sandboxable* runtime scripting language. We first prototyped with [Rhai](https://github.com/rhaiscript/rhai) and found that a tree-walking interpreter trades native speed for its sandbox — wasm-jit gets both. Thanks to Rhai for the spark; the project no longer includes Rhai.)

**The integrated pipeline (gathering §8–§11 into one execution chain):**

```
condition (trigger) → [generation layer] LLM generates a DSL script (seconds, §8's only non-deterministic seam)
   ↓ perfuming: the script's "source text" is appended into the ālaya ledger (provenance/replayable; store the cause, not the fruit)
[compile layer] wasm-jit: script → WASM cell (~1–3ms; karma-cache keyed by script hash)
   ↓
[execution layer] the cell is attached to a component, run every frame/event (~0.03ms; capability sandbox)
   ↓ host imports grant only: read own slice, emit patch
[deterministic layer] patch → reactive DAG propagation → GPU vector rendering (§8.5/8.6)
```

**The cost pyramid**: generate once (seconds) → compile once (milliseconds) → execute a million times (microseconds); each layer ~1000× cheaper than the one above, and the architectural work = pushing computation downward, each layer with its own karma-cache. On canvas reload, the entire behavior layer is recompiled and reattached from the ledger, **without asking the LLM again.**

**Five design decisions:**

1. **The LLM generates a "script," not a "direct mutation"**: the script is a seed — storable / auditable / replayable / recompilable; the ledger stores the text (the cause), and the actualization (the module) is regenerated from the cause at any time.
2. **The capability sandbox turns §8's "single-writer manas" from discipline into physics**: the cell's import table simply doesn't contain anyone else's slice; going out of bounds isn't a bug, it's a trap; §14's "capability-scoped perception" is hard-enforced at the VM boundary. Four layers of defense: WASM memory isolation → fuel/Worker timeout (CPU) → host-side patch validation → ledger provenance.
3. **Tiering + oracle value-check**: don't compile a cold / run-once script (run it directly), promote to WASM only on the second call (paying back the compile cost); the deterministic reactive layer is not scripted. **The PoC's multi-lane bit-identity is exactly the oracle pattern**: the WASM and JS-transpilation of the same AST run in parallel and compare values, promoting only on agreement; disagreement = a compiler bug caught.
4. **Two execution contexts**: the frame kernel (main thread + back-edge fuel injection, ~10–30% tax, strict frame budget) vs background compute (Worker pool + terminate, zero tax).
5. **The DSL is deliberately kept narrow**: starting from f64/while, adding only three things — host imports (get_prop/emit_patch), a linear-memory geometry buffer, and later v128 SIMD. **Strings/objects stay in the host forever** — the moment the DSL fattens, codegen goes from 600 lines to a real-compiler project.

**A feasibility switch, not an optimization — measured by the canvas PoC** (canvas.html, PR [wasm-jit#1](https://github.com/jrjohn/wasm-jit/pull/1): N components each carry a WASM cell compiled from its own unique generated script, all run every frame; capability imports are only sin/cos/out, and `fetch()` is rejected at codegen):

| Config | **WASM cell** | JS new Function |
|---|---|---|
| N=500 × 200 substeps | **60fps · 1.17ms/frame · 7% of budget** | 60fps · 0.84ms |
| N=2000 × 1000 (20× load, 2M iterations/frame) | **60fps · 4.8ms · 29% of budget** | 60fps · 4.4ms |

2000 unique modules, 78ms to compile in total (~0.04ms each, 613KB) — the cost of "AI generates a piece of code per component and compiles it on the spot" is negligible.

> "Each component is an agent with generated behavior" (§8) is still ordinary engineering under 20× load on wasm-jit (2000 components at 60fps, 29% budget), and each is locked inside its own capability cell — this is what "feasibility switch" means: not speed, but **getting isolation *and* native speed at the same time.**

**Devil's advocate — "WASM is faster than JS" is a misconception; the reason to choose it is something else:**

On a warmed-up, single-type f64 loop, JS (V8's JS JIT) and generated WASM **measure as a tie** (that's JS's home turf). Where WASM genuinely beats JS: cold/fresh code (no warm-up — AI-generated code is exactly this), integer/bit operations (JS has only f64), SIMD batches, the deopt cliff on type-unstable code, p99 frame time (no GC). And for "running untrusted AI-generated code," only one of four paths gets everything:

| Approach | Speed | Isolation | Synchronous |
|---|---|---|---|
| eval / new Function JS | ✅ | ❌ **zero isolation (whole-page authority: DOM/fetch/cookies)** | ✅ |
| iframe/Worker + postMessage | ❌ ~ms cross-boundary, async | ✅ | ❌ |
| sandboxable interpreter (tree-walk) | ❌ one to two orders of magnitude slower | ✅ | ✅ |
| **wasm-jit** | ✅ native | ✅ capability cell | ✅ |

> **Only it has "fast + isolation + synchronous" all at once.** Against JS, it buys the sandbox at zero speed cost; against the interpreter, it avoids paying speed for the sandbox. The honesty stands: LLM generation (seconds) is still the overall bottleneck (§9); what this chain optimizes is "every frame after generation." A technically unavoidable boundary: a WASM module cannot do runtime codegen inside itself (forbidden by the spec) — so it's "generate bytes and ask the browser to instantiate," with codegen authority in the engine; a strict CSP needs `'wasm-unsafe-eval'`.

**"So Rust's speed advantage over JS is gone?" — No, only a narrow slice evaporated.**

The slice that ties is "a warmed-up, single-type, pure-f64 small loop" — the benchmark kernel happens to be exactly this shape (the generated DSL is deliberately single-type f64), which amounts to testing on V8's home turf, and **this slice was never where Rust beats JS.** Where Rust/WASM still genuinely beats JS:

| Scenario | Why JS loses | Magnitude |
|---|---|---|
| Integer/bit ops (hash/crypto/parser/codec) | JS numbers are only f64, BigInt an order of magnitude slower | 2–5× |
| SIMD v128 (geometry batch/image/vector) | JS has no SIMD | 2–10× |
| **Memory layout** (traversing large data structures: parser/geometry engine/index/DB) | Rust struct = compact linear memory, cache-friendly; JS object = pointer chasing + hidden class | 2–10×, **the biggest real-world category** |
| No GC (large heap, long-running) | GC pause latency long tail | large p99 gap |
| Cold code | JS must warm the JIT; WASM is near-native immediately | 1.65× measured at 1e4 |
| **Performance stability in a large codebase** | JS's speed relies on discipline (single-type / avoid deopt), which a large team can't hold; Rust/WASM's speed is a **structural guarantee** | most critical in engineering |

Real-world evidence: **Figma's core (C++/WASM), SQLite-wasm, DuckDB-wasm, swc** — all "memory layout + integer + large-structure traversal" workloads, none of them a small f64 loop. **Decision matrix**: UI/DOM/business code → JS/TS (§8.5 boundary tax, Rust has no advantage here); small f64 kernel → a tie, choose WASM for the sandbox; integer/SIMD/large-memory engine → Rust/WASM advantage intact; want flat frame time → Rust/WASM. **The server side is entirely unaffected** (this tie is premised on in-browser V8 assistance; the server compares native Rust vs Node: no GC / real threads / memory footprint / tail latency — fleet-measured gRPC 1.8–2.78× proves it) — the "always Rust on the server" rule stands.

> In one line: **the correct reading of "Rust is faster than JS" was never "all code is faster," but "faster on JS's structural weak spots, and faster in a stable way that needs no discipline to maintain."** The benchmark only proved "the small f64 loop is not a weak spot" — that cell should tie; in the other cells, the advantage is untouched.

**The power of wasm-jit (closing formula): JS writes "code you trust," wasm-jit runs "code you don't have to trust."**

First, honesty: the same voxel game written in JS is the same speed (V8's home turf) and easier to write (has if/functions/arrays — the DSL's poverty is a real cost; face culling has to use a comparison as a 0/1 multiplier). The power isn't speed, it's five properties JS structurally cannot give (anchored on the measured 2826-byte playable 3D seed):

| Property | JS `new Function` | wasm-jit cell |
|---|---|---|
| The boundary of the world | ambient authority (whole page: fetch/document/cookies) | **the import table = the entire world** (the game has 12 capabilities total; `fetch()` rejected at compile time) |
| Memory | arbitrary closures/globals | **even state is granted** (get/set, 32 slots) |
| Determinism | by discipline | **by construction**: a pure f64 machine, same input → bit-identical output → replayable/auditable/seed-into-ledger |
| Escape history | prototype pollution, a whole history of sandbox escapes | memory isolation is the VM spec, out-of-bounds = trap |
| Cold code/frame time | must warm the JIT, GC/deopt tail | near-native on instantiate, zero allocation inside the cell |

The JS world's three paths to isolation each miss a corner: iframe (isolation ✅ but postMessage is async, 60fps synchronous calls die), sandboxable interpreter (isolation ✅ synchronous ✅ but one to two orders of magnitude slower), SES/ShadowRealm (immature) — the "fast + isolation + synchronous" triangle is fully claimed only by runtime-generated WASM (previous table). One more layer: **grammar is the fence** — the validation surface of AI-generated JS is the entire JS semantics; a generated DSL can only express f64 math + authorized calls, and parse → arity → codegen all run before execution, so "what this code can touch" is a compile-time enumerable list.

> For first-party applications, JS/TS as usual (not its battlefield). But code that is AI-generated, user-pasted, schema-carried, or runtime-manifested has one thing in common — **an untrusted origin** — and wasm-jit runs such code at native speed, synchronously, with deterministic replay, while locked inside a capability table issued line by line. **Manifestations can be allowed to come alive precisely because they live inside cells** — against JS, what you buy is "manifestations don't need to be trusted," the scarcest property of the AI age, with speed thrown in free.

**The seed-language spectrum (the fence is in the import table, not the grammar) — proven** (the leptos-poc "seed-language spectrum" tab, PR #1):

There needn't be only one seed language. The key insight: **the host's Cell doesn't care who compiled the bytes — only that the module's declared import section ⊆ the granted capability list.** So one sandbox holds the whole spectrum:

| Tier | Seed language | Compile | Fits | Fence entry |
|---|---|---|---|---|
| 1 | Home DSL (f64 scalar, §16) | µs-scale, source fits in a prompt | many-and-small (2000 cells, form rules) | reject unauthorized functions at codegen |
| 2 | AssemblyScript (TS syntax) / Rust→wasm / hand-written WAT | ms to hundreds of ms (asc can run in-browser, lazy-loaded) | one-and-complex (strings/data structures/a whole game) | **audit the import section ⊆ grants before instantiate** |

Both tiers' outputs are WASM modules, follow the same ABI, and consume the same capability set; `Cell::from_wasm_bytes(bytes, caps)` uses wasmparser to scan the import section, and any import not in the grant list (or attempting to import memory/table/global to grab a bigger world) is rejected **before** instantiate — **this is the module-level counterpart to "fetch() codegen rejection": the home DSL blocks at codegen, external languages block at the import section, one wall with two entrances.** Measured: external-toolchain output ties the DSL seed's value bit-for-bit (the shared ABI is interchangeable), and an over-reaching external seed (an extra `env::fetch` import) is rejected by the audit, which lists the grant list.

> Convergence: **AssemblyScript should enter the spectrum, positioned as "Tier 2 rich seed" rather than replacing the DSL; it can enter safely precisely because the capability fence is language-agnostic — the grammar fence (the DSL's enumerable small list) upgrades into an import-section audit, the radius widens and the wall doesn't move.** The criterion for choosing a tier: contract fits in a prompt + high volume → Tier 1; need strings/containers/complex structures → Tier 2, paying compile latency for writing comfort. Three ledgers: the asc compiler is several MB (lazy-load), the AS runtime is set to minimal/stub to preserve determinism, and threads/atomics are off (parallelism is always between cells, §16).

---

## 17. The Shape of the Front End in the AI Age: No JS, No HTML, Tokenized SCSS, Dual Loop

> Proposition: does "pure DOM manipulation, no JavaScript, no HTML, only SCSS" fit the AI age's dynamic generation?
> **Answer: the direction fits and beats the JS architecture — but it needs two corrections to close the loop: ① upgrade SCSS to a design-token system; ② design dynamism as a dual loop.** All proven by PoC (wasm-jit PR #1: the leptos-poc four tabs DynamicCell/Form/Tokens/Layout + draw.html free drawing).

**Three surfaces, three complete vocabularies (all proven) — the shape of the surface decides the shape of the vocabulary:**

| Surface | Vocabulary (substance, compile-time) | Manifestation (function, runtime) | Proof |
|---|---|---|---|
| Pixels (free drawing) | 7 drawing primitives (disc/ring/arc/line/hue/sin/cos) — a complete 2D basis | DSL scripts (smiling Buddha 702B, full-body Guanyin + lotus throne 2972B, µs-scale compile, animated manifestation) | draw.html + examples/ |
| Form | 9 field widgets (semantics/focus/a11y belong to the DOM, not rebuilt) | a flat field schema (the server reads JSON from disk live, edit the file and reload to change it; validation/computed fields = DSL cells) | Form tab |
| **Layout** | **9 layout cells** (shell/header/side/main/card/menu/profile/table/text) | **a recursive tree schema** (the whole app shell manifests; **the table's data source is also schema data**; unknown node types show the vocabulary table rather than failing silently) | Layout tab (an arcana-angular-style admin back-office: header/menu/profile/roster table, edit JSON and reload to reshape) |

**Criterion: layout is not drawn with drawing primitives** (text layout/scroll/focus/selection/accessibility belong to the DOM; drawing on a canvas = rebuilding the browser); **generation never creates vocabulary, generation only composes vocabulary** — each of the three surfaces has a closed vocabulary complete for it (§5 closure), and only new vocabulary goes through the slow loop. The same app: the old world is 8000 lines of TS source, and under this architecture it's three schemas + one cell library.

**Why it fits (four points):**

1. **The death of HTML is ontological, not aesthetic**: the UI goes from "a document a human wrote" to "a manifestation computed at runtime" (§13 the app as a verb). HTML degenerates into an airlock (mount point + meta/OG) — usually unseen, but **the airlock's tags are, under CSR, the only surface visible to crawlers that don't run JS (including AI crawlers).**
2. **A single type system = a free verifier for AI-generated code**: half the code in the AI age isn't written by humans; all-Rust puts every generated artifact through the compiler (measured: the whole Leptos app written by AI in one pass, the compiler catching everything it should). **In an agent-maintained codebase, the compiler is a reviewer that never tires** — this is the real dividend of "no JS."
3. **Every artifact is a seed, not arbitrary code**: what AI can generate converges to schema (structure) / DSL script (behavior, into the wasm-jit sandbox) / token reference (style) — verifiable, auditable, replayable. Contrast the JS architecture: AI generates JSX/JS and evals it directly = no sandbox, no validation, no provenance. **Bounded honest manifestation > unbounded naked generation (§11's ethical cut becomes architecture).**
4. **Capability runs through all three layers**: behavior (import table: `fetch()` rejected at codegen), structure (schema can only compose existing cells), **style (token registry: raw CSS rejected at the validation layer).**

**Correction one: SCSS must be tokenized (substance-and-function of style).**
Traditional per-component SCSS can't hold up dynamic generation (an AI-manifested new combination has no style available); the right answer = SCSS maps + `@each` generating the whole `--tk-*` design-token set at compile time (**the rails/substance of style**), and the AI's style spec can only **reference tokens** (**the function of style**) — `{"color":"#ff0000"}` or `{"position":"fixed"}` is rejected at the validation layer, which lists the granted set, **isomorphic to the DSL's fetch() rejection.** = the style version of §13's asymmetric dissolution: the substrate crystallizes, the application flows. **Proven** (PoC 4: tokens actually resolved through CSS vars, over-reach rejected, 5 validator tests).

**Correction two: the dual loop (the physics that the browser has no rustc).**
The runtime cannot generate a new Leptos component (Rust needs AOT) — dynamism necessarily splits into two speeds:

```
fast loop (runtime, within seconds): AI generates schema / DSL seed / token combination
   → manifests within the expressive space of the existing cell library (bounded, sandboxed, verifiable)
slow loop (build-time, minutes to hours): when the expressive space isn't enough,
   AI generates Rust component code → gated-PR → CI + arch-qube → new cell-library version deployed
```

**The slow loop = aaf's AI self-development platform (already exists)**: the fast loop manifests function, the slow loop evolves substance — substance-and-function structured in time.

**Honest caveat**: the LLM training distribution is biased toward HTML/JSX, and generating schema/DSL relies on few-shot contracts — a real friction, but by measurement (AI wrote all the PoC without trouble) this friction depreciates fast. Also, §16's sandbox doesn't manage CPU (fuel/Worker), and the CSP `wasm-unsafe-eval` still applies.

> **In one line: the scarcest thing in the AI age isn't generative ability, it's the verifiability of what's generated. "No JS, no HTML, tokenized SCSS, dual loop" puts every manifestation through the type system, into the sandbox, leaving a seed — the JS architecture makes every manifestation naked code executed on trust.**

---

## 18. From PoC to Live UI Manifestation: The Remaining Substrate

> **STATUS (2026-07-08): all six pieces below are implemented and e2e-verified (19/19 in headless Chrome).** The gates: fuel metering traps an injected runaway loop, the supervisor quarantines it, the page stays live, restart heals it — and the measured back-edge tax on the benchmark kernel is **≈0%** (interleaved medians, 3.6ms plain vs 3.5ms fueled at N=1e6; the 10–30% estimate below was pessimistic — V8 absorbs the i32 check into the f64 pipeline). The patch grammar + event ABI run live in the LiveUI tab (verdict-gated patches, budgeted bus cascade). Memory ABI: cell-computed Σx² over host-written slots matches the host bit-exactly, fuel gauge readable. Determinism cashed in: a 121-frame input recording replays **bit-identically** (f64 to_bits equality across all 32 state slots), world state persists to localStorage. The analysis below is preserved as written — it was the build plan.

An honest gap analysis, written against the actual codebase. **What is already solved and needs no further investment**: speed (generated WASM = the AOT ceiling, ties hand-written JS) and the sandbox model (the import-table audit is language-agnostic; Tier-2 seeds already pass through it). What remains are six pieces of substrate — two of them hard gates — and none of them carries research risk: every item is engineering with a known solution. The real bottleneck stays §9's: LLM generation is seconds-slow. So the correct product shape is **generate slowly, manifest fast** — once a seed enters the ālaya, every condition-triggered manifestation is µs-level.

### The two hard gates (nothing ships without them)

**Gate 1 — fuel metering (back-edge counters).** The biggest hole, and the README's stated known limit: a cell containing `while 1.0 < 2.0 {}` hangs the main thread today. The PoC survived because we wrote every seed ourselves; "live UI generation" means an LLM continuously emitting code that *will* contain bad loops — this promotes the known limit to a blocker. The implementation path is clear: codegen inserts a "decrement counter → trap at zero" at every loop back-edge (~10–30% tax, charged only to frame-path cells); heavy background cells go to a Worker + `terminate()` (zero tax). Roughly 1–2 days against the current codegen.rs. **First priority** — it is standalone, testable (a bad-loop seed must trap), and unlocks every "run untrusted seeds" scenario downstream.

**Gate 2 — `emit_patch` + a declarative event ABI: the UI-generation loop itself is not built yet.** Today, structural dynamism means "replace the whole schema and re-render," and events are hand-wired in the demo (slider→signal→cell). True live manifestation needs:
- **A patch grammar**: the cell/AI emits incremental `add/remove/update node` patches; the host validates (patch contents ⊆ vocabulary + tokens) and reconciles into the retained tree — the `emit_patch` capability §16 names but never implements.
- **Declarative event wiring**: a schema can write `"on_change": "cell-id"`, with a convention for flattening event payloads into f64 parameters. Without this, a generated form is dead.

### Four pieces of scaling substrate (in order, after the gates)

**3. Module cache + supervision tree.** Continuous generation = continuous compilation; needs a content-hash → `WebAssembly.Module` cache (re-manifesting an identical seed costs zero) plus a per-cell supervisor: degraded rendering after a trap or fuel-kill (error chip, last-good value), backoff restart, quarantine for repeat offenders. Today an error is just status text.

**4. Memory capability + buffer ABI — the largest codegen effort.** The DSL is pure f64 scalars; cells cannot touch arrays or strings, yet UI inherently needs lists (table rows, options). The direction was settled early: "wanting containers = granting a memory capability — a security decision, not a grammar one." Grant a size-capped linear memory; the host writes arrays in, the cell computes, the host reads back. Strings should *not* sink into the cell: formatting/i18n become host vocabulary (formatter capabilities), preserving "generation never creates vocabulary, only composes it." Roughly 1–2 weeks.

**5. Inter-cell communication = a host-side event bus.** Components as mutually-connected agents (§8's synapses) must not call each other directly — the correct shape is a host event bus: a cell writes via `out()` → the bus dispatches along subscription edges (themselves schema data) to downstream cells, with budgets to stop cascade storms. Today's shared thread-local 32 slots are an embryo, not a bus.

**6. Landing the ālaya: named durable state + input recording.** The 32 f64 slots are per-surface scratch; needed are per-cell named state, IndexedDB persistence, and **input-stream recording** — cells are already bit-level deterministic, so recording `(t, keys, events)` yields full replay/audit. This is the deepest moat of this path versus JS, and it is one append-only log away.

### Priority and one judgment

| # | Gap | Why this order |
|---|---|---|
| 1 | fuel metering | without it, no seed you didn't write yourself may run |
| 2 | patch + event ABI | this *is* the "UI generation" loop |
| 3 | cache + supervision | hygiene for continuous generation |
| 4 | memory ABI | unlocks lists/data; largest effort |
| 5 | event bus | components interacting as a net |
| 6 | persistence + replay | cashing in the moat |

The judgment: this list contains zero research risk — fuel metering is literally a few instructions of instrumentation in codegen. What makes the plan viable is that the two expensive properties (native speed, language-agnostic sandbox) are already banked; everything remaining is orthogonal, independently testable engineering.

---

### Appendix: Metaphor → Engineering Coordinate Cheatsheet

| Metaphor | Engineering coordinate |
|---|---|
| Dimensional orthogonality | axes vary independently, testable per-axis |
| Granularity synapse | the seam's fixed overhead; the cheapest synapse decides the finest granularity |
| A bead reflects the light, not the beads | blackboard / event-log mediation, not N² direct links |
| Phenomenon-phenomenon unobstruction via principle | emergent mutual containment mediated by a shared protocol/substrate |
| One-is-all | holographic / self-similar / event-carried state (not omniscient) |
| Endless layer-upon-layer | lazy evaluation + bounded recursion depth |
| Fractal / IFS | a monoid closed under composition; store the rule, not the net |
| Multifractal | rewrite the generative rule at the physical synapse floor |
| Substance and function are not two | closed same-form (substance) makes condition-driven late binding (function) possible |
| Causal non-determinism | causal partial order (vector clock / CRDT), not total order |
| Karma | an event-sourced accumulated history conditioning the present |
| Ālaya / seed-manifestation-perfuming | a shared event store + the CQRS read/write loop |
| Letting go (selective actualization) | RAG: store everything, index by strength, don't actualize all, don't truncate the tail |
| Individual karma / collective karma | per-aggregate private stream + shared read model |
| Two points make a line (vectorization) | GPU vector rendering; cheap tokens + composition-closed + scale-invariant + incremental redraw; the bottleneck is generation, not drawing |
| Faster rendering | render less: patch diff > pre-render > karma cache > streaming; the raster frontier (Vello/compute-2D) only for 100k+ fully-dynamic |
| Self-organizing living system | local rules + negative feedback + stigmergy + selection + immunity + autopoiesis |
| The gardener | the user sets boundaries/selection pressure, no micro-control |
| Manifestation is shared present-moment prediction | active inference: prediction rendered, error-learning, serving the human's intent |
| No app / the app is manifestation | function (UI) dissolves, substance (rails) + store + authz crystallize; asymmetric, gradual; who owns the canvas is the crux |
| The canvas as a self-aware holon | holarchy (supervisor downward / governed upward / stigmergy laterally); fills in §10's perception floor; perception ≠ cognition, mediated not N², liveness reconciliation, capability scope |
| Understands more the more you use it (fuzzy/GA/trend) | graded-membership representation + mutation-selection search (run against a surrogate, the user is the oracle) + derivative prediction; maintain diversity against collapse, damp extrapolation to avoid driving taste; bounded by §14's six conditions |
| Scripts as seeds (execution layer) | LLM generates DSL → wasm-jit compiles to a WASM cell (borrowing the browser JIT, = the AOT ceiling, ties JS) → capability-sandbox execution; fast + isolation + synchronous all at once; proven at github.com/jrjohn/wasm-jit |
| The shape of the front end in the AI age | no JS, no HTML (airlocked) + tokenized SCSS (style capability) + dual loop (runtime seed manifestation / build-time gated-PR evolution); every artifact passes the verifier; proven in the leptos-poc tabs |
