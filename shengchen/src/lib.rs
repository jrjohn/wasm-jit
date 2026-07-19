//! 聲塵器 — the sound-dust engine.
//!
//! 楞嚴三律, kept by construction:
//!   動則有聲 — sound exists only as an EVENT someone caused (a drop spawned,
//!              a strike struck). No cause, no sample. Silence is the default.
//!   根塵和合 — every seat has a position; what you hear is the meeting of
//!              source and listener (distance gain + equal-power pan).
//!   聲從身出 — a being's sound sits at the being's coordinates (its seat).
//!
//! The engine provides the PHYSICS of primitives (a raindrop's 4–15ms band-passed
//! ping, a bubble's rising Minnaert sine, a chirp's swept tone, a bell's five
//! inharmonic partials, the throat's glottal pulse through vowel formants).
//! WHEN and HOW OFTEN those events happen is not the engine's business — that is
//! the seed cell's 自性. The engine is a throat and a sky, not a song.
//!
//! Compiled with `--target wasm32-unknown-unknown` (no wasm-bindgen) the module
//! imports NOTHING: it cannot touch the world; it can only vibrate.

// ---------------------------------------------------------------- rng --------

/// xorshift32 — our own randomness, so the wasm needs no `getrandom` import.
#[derive(Clone)]
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Self {
        Rng(if seed == 0 { 0x9E37_79B9 } else { seed })
    }
    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    /// uniform in [0,1)
    #[inline]
    pub fn f(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / 16_777_216.0
    }
    /// uniform in [-1,1)
    #[inline]
    pub fn bi(&mut self) -> f32 {
        self.f() * 2.0 - 1.0
    }
}

// ------------------------------------------------------------- filters ------

/// Two-pole resonator (band-pass): the body of a raindrop ping and of a vowel
/// formant. y = 2 r cos(w) y1 - r^2 y2 + g x, g normalizes for r.
#[derive(Clone, Copy, Default)]
struct Reso {
    b1: f32,
    b2: f32,
    g: f32,
    y1: f32,
    y2: f32,
}

impl Reso {
    fn tune(&mut self, sr: f32, cf: f32, r: f32) {
        let w = core::f32::consts::TAU * (cf / sr).clamp(0.0005, 0.45);
        self.b1 = 2.0 * r * w.cos();
        self.b2 = -r * r;
        self.g = (1.0 - r * r) * w.sin(); // ≈ unit gain at resonance, cf-independent
    }
    #[inline]
    fn run(&mut self, x: f32) -> f32 {
        let y = self.b1 * self.y1 + self.b2 * self.y2 + self.g * x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

/// One-pole lowpass — the nasal murmur, the rain bed, smoothing of control values.
#[derive(Clone, Copy, Default)]
struct Lp {
    k: f32,
    y: f32,
}

impl Lp {
    fn tune(&mut self, sr: f32, cf: f32) {
        self.k = (core::f32::consts::TAU * cf / sr).clamp(0.0, 1.0);
    }
    #[inline]
    fn run(&mut self, x: f32) -> f32 {
        self.y += self.k * (x - self.y);
        self.y
    }
}

// -------------------------------------------------------------- events ------

const MAX_EVENTS: usize = 64; // per seat; overflow drops the oldest (no panic)

/// One transient sound-event: a drop, a bubble, a chirp or a strike.
/// All share (age, dur); the union of per-kind state is small enough to inline.
#[derive(Clone, Copy)]
enum Event {
    /// a raindrop: noise burst through a resonator, exponential decay
    Drop { t: f32, dur: f32, amp: f32, bp: Reso },
    /// a water bubble: decaying sine whose pitch RISES (Minnaert/van den Doel)
    Bubble { t: f32, dur: f32, amp: f32, f: f32, ph: f32 },
    /// a bird syllable: sine swept f1→f2 under a Hann window (+2nd harmonic sheen)
    Chirp { t: f32, dur: f32, amp: f32, f1: f32, f2: f32, ph: f32 },
    /// a modal strike: five inharmonic partials, each its own decay; the first
    /// is a detuned PAIR so a big bell warbles (beats) the way temple bells do
    Strike { t: f32, amp: f32, f0: f32, ph: [f32; 6] },
}

/// bell partials: ratio, relative amp, decay seconds (scaled by energy)
const BELL: [(f32, f32, f32); 5] = [
    (1.0, 1.0, 6.0),
    (2.0, 0.55, 3.4),
    (2.98, 0.38, 2.0),
    (4.2, 0.22, 1.2),
    (5.4, 0.13, 0.8),
];

impl Event {
    /// render one sample; returns (value, still_alive)
    #[inline]
    fn run(&mut self, sr: f32, rng: &mut Rng) -> (f32, bool) {
        let dt = 1.0 / sr;
        match self {
            Event::Drop { t, dur, amp, bp } => {
                *t += dt;
                if *t >= *dur {
                    return (0.0, false);
                }
                // a sharp tick that dies fast — drops must stay SEPARATE, not smear.
                // The landing "tak" is the SAME watery body driven harder for ~1ms —
                // a raw broadband click reads as static electricity, not water
                let env = (-14.0 * *t / *dur).exp();
                let drive = if *t < 0.0012 { 3.2 } else { 1.0 };
                (bp.run(rng.bi() * drive) * env * *amp, true)
            }
            Event::Bubble { t, dur, amp, f, ph } => {
                *t += dt;
                if *t >= *dur {
                    return (0.0, false);
                }
                // the pitch sweeps UP as the bubble shrinks — the "blip" of water
                let fi = *f * (1.0 + 0.6 * *t / *dur);
                *ph += core::f32::consts::TAU * fi * dt;
                let env = (-6.0 * *t / *dur).exp();
                ((*ph).sin() * env * *amp, true)
            }
            Event::Chirp { t, dur, amp, f1, f2, ph } => {
                *t += dt;
                if *t >= *dur {
                    return (0.0, false);
                }
                let u = *t / *dur;
                let fi = *f1 + (*f2 - *f1) * u;
                *ph += core::f32::consts::TAU * fi * dt;
                let w = (core::f32::consts::PI * u).sin(); // Hann-ish window
                let s = (*ph).sin() + 0.35 * (2.0 * *ph).sin(); // a little sheen
                (s * w * w * *amp, true)
            }
            Event::Strike { t, amp, f0, ph } => {
                *t += dt;
                let mut v = 0.0;
                let mut alive = false;
                for (i, (ratio, a, dec)) in BELL.iter().enumerate() {
                    let env = (-*t / dec).exp();
                    if env > 0.001 {
                        alive = true;
                    }
                    ph[i] += core::f32::consts::TAU * *f0 * ratio * dt;
                    v += ph[i].sin() * a * env;
                }
                // the hum note's detuned twin — the warble of a big bell
                ph[5] += core::f32::consts::TAU * *f0 * 1.004 * dt;
                v += ph[5].sin() * 0.8 * (-*t / BELL[0].2).exp();
                (v * *amp * 0.28, alive)
            }
        }
    }
}

// --------------------------------------------------------------- voice ------

/// vowel formant targets (F1,F2,F3 in Hz + per-formant gains):
/// index 0 = o(嗡), 1 = a(阿), 2 = u(吽) — the mantra's three mouths.
const VOWELS: [([f32; 3], [f32; 3]); 5] = [
    ([500.0, 900.0, 2450.0], [1.0, 0.70, 0.20]), // 0 o
    ([800.0, 1150.0, 2800.0], [1.0, 0.80, 0.30]), // 1 a
    ([330.0, 700.0, 2200.0], [1.0, 0.55, 0.15]), // 2 u
    ([320.0, 2350.0, 3000.0], [1.0, 0.45, 0.22]), // 3 i
    ([520.0, 1850.0, 2550.0], [1.0, 0.60, 0.25]), // 4 e
];

/// The throat: glottal pulse → three vowel formants → nasal murmur (the m of
/// 嗡/吽). Continuous — the seed steers f0/vowel/nasal; f0<=0 closes the throat.
#[derive(Clone, Copy, Default)]
struct Voice {
    f0_t: f32, // targets (what the seed last asked for)
    vow_t: f32,
    nas_t: f32,
    f0: f32, // smoothed actuals (a real throat cannot jump)
    vow: f32,
    nas: f32,
    level: f32, // smoothed on/off so syllables don't click
    ph: f32,
    vib: f32,
    formants: [Reso; 3],
    fgain: [f32; 3],
    murmur: Lp,
    soften: Lp,
    ctrl: u32, // control-rate divider for formant retuning
    // consonants: frication is UNVOICED — a hiss shaped by the mouth, alive
    // even while the glottis is closed (s, x, h; a stop's release burst)
    fcf_t: f32,
    flvl_t: f32,
    flvl: f32,
    fric: Reso,
    fctrl: u32,
}

impl Voice {
    fn set_fric(&mut self, cf: f32, level: f32) {
        self.fcf_t = cf.clamp(300.0, 8000.0);
        self.flvl_t = level.clamp(0.0, 1.0);
    }

    fn set(&mut self, f0: f32, vowel: f32, nasal: f32) {
        self.f0_t = f0.clamp(0.0, 1000.0);
        self.vow_t = vowel.clamp(0.0, 4.0);
        self.nas_t = nasal.clamp(0.0, 1.0);
    }

    fn retune(&mut self, sr: f32) {
        // interpolate the vowel targets: 0..1 = o→a, 1..2 = a→u
        let ia = (self.vow.floor() as usize).min(3);
        let (ia, ib, u) = (ia, ia + 1, self.vow - ia as f32);
        let (fa, ga) = VOWELS[ia];
        let (fb, gb) = VOWELS[ib];
        // register: a higher voice is a SHORTER vocal tract — above f0≈150Hz the
        // formants migrate up (to ~+22% at soprano), which is what "female" IS
        // acoustically; a chanting 翁 at 110Hz is untouched
        let reg = ((self.f0 - 150.0) / 130.0).clamp(0.0, 1.0);
        let tract = 1.0 + 0.22 * reg;
        for k in 0..3 {
            let cf = (fa[k] + (fb[k] - fa[k]) * u) * tract;
            self.formants[k].tune(sr, cf, 0.995);
            // nasal closure damps the mouth's formants
            self.fgain[k] = (ga[k] + (gb[k] - ga[k]) * u) * (1.0 - 0.7 * self.nas);
        }
        self.murmur.tune(sr, 280.0);
        self.soften.tune(sr, 1800.0);
    }

    #[inline]
    fn run(&mut self, sr: f32, rng: &mut Rng) -> f32 {
        let dt = 1.0 / sr;
        // smooth every control toward its target (~25ms) — no clicks, real glide
        let k = 1.0 - (-dt / 0.025).exp();
        self.f0 += k * (self.f0_t - self.f0);
        self.vow += k * (self.vow_t - self.vow);
        self.nas += k * (self.nas_t - self.nas);
        let want = if self.f0_t > 20.0 { 1.0 } else { 0.0 };
        self.level += (1.0 - (-dt / 0.06).exp()) * (want - self.level);
        // frication first — it sounds through a closed throat (s… before 蓑's uo)
        self.flvl += (1.0 - (-dt / 0.012).exp()) * (self.flvl_t - self.flvl);
        let mut hiss = 0.0;
        if self.flvl > 0.001 {
            if self.fctrl == 0 {
                self.fric.tune(sr, self.fcf_t.max(300.0), 0.965);
                self.fctrl = 32;
            }
            self.fctrl -= 1;
            hiss = self.fric.run(rng.bi()) * self.flvl * 1.15;
        }
        if self.level < 0.001 {
            return hiss;
        }
        if self.ctrl == 0 {
            self.retune(sr);
            self.ctrl = 32;
        }
        self.ctrl -= 1;
        // vibrato — the living unevenness of a held tone; a singing (higher)
        // register carries a slightly deeper vibrato and more air in the tone
        let reg = ((self.f0 - 150.0) / 130.0).clamp(0.0, 1.0);
        self.vib += core::f32::consts::TAU * (5.3 + 0.6 * reg) * dt;
        let f = self.f0.max(20.0) * (1.0 + (0.011 + 0.007 * reg) * self.vib.sin());
        self.ph = (self.ph + f * dt).fract();
        // glottal source: soft saw + breath
        let saw = 2.0 * self.ph - 1.0;
        let src = self.soften.run(saw) + rng.bi() * (0.015 + 0.022 * reg);
        // three formants carve the buzz into a mouth
        let mut v = 0.0;
        for kf in 0..3 {
            v += self.formants[kf].run(src) * self.fgain[kf];
        }
        // the nasal hum — a colour on the voice, not its master: the probe showed
        // murmur 10x the formants, making loudness track nasal (阿 faint, 吽 booming)
        v = v * 1.5 + self.murmur.run(src) * self.nas * 0.33;
        v * 1.25 * self.level + hiss
    }
}

// -------------------------------------------------------------- breath ------

/// Wind / breath / a river's bed: band-passed noise whose centre WANDERS
/// (vortex whistling) under a slowly gusting envelope.
#[derive(Clone, Copy, Default)]
struct Breath {
    level_t: f32,
    level: f32,
    cf: f32,
    bp: Reso,
    gust: f32,
    gust_lp: Lp,
    ctrl: u32,
}

impl Breath {
    #[inline]
    fn run(&mut self, sr: f32, rng: &mut Rng) -> f32 {
        let dt = 1.0 / sr;
        self.level += (1.0 - (-dt / 0.2).exp()) * (self.level_t - self.level);
        if self.level < 0.001 {
            return 0.0;
        }
        if self.ctrl == 0 {
            // centre frequency random-walks 180–900 Hz; gusts drift slowly
            if self.cf < 180.0 {
                self.cf = 420.0;
                self.gust_lp.tune(sr, 0.5);
                self.gust_lp.y = 0.5; // begin mid-breath, not from vacuum
            }
            self.cf = (self.cf + rng.bi() * 40.0).clamp(180.0, 900.0);
            self.bp.tune(sr, self.cf, 0.985);
            self.gust = self.gust_lp.run(0.55 + 0.45 * rng.bi());
            self.ctrl = 256;
        }
        self.ctrl -= 1;
        self.bp.run(rng.bi()) * self.level * (0.4 + 0.6 * self.gust) * 1.7
    }
}

// --------------------------------------------------------------- seats ------

/// One seat = one sound cell's place in the world: a position, its live events,
/// its continuous voice and breath. 聲從身出 — the seat IS the body's location.
struct Seat {
    alive: bool,
    positional: bool,
    x: f32,
    y: f32,
    events: [Option<Event>; MAX_EVENTS],
    next_ev: usize, // ring cursor: overflow overwrites the oldest
    voice: Voice,
    breath: Breath,
}

impl Seat {
    fn new(positional: bool) -> Self {
        Seat {
            alive: true,
            positional,
            x: 0.0,
            y: 0.0,
            events: [None; MAX_EVENTS],
            next_ev: 0,
            voice: Voice::default(),
            breath: Breath::default(),
        }
    }
    fn push(&mut self, ev: Event) {
        // find a free slot from the cursor; if the pool is full, the oldest dies
        for _ in 0..MAX_EVENTS {
            let i = self.next_ev % MAX_EVENTS;
            self.next_ev = self.next_ev.wrapping_add(1);
            if self.events[i].is_none() {
                self.events[i] = Some(ev);
                return;
            }
        }
        let i = self.next_ev % MAX_EVENTS;
        self.next_ev = self.next_ev.wrapping_add(1);
        self.events[i] = Some(ev);
    }
}

// -------------------------------------------------------------- engine ------

pub const MAX_FRAMES: usize = 256;
const MAX_SEATS: usize = 24;

pub struct Engine {
    sr: f32,
    rng: Rng,
    lx: f32,
    ly: f32,
    seats: Vec<Seat>,
    pub out: [f32; MAX_FRAMES * 2], // interleaved stereo
}

impl Engine {
    pub fn new(sr: f32) -> Self {
        Engine {
            sr: sr.clamp(8000.0, 192_000.0),
            rng: Rng::new(0x5EED_0DDF),
            lx: 0.0,
            ly: 0.0,
            seats: Vec::new(),
            out: [0.0; MAX_FRAMES * 2],
        }
    }

    pub fn listener(&mut self, x: f32, y: f32) {
        self.lx = x;
        self.ly = y;
    }

    pub fn seat_add(&mut self, positional: bool) -> u32 {
        // reuse a dead seat first
        for (i, s) in self.seats.iter_mut().enumerate() {
            if !s.alive {
                *s = Seat::new(positional);
                return i as u32;
            }
        }
        if self.seats.len() >= MAX_SEATS {
            return u32::MAX; // the world is full of mouths — refuse politely
        }
        self.seats.push(Seat::new(positional));
        (self.seats.len() - 1) as u32
    }

    pub fn seat_pos(&mut self, id: u32, x: f32, y: f32) {
        if let Some(s) = self.seats.get_mut(id as usize) {
            s.x = x;
            s.y = y;
        }
    }

    pub fn seat_remove(&mut self, id: u32) {
        if let Some(s) = self.seats.get_mut(id as usize) {
            s.alive = false;
        }
    }

    pub fn clear(&mut self) {
        self.seats.clear();
    }

    fn seat(&mut self, id: u32) -> Option<&mut Seat> {
        self.seats.get_mut(id as usize).filter(|s| s.alive)
    }

    // ---- the primitives: physics only; cause is the caller's 自性 ----

    pub fn ev_drop(&mut self, id: u32, bright: f32) {
        let sr = self.sr;
        let (cf, dur, amp) = {
            let b = bright.clamp(0.0, 1.0);
            let r = &mut self.rng;
            (
                900.0 + b * 5200.0 * (0.6 + 0.4 * r.f()),
                0.003 + 0.006 * r.f(),
                {
                    // most drops small, a few fat — the skew is what reads as "rain"
                    let u = r.f();
                    3.0 + 6.0 * u * u
                },
            )
        };
        if let Some(seat) = self.seat(id) {
            let mut bp = Reso::default();
            bp.tune(sr, cf, 0.994);
            seat.push(Event::Drop { t: 0.0, dur, amp, bp });
        }
    }

    pub fn ev_bubble(&mut self, id: u32, pitch: f32) {
        let (f, dur, amp) = {
            let p = pitch.clamp(0.0, 1.0);
            let r = &mut self.rng;
            (
                400.0 + p * 2600.0 * (0.75 + 0.25 * r.f()),
                0.018 + 0.045 * r.f(),
                0.045 + 0.055 * r.f(), // a small thing in a large water — the bed carries the river
            )
        };
        if let Some(seat) = self.seat(id) {
            seat.push(Event::Bubble { t: 0.0, dur, amp, f, ph: 0.0 });
        }
    }

    pub fn ev_chirp(&mut self, id: u32, f1: f32, f2: f32, dur: f32) {
        let f1 = f1.clamp(300.0, 9000.0);
        let f2 = f2.clamp(300.0, 9000.0);
        let dur = dur.clamp(0.015, 0.3); // a syllable, not a siren
        if let Some(seat) = self.seat(id) {
            seat.push(Event::Chirp { t: 0.0, dur, amp: 0.22, f1, f2, ph: 0.0 });
        }
    }

    pub fn ev_strike(&mut self, id: u32, f0: f32, energy: f32) {
        let f0 = f0.clamp(40.0, 2000.0);
        let amp = energy.clamp(0.0, 1.0);
        if let Some(seat) = self.seat(id) {
            seat.push(Event::Strike { t: 0.0, amp, f0, ph: [0.0; 6] });
        }
    }

    pub fn voice_set(&mut self, id: u32, f0: f32, vowel: f32, nasal: f32) {
        if let Some(seat) = self.seat(id) {
            seat.voice.set(f0, vowel, nasal);
        }
    }

    pub fn voice_fric(&mut self, id: u32, cf: f32, level: f32) {
        if let Some(seat) = self.seat(id) {
            seat.voice.set_fric(cf, level);
        }
    }

    pub fn breath_set(&mut self, id: u32, level: f32) {
        if let Some(seat) = self.seat(id) {
            seat.breath.level_t = level.clamp(0.0, 1.0);
        }
    }

    // ---- 根塵和合: what reaches the ear is source × meeting ----

    #[inline]
    fn meet(&self, seat: &Seat) -> (f32, f32) {
        if !seat.positional {
            return (0.7071, 0.7071); // ambient: everywhere, centred
        }
        let dx = seat.x - self.lx;
        let dy = seat.y - self.ly;
        let dist = (dx * dx + dy * dy).sqrt();
        let g = 1.0 / (1.0 + dist * 0.22); // walk closer, hear more
        let pan = (dx * 0.08).clamp(-1.0, 1.0);
        let a = (pan + 1.0) * core::f32::consts::FRAC_PI_4;
        (g * a.cos(), g * a.sin())
    }

    /// render `frames` (≤ MAX_FRAMES) interleaved-stereo samples into `self.out`
    pub fn render(&mut self, frames: usize) {
        let frames = frames.min(MAX_FRAMES);
        let sr = self.sr;
        self.out[..frames * 2].fill(0.0);
        for si in 0..self.seats.len() {
            if !self.seats[si].alive {
                continue;
            }
            let (gl, gr) = self.meet(&self.seats[si]);
            if gl < 0.0005 && gr < 0.0005 {
                continue; // too far to meet the ear — but its events still age below? no: unheard is unrendered (真: 不和合則不聞)
            }
            let seat = &mut self.seats[si];
            for fi in 0..frames {
                let mut v = 0.0;
                for ev in seat.events.iter_mut() {
                    if let Some(e) = ev {
                        let (s, alive) = e.run(sr, &mut self.rng);
                        v += s;
                        if !alive {
                            *ev = None;
                        }
                    }
                }
                v += seat.voice.run(sr, &mut self.rng);
                v += seat.breath.run(sr, &mut self.rng);
                // soft ceiling per seat, then the meeting law
                let v = v / (1.0 + v.abs() * 0.4);
                self.out[fi * 2] += v * gl;
                self.out[fi * 2 + 1] += v * gr;
            }
        }
        // master soft-clip: many mouths may not tear one sky
        for s in self.out[..frames * 2].iter_mut() {
            *s = (*s * 0.9).clamp(-1.0, 1.0);
        }
    }
}

// ----------------------------------------------------------- wasm ABI -------
// raw extern "C" — no wasm-bindgen, no imports. The audio thread instantiates
// with an EMPTY import object and drives these directly.

use std::sync::Mutex;

static ENGINE: Mutex<Option<Engine>> = Mutex::new(None);

fn with<T>(f: impl FnOnce(&mut Engine) -> T, default: T) -> T {
    match ENGINE.lock() {
        Ok(mut g) => match g.as_mut() {
            Some(e) => f(e),
            None => default,
        },
        Err(_) => default,
    }
}

#[no_mangle]
pub extern "C" fn init(sample_rate: f32) {
    if let Ok(mut g) = ENGINE.lock() {
        *g = Some(Engine::new(sample_rate));
    }
}

#[no_mangle]
pub extern "C" fn seat_add(positional: u32) -> u32 {
    with(|e| e.seat_add(positional != 0), u32::MAX)
}

#[no_mangle]
pub extern "C" fn seat_pos(id: u32, x: f32, y: f32) {
    with(|e| e.seat_pos(id, x, y), ())
}

#[no_mangle]
pub extern "C" fn seat_remove(id: u32) {
    with(|e| e.seat_remove(id), ())
}

#[no_mangle]
pub extern "C" fn clear() {
    with(|e| e.clear(), ())
}

#[no_mangle]
pub extern "C" fn listener(x: f32, y: f32) {
    with(|e| e.listener(x, y), ())
}

#[no_mangle]
pub extern "C" fn ev_drop(seat: u32, bright: f32) {
    with(|e| e.ev_drop(seat, bright), ())
}

#[no_mangle]
pub extern "C" fn ev_bubble(seat: u32, pitch: f32) {
    with(|e| e.ev_bubble(seat, pitch), ())
}

#[no_mangle]
pub extern "C" fn ev_chirp(seat: u32, f1: f32, f2: f32, dur: f32) {
    with(|e| e.ev_chirp(seat, f1, f2, dur), ())
}

#[no_mangle]
pub extern "C" fn ev_strike(seat: u32, f0: f32, energy: f32) {
    with(|e| e.ev_strike(seat, f0, energy), ())
}

#[no_mangle]
pub extern "C" fn voice_set(seat: u32, f0: f32, vowel: f32, nasal: f32) {
    with(|e| e.voice_set(seat, f0, vowel, nasal), ())
}

#[no_mangle]
pub extern "C" fn voice_fric(seat: u32, cf: f32, level: f32) {
    with(|e| e.voice_fric(seat, cf, level), ())
}

#[no_mangle]
pub extern "C" fn breath_set(seat: u32, level: f32) {
    with(|e| e.breath_set(seat, level), ())
}

#[no_mangle]
pub extern "C" fn out_ptr() -> *const f32 {
    with(|e| e.out.as_ptr(), core::ptr::null())
}

#[no_mangle]
pub extern "C" fn render(frames: u32) {
    with(|e| e.render(frames as usize), ())
}

// --------------------------------------------------------------- tests ------

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 44_100.0;

    fn rms(e: &Engine, frames: usize) -> f32 {
        let n = frames.min(MAX_FRAMES) * 2;
        let s: f32 = e.out[..n].iter().map(|v| v * v).sum();
        (s / n as f32).sqrt()
    }

    /// render `secs` and return overall rms
    fn run_secs(e: &mut Engine, secs: f32) -> f32 {
        let blocks = (secs * SR / 128.0) as usize;
        let mut acc = 0.0;
        for _ in 0..blocks {
            e.render(128);
            let r = rms(e, 128);
            acc += r * r;
        }
        (acc / blocks.max(1) as f32).sqrt()
    }

    /// Goertzel power of frequency f over the last rendered block sequence
    fn goertzel(samples: &[f32], sr: f32, f: f32) -> f32 {
        let w = core::f32::consts::TAU * f / sr;
        let c = 2.0 * w.cos();
        let (mut s0, mut s1, mut s2) = (0.0f32, 0.0f32, 0.0f32);
        for &x in samples {
            s0 = x + c * s1 - s2;
            s2 = s1;
            s1 = s0;
        }
        (s1 * s1 + s2 * s2 - c * s1 * s2) / samples.len() as f32
    }

    /// collect `secs` of MONO samples (left channel)
    fn collect(e: &mut Engine, secs: f32) -> Vec<f32> {
        let blocks = (secs * SR / 128.0) as usize;
        let mut v = Vec::with_capacity(blocks * 128);
        for _ in 0..blocks {
            e.render(128);
            for i in 0..128 {
                v.push(e.out[i * 2]);
            }
        }
        v
    }

    #[test]
    fn silence_without_cause() {
        // 動則有聲: no event, no sound — silence is the default, BY LAW
        let mut e = Engine::new(SR);
        let s = e.seat_add(false);
        assert_eq!(s, 0);
        let r = run_secs(&mut e, 0.5);
        assert!(r < 1e-6, "uncaused sound leaked: rms={r}");
    }

    #[test]
    fn drops_sound_and_die() {
        let mut e = Engine::new(SR);
        let s = e.seat_add(false);
        for _ in 0..40 {
            e.ev_drop(s, 0.6);
        }
        let r = run_secs(&mut e, 0.1);
        assert!(r > 0.005, "40 raindrops were inaudible: rms={r}");
        // all drops are ≤15ms — after half a second the sky must be silent again
        let quiet = run_secs(&mut e, 0.3);
        assert!(quiet < 1e-5, "drops refused to die: rms={quiet}");
    }

    #[test]
    fn meeting_law_distance() {
        // 根塵和合: the same bubbling spring, heard near and heard far
        let hear = |lx: f32| {
            let mut e = Engine::new(SR);
            let s = e.seat_add(true);
            e.seat_pos(s, 10.0, 10.0);
            e.listener(lx, 10.0);
            let mut acc = 0.0;
            let blocks = 200;
            for b in 0..blocks {
                if b % 10 == 0 {
                    e.ev_bubble(s, 0.5);
                }
                e.render(128);
                let r = rms(&e, 128);
                acc += r * r;
            }
            (acc / blocks as f32).sqrt()
        };
        let near = hear(12.0); // 2 cells away
        let far = hear(60.0); // 50 cells away
        assert!(
            near > far * 3.0,
            "meeting law broken: near={near} far={far}"
        );
    }

    #[test]
    fn bell_decays() {
        // 擊則有, 不擊則無 — and the ring must FADE, like a struck thing
        let mut e = Engine::new(SR);
        let s = e.seat_add(false);
        let before = run_secs(&mut e, 0.2);
        assert!(before < 1e-6, "the bell rang before it was struck");
        e.ev_strike(s, 130.0, 0.9);
        let early = run_secs(&mut e, 1.0);
        let mid = run_secs(&mut e, 1.0);
        let late = run_secs(&mut e, 2.0);
        assert!(early > 0.01, "the strike was inaudible: {early}");
        assert!(early > mid && mid > late, "the bell does not decay: {early} {mid} {late}");
    }

    #[test]
    fn chant_om_has_harmonics_and_formants() {
        // the throat is a VOICE, not a sine: harmonics above f0 must carry power
        let mut e = Engine::new(SR);
        let s = e.seat_add(false);
        e.voice_set(s, 110.0, 0.0, 0.6); // 嗡: o-vowel, nasal
        let _warm = collect(&mut e, 0.3); // let the envelope open
        let body = collect(&mut e, 1.0);
        let p1 = goertzel(&body, SR, 110.0);
        let p2 = goertzel(&body, SR, 220.0);
        let p3 = goertzel(&body, SR, 330.0);
        let p4 = goertzel(&body, SR, 440.0);
        assert!(p1 > 0.0, "no fundamental");
        let harm = p2 + p3 + p4;
        assert!(
            harm > p1 * 0.05,
            "voice is a bare sine (no harmonics): p1={p1} harm={harm}"
        );
        // and it must actually sound
        let r: f32 = (body.iter().map(|v| v * v).sum::<f32>() / body.len() as f32).sqrt();
        assert!(r > 0.01, "the chant is inaudible: rms={r}");
    }

    #[test]
    fn chant_closes_when_f0_zero() {
        let mut e = Engine::new(SR);
        let s = e.seat_add(false);
        e.voice_set(s, 110.0, 1.0, 0.0);
        let open = run_secs(&mut e, 0.5);
        assert!(open > 0.01);
        e.voice_set(s, 0.0, 1.0, 0.0); // close the throat
        let _release = run_secs(&mut e, 0.4);
        let closed = run_secs(&mut e, 0.3);
        assert!(closed < 1e-4, "the throat will not close: {closed}");
    }

    #[test]
    fn chirp_duration_is_clamped() {
        // a syllable, not a siren: dur is capped at 0.3s
        let mut e = Engine::new(SR);
        let s = e.seat_add(false);
        e.ev_chirp(s, 2500.0, 3500.0, 60.0); // absurd ask
        let _first = run_secs(&mut e, 0.35);
        let after = run_secs(&mut e, 0.2);
        assert!(after < 1e-5, "chirp outlived its clamp: {after}");
    }

    #[test]
    fn event_pool_overflow_is_graceful() {
        let mut e = Engine::new(SR);
        let s = e.seat_add(false);
        for _ in 0..5000 {
            e.ev_drop(s, 0.5); // a monsoon — must not panic, oldest die
        }
        e.render(128);
        let r = rms(&e, 128);
        assert!(r.is_finite() && r > 0.0);
    }

    #[test]
    fn breath_rises_and_falls() {
        let mut e = Engine::new(SR);
        let s = e.seat_add(false);
        e.breath_set(s, 0.7);
        let blowing = run_secs(&mut e, 1.0);
        assert!(blowing > 0.01, "wind did not blow: {blowing}");
        e.breath_set(s, 0.0);
        let _settle = run_secs(&mut e, 1.0);
        let calm = run_secs(&mut e, 0.5);
        assert!(calm < 1e-3, "wind will not still: {calm}");
    }

    #[test]
    fn frication_sounds_through_a_closed_throat() {
        // s/x/h are UNVOICED: throat closed (f0=0), hiss on -> sound; hiss off -> silence
        let mut e = Engine::new(SR);
        let s = e.seat_add(false);
        e.voice_set(s, 0.0, 1.0, 0.0);
        e.voice_fric(s, 5500.0, 0.6);
        let hissing = run_secs(&mut e, 0.4);
        assert!(hissing > 0.005, "the closed-throat hiss was silent: {hissing}");
        e.voice_fric(s, 5500.0, 0.0);
        let _settle = run_secs(&mut e, 0.2);
        let quiet = run_secs(&mut e, 0.2);
        assert!(quiet < 1e-4, "the hiss would not stop: {quiet}");
    }

    #[test]
    fn seats_are_bounded_and_reusable() {
        let mut e = Engine::new(SR);
        for _ in 0..MAX_SEATS {
            assert_ne!(e.seat_add(false), u32::MAX);
        }
        assert_eq!(e.seat_add(false), u32::MAX, "the world grew a 25th mouth");
        e.seat_remove(3);
        assert_eq!(e.seat_add(true), 3, "a dead seat was not reused");
    }


    #[test]
    #[ignore] // diagnostic probe — run with --ignored --nocapture
    fn probe_mantra() {
        let mut e = Engine::new(SR);
        let s = e.seat_add(false);
        const SCORE: [(f32, f32, f32, f32); 9] = [
            (0.0, 108.0, 0.0, 0.60),
            (0.8, 110.0, 0.0, 0.25),
            (2.6, 110.0, 0.0, 0.80),
            (3.4, 114.0, 1.0, 0.05),
            (6.2, 114.0, 1.0, 0.10),
            (7.0, 106.0, 2.0, 0.30),
            (9.0, 102.0, 2.0, 0.85),
            (10.6, 100.0, 2.0, 0.95),
            (11.4, 0.0, 2.0, 0.95),
        ];
        let blocks = (12.0 * SR / 128.0) as usize;
        for b in 0..blocks {
            let tt = b as f32 * 128.0 / SR;
            let mut ctl = (0.0f32, 2.0f32, 0.95f32);
            for w in SCORE.windows(2) {
                let (t0, f0a, va, na) = w[0];
                let (t1, f0b, vb, nb) = w[1];
                if tt >= t0 && tt < t1 {
                    let u = (tt - t0) / (t1 - t0);
                    ctl = (f0a + (f0b - f0a) * u, va + (vb - va) * u, na + (nb - na) * u);
                    break;
                }
            }
            e.voice_set(s, ctl.0, ctl.1, ctl.2);
            e.render(128);
            if b % 172 == 0 { // ~every 0.5s
                let r = {
                    let n = 128 * 2;
                    let ss: f32 = e.out[..n].iter().map(|v| v * v).sum();
                    (ss / n as f32).sqrt()
                };
                let v = &e.seats[0].voice;
                println!(
                    "t={tt:5.2} ctl=({:6.1},{:4.2},{:4.2}) f0={:6.1} vow={:4.2} nas={:4.2} lvl={:4.2} rms={r:.4} y1[0]={:9.4} y1[1]={:9.4} y1[2]={:9.4} mur={:9.4}",
                    ctl.0, ctl.1, ctl.2, v.f0, v.vow, v.nas, v.level,
                    v.formants[0].y1, v.formants[1].y1, v.formants[2].y1, v.murmur.y
                );
            }
        }
    }

    #[test]
    fn abi_smoke() {
        // drive the extern "C" surface exactly as the worklet will
        init(48_000.0);
        let s = seat_add(0);
        ev_drop(s, 0.5);
        ev_strike(s, 200.0, 0.5);
        voice_set(s, 140.0, 1.0, 0.2);
        breath_set(s, 0.3);
        listener(0.0, 0.0);
        render(128);
        let p = out_ptr();
        assert!(!p.is_null());
        // the buffer must contain live audio
        let heard = with(|e| rms(e, 128), 0.0);
        assert!(heard > 0.0);
    }
}
