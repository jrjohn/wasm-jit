//! Render the 聲塵器's primitives to .wav files for HUMAN ears.
//! Usage: cargo run --release -p shengchen --example demo -- <outdir>
//!
//! In Phase B the "conductor" loops below become seed cells (自性);
//! here the demo plays conductor so the physics can be judged alone.

use shengchen::Engine;
use std::io::Write;

const SR: f32 = 44_100.0;

/// minimal 16-bit mono WAV writer — no dependencies, 44-byte header
fn write_wav(path: &std::path::Path, samples: &[f32]) -> std::io::Result<()> {
    let n = samples.len() as u32;
    let data_len = n * 2;
    let mut f = std::fs::File::create(path)?;
    f.write_all(b"RIFF")?;
    f.write_all(&(36 + data_len).to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?; // PCM
    f.write_all(&1u16.to_le_bytes())?; // mono
    f.write_all(&(SR as u32).to_le_bytes())?;
    f.write_all(&((SR as u32) * 2).to_le_bytes())?;
    f.write_all(&2u16.to_le_bytes())?;
    f.write_all(&16u16.to_le_bytes())?;
    f.write_all(b"data")?;
    f.write_all(&data_len.to_le_bytes())?;
    for &s in samples {
        f.write_all(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())?;
    }
    Ok(())
}

/// run `secs`, calling `conduct(block_index, engine)` before each 128-frame block;
/// collects the LEFT channel (demo scenes are centred/mono anyway)
fn perform(e: &mut Engine, secs: f32, mut conduct: impl FnMut(usize, &mut Engine)) -> Vec<f32> {
    let blocks = (secs * SR / 128.0) as usize;
    let mut out = Vec::with_capacity(blocks * 128);
    for b in 0..blocks {
        conduct(b, e);
        e.render(128);
        for i in 0..128 {
            out.push(e.out[i * 2]);
        }
    }
    out
}

/// block index → seconds
fn t(b: usize) -> f32 {
    b as f32 * 128.0 / SR
}

fn main() {
    let dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/shengchen".into());
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).expect("mkdir outdir");
    let mut rng_state = 0x1234_5678u32;
    let mut rnd = move || {
        rng_state ^= rng_state << 13;
        rng_state ^= rng_state >> 17;
        rng_state ^= rng_state << 5;
        (rng_state >> 8) as f32 / 16_777_216.0
    };

    // ---- 1. 嗡阿吽 — ONE BREATH: the throat never stops, only the MOUTH
    // changes shape. o→a→u morph continuously; the nasal closes and opens;
    // the tone glides. 字字相連 — that is what a mantra is.
    {
        let mut e = Engine::new(SR);
        let s = e.seat_add(false);
        // piecewise-linear score over one breath: (time, f0, vowel, nasal)
        const SCORE: [(f32, f32, f32, f32); 9] = [
            (0.0, 108.0, 0.0, 0.60), // 嗡 begins already humming (m...)
            (0.8, 110.0, 0.0, 0.25), // ...opens into o
            (2.6, 110.0, 0.0, 0.80), // ...and closes back toward the nose
            (3.4, 114.0, 1.0, 0.05), // MORPH into 阿 — no gap, the mouth opens
            (6.2, 114.0, 1.0, 0.10), // 阿 held, open
            (7.0, 104.0, 2.0, 0.35), // MORPH into 吽 — lips rounding
            (8.2, 103.0, 2.0, 0.92), // 吽 sinks fast into the deep hum (short)
            (9.25, 103.0, 2.0, 0.95), // hold the hum — PITCH STEADY, no dive
            (9.4, 0.0, 2.0, 0.95),   // throat closes; the 60ms envelope fades it
        ];
        let wav = perform(&mut e, 10.0, |b, e| {
            let tt = t(b);
            // interpolate the score — every control glides, nothing jumps;
            // the throat's own 60ms envelope rounds the final breath-out
            let mut ctl = (0.0, 2.0, 0.95);
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
        });
        write_wav(&dir.join("1-om-ah-hum.wav"), &wav).unwrap();
    }

    // ---- 2. rain — every drop a separate decision ----
    {
        let mut e = Engine::new(SR);
        let s = e.seat_add(false);
        e.breath_set(s, 0.02); // almost nothing — the drops ARE the rain
        let wav = perform(&mut e, 6.0, |b, e| {
            // intensity swells then eases — a shower passing over
            let u = (t(b) / 6.0) * std::f32::consts::PI;
            let intensity = 0.35 + 0.5 * u.sin();
            // Poisson-ish: expected drops per block = rate * blockdur
            let expected = 26.0 * intensity * (128.0 / SR); // sparse enough to hear EACH drop
            let n = expected.floor() as usize + ((rnd() < expected.fract()) as usize);
            for _ in 0..n {
                e.ev_drop(s, 0.25 + 0.55 * rnd());
            }
        });
        write_wav(&dir.join("2-rain.wav"), &wav).unwrap();
    }

    // ---- 3. bird — phrases with thought between them ----
    {
        let mut e = Engine::new(SR);
        let s = e.seat_add(false);
        let mut next_phrase = 0.4f32;
        let mut in_phrase = 0usize;
        let mut next_syll = 0.0f32;
        let wav = perform(&mut e, 7.0, |b, e| {
            let tt = t(b);
            if in_phrase == 0 && tt >= next_phrase {
                in_phrase = 3 + (rnd() * 4.0) as usize; // 3-6 syllables
                next_syll = tt;
            }
            if in_phrase > 0 && tt >= next_syll {
                let f1 = 2200.0 + rnd() * 2200.0;
                let f2 = f1 + (rnd() - 0.35) * 1800.0;
                e.ev_chirp(s, f1, f2, 0.05 + rnd() * 0.09);
                in_phrase -= 1;
                next_syll = tt + 0.09 + rnd() * 0.12;
                if in_phrase == 0 {
                    next_phrase = tt + 1.2 + rnd() * 1.8; // silence is part of the song
                }
            }
        });
        write_wav(&dir.join("3-bird.wav"), &wav).unwrap();
    }

    // ---- 4. bell — struck once, left to die ----
    {
        let mut e = Engine::new(SR);
        let s = e.seat_add(false);
        let mut struck = (false, false);
        let wav = perform(&mut e, 9.0, |b, e| {
            let tt = t(b);
            if !struck.0 && tt >= 0.5 {
                e.ev_strike(s, 98.0, 0.95);
                struck.0 = true;
            }
            if !struck.1 && tt >= 5.5 {
                e.ev_strike(s, 98.0, 0.45); // a second, softer — 夜半鐘聲
                struck.1 = true;
            }
        });
        write_wav(&dir.join("4-bell.wav"), &wav).unwrap();
    }

    // ---- 5. river — a population of bubbles over a cold bed ----
    {
        let mut e = Engine::new(SR);
        let s = e.seat_add(false);
        e.breath_set(s, 0.32); // the WATER itself — the bed is the river, bubbles are its punctuation
        let wav = perform(&mut e, 6.0, |_b, e| {
            let expected = 11.0 * (128.0 / SR);
            let n = expected.floor() as usize + ((rnd() < expected.fract()) as usize);
            for _ in 0..n {
                e.ev_bubble(s, 0.1 + 0.6 * rnd());
            }
        });
        write_wav(&dir.join("5-river.wav"), &wav).unwrap();
    }

    // ---- 6. wind — nothing but breath, gusting ----
    {
        let mut e = Engine::new(SR);
        let s = e.seat_add(false);
        let wav = perform(&mut e, 7.0, |b, e| {
            let tt = t(b);
            let gust = 0.30 + 0.45 * (tt * 0.7).sin().max(0.0) + 0.15 * (tt * 2.3).sin().abs();
            e.breath_set(s, gust.min(1.0));
        });
        write_wav(&dir.join("6-wind.wav"), &wav).unwrap();
    }

    // ---- 7. 寒江夜 — everything at once: the composed world ----
    // wind + sparse snow-drops + river + a far bell + the fisherman's 嗡阿吽.
    // In Phase B each line below is a seed cell at its own seat/position.
    {
        let mut e = Engine::new(SR);
        let wind = e.seat_add(false);
        let river = e.seat_add(true);
        e.seat_pos(river, 4.0, 0.0);
        let bell = e.seat_add(true);
        e.seat_pos(bell, 18.0, -6.0);
        let weng = e.seat_add(true);
        e.seat_pos(weng, 2.0, 1.0);
        e.listener(0.0, 0.0);
        e.breath_set(wind, 0.22);
        e.breath_set(river, 0.24);
        let mut bell_struck = false;
        let wav = perform(&mut e, 14.0, |b, e| {
            let tt = t(b);
            // river bubbles
            let expected = 8.0 * (128.0 / SR);
            let n = expected.floor() as usize + ((rnd() < expected.fract()) as usize);
            for _ in 0..n {
                e.ev_bubble(river, 0.1 + 0.5 * rnd());
            }
            // wind gusts slowly
            e.breath_set(wind, (0.18 + 0.14 * (tt * 0.5).sin()).max(0.06));
            // one far bell at t=2
            if !bell_struck && tt >= 2.0 {
                e.ev_strike(bell, 98.0, 0.9);
                bell_struck = true;
            }
            // the fisherman hums 嗡阿吽 from t=4 — ONE BREATH, the mouth
            // morphing o→a→u without the throat ever stopping (字字相連)
            let ct = tt - 4.0;
            let (f0, vow, nas) = if ct < 0.0 || ct > 8.3 {
                (0.0, 2.0, 0.9)
            } else {
                match ct {
                    x if x < 2.6 => (108.0, 0.0, 0.6 - 0.35 * (x / 2.6) + 0.55 * (x / 2.6) * (x / 2.6)),
                    x if x < 3.2 => (108.0 + 6.0 * ((x - 2.6) / 0.6), (x - 2.6) / 0.6, 0.1),
                    x if x < 5.4 => (114.0, 1.0, 0.08),
                    x if x < 6.0 => (114.0 - 10.0 * ((x - 5.4) / 0.6), 1.0 + (x - 5.4) / 0.6, 0.3),
                    x => (104.0, 2.0, (0.35 + 0.6 * ((x - 6.0) / 1.8)).min(0.95)), // 吽 short, pitch STEADY
                }
            };
            e.voice_set(weng, f0, vow, nas);
        });
        write_wav(&dir.join("7-cold-river-night.wav"), &wav).unwrap();
    }

    // ---- 8. 茉莉花 — a WOMAN sings the LYRICS, not a vocalise ----
    // Each syllable is a sung gesture: onset consonant (fric/burst/nasal) +
    // vowel glide + nasal coda; the MELODY replaces the tones (as in real
    // Mandarin song). 花/椏 carry melisma — one word over several notes.
    {
        struct G {
            closure: f32,
            fric: Option<(f32, f32, f32, f32)>, // (cf, start, len, lvl)
            nas0: f32,                          // nasal onset (m / n / l-ish)
            vow: &'static [(f32, f32)],         // glide over the first ~180ms
            coda: f32,                          // nasal coda level (n / ng)
        }
        const HAO: G = G { closure: 0.05, fric: Some((1400.0, 0.0, 0.08, 0.30)), nas0: 0.0, vow: &[(0.0, 1.0), (0.6, 1.0), (1.0, 0.0)], coda: 0.0 };
        const YI: G = G { closure: 0.0, fric: None, nas0: 0.0, vow: &[(0.0, 3.0)], coda: 0.0 };
        const DUO: G = G { closure: 0.05, fric: Some((3000.0, 0.04, 0.025, 0.45)), nas0: 0.0, vow: &[(0.0, 2.0), (0.4, 0.0), (1.0, 0.0)], coda: 0.0 };
        const MEI: G = G { closure: 0.0, fric: None, nas0: 0.85, vow: &[(0.0, 4.0), (0.6, 4.0), (1.0, 3.0)], coda: 0.0 };
        const LI: G = G { closure: 0.03, fric: None, nas0: 0.45, vow: &[(0.0, 3.0)], coda: 0.0 };
        const DE: G = G { closure: 0.05, fric: Some((3000.0, 0.04, 0.02, 0.4)), nas0: 0.0, vow: &[(0.0, 4.0)], coda: 0.0 };
        const MO: G = G { closure: 0.0, fric: None, nas0: 0.85, vow: &[(0.0, 0.0)], coda: 0.0 };
        const HUA: G = G { closure: 0.06, fric: Some((1500.0, 0.0, 0.09, 0.28)), nas0: 0.0, vow: &[(0.0, 2.0), (0.4, 1.0), (1.0, 1.0)], coda: 0.0 };
        const FEN: G = G { closure: 0.05, fric: Some((2200.0, 0.0, 0.08, 0.30)), nas0: 0.0, vow: &[(0.0, 4.0)], coda: 0.7 };
        const FANG: G = G { closure: 0.05, fric: Some((2200.0, 0.0, 0.08, 0.30)), nas0: 0.0, vow: &[(0.0, 1.0)], coda: 0.8 };
        const MAN: G = G { closure: 0.0, fric: None, nas0: 0.85, vow: &[(0.0, 1.0)], coda: 0.75 };
        const ZHI: G = G { closure: 0.06, fric: Some((2800.0, 0.0, 0.08, 0.5)), nas0: 0.0, vow: &[(0.0, 3.0)], coda: 0.0 };
        const YA: G = G { closure: 0.0, fric: None, nas0: 0.0, vow: &[(0.0, 3.0), (0.35, 1.0), (1.0, 1.0)], coda: 0.0 };
        // (gesture-or-melisma, scale degree, beats); degree 0 = breath
        type N = (Option<&'static G>, u8, f32);
        const PHRASE1: [N; 12] = [
            (Some(&HAO), 3, 0.6), (Some(&YI), 3, 0.4), (Some(&DUO), 5, 0.5), (Some(&MEI), 6, 0.5),
            (Some(&LI), 8, 0.9), (Some(&DE), 8, 0.45), (Some(&MO), 6, 0.45), (Some(&LI), 5, 0.5),
            (Some(&HUA), 5, 0.5), (None, 6, 0.4), (None, 5, 1.4), (None, 0, 0.9),
        ];
        const PHRASE3: [N; 8] = [
            (Some(&FEN), 5, 0.5), (Some(&FANG), 5, 0.5), (Some(&MEI), 5, 0.5), (Some(&LI), 3, 0.5),
            (Some(&MAN), 5, 0.5), (Some(&ZHI), 6, 0.9), (Some(&YA), 6, 0.4), (None, 5, 1.6),
        ];
        const F: [(u8, f32); 6] = [(1, 294.0), (2, 330.0), (3, 370.0), (5, 440.0), (6, 494.0), (8, 587.0)];
        let f_of = |d: u8| F.iter().find(|(k, _)| *k == d).map(|(_, f)| *f).unwrap_or(294.0);
        let beat = 0.62f32;
        // (start, dur, freq, gesture, is_last_of_word)
        let mut notes: Vec<(f32, f32, f32, Option<&G>)> = Vec::new();
        let mut cursor = 0.3f32;
        for list in [&PHRASE1[..], &PHRASE1[..], &PHRASE3[..]] {
            for &(g, d, b) in list {
                let dur = b * beat;
                if d == 0 {
                    cursor += dur; // breath between phrases
                } else {
                    notes.push((cursor, dur, f_of(d), g));
                    cursor += dur;
                }
            }
        }
        let total = cursor + 1.2;
        let mut e = Engine::new(SR);
        let s = e.seat_add(false);
        let wav = perform(&mut e, total, |b, e| {
            let tt = t(b);
            let mut set = (0.0f32, 1.0f32, 0.08f32);
            let mut fric = (1000.0f32, 0.0f32);
            let mut carry_vow = 1.0f32;
            for &(start, dur, freq, g) in &notes {
                if tt >= start + dur {
                    if let Some(g) = g {
                        carry_vow = g.vow[g.vow.len() - 1].1; // melisma carries the word's vowel
                    }
                    continue;
                }
                if tt < start {
                    break;
                }
                let ct = tt - start;
                match g {
                    Some(g) => {
                        if let Some((cf, fs, fl, lv)) = g.fric {
                            if ct >= fs && ct < fs + fl {
                                let u = (ct - fs) / fl;
                                fric = (cf, lv * (std::f32::consts::PI * u).sin());
                            }
                        }
                        if ct >= g.closure {
                            let vt = ((ct - g.closure) / 0.18).min(1.0); // the glide window
                            let mut vow = {
                                // piecewise track
                                let pts = g.vow;
                                let mut v = pts[pts.len() - 1].1;
                                if vt <= pts[0].0 { v = pts[0].1; }
                                for w in pts.windows(2) {
                                    if vt >= w[0].0 && vt < w[1].0 {
                                        let k = (vt - w[0].0) / (w[1].0 - w[0].0);
                                        v = w[0].1 + (w[1].1 - w[0].1) * k;
                                    }
                                }
                                v
                            };
                            let mut nas = 0.06 + g.nas0 * (-(ct - g.closure) / 0.06).exp();
                            if g.coda > 0.0 && ct > dur - 0.14 {
                                let k = (ct - (dur - 0.14)) / 0.14;
                                nas = nas.max(g.coda * k);
                                vow = vow.min(2.0); // the mouth closes toward the nose
                            }
                            set = (freq, vow, nas);
                        }
                    }
                    None => set = (freq, carry_vow, 0.08), // melisma: same word, new note
                }
                break;
            }
            e.voice_set(s, set.0, set.1, set.2);
            e.voice_fric(s, fric.0, fric.1);
        });
        write_wav(&dir.join("8-jasmine.wav"), &wav).unwrap();
    }

    // ---- 9. 老翁講話 — "孤舟蓑笠翁 獨釣寒江雪", spoken, an old man's voice ----
    // Klatt-style speech gestures: initials = closure + shaped hiss (burst /
    // fricative), finals = vowel glides + nasal codas, Mandarin tones = f0
    // contours. 老 = low f0 (~100Hz), slow syllables, a 4Hz tremor, extra air.
    {
        struct Syl {
            closure: f32,                       // silence before voicing (stop / fric onset)
            fric: Option<(f32, f32, f32, f32)>, // (cf Hz, start s, len s, level)
            vow: &'static [(f32, f32)],         // (voiced-fraction, vowel target 0..4)
            nas: &'static [(f32, f32)],         // (voiced-fraction, nasal 0..1)
            tone: (f32, f32, f32),              // f0 multiplier start/mid/end
            dur: f32,
            gap: f32,                           // articulatory pause after
        }
        fn track(pts: &[(f32, f32)], u: f32) -> f32 {
            if pts.is_empty() { return 0.0; }
            if u <= pts[0].0 { return pts[0].1; }
            for w in pts.windows(2) {
                if u >= w[0].0 && u < w[1].0 {
                    let k = (u - w[0].0) / (w[1].0 - w[0].0);
                    return w[0].1 + (w[1].1 - w[0].1) * k;
                }
            }
            pts[pts.len() - 1].1
        }
        // 孤舟蓑笠翁 獨釣寒江雪 — ten syllables, hand-scored
        let verse: [Syl; 10] = [
            Syl { closure: 0.06, fric: Some((1600.0, 0.045, 0.04, 0.50)), vow: &[(0.0, 2.0)], nas: &[(0.0, 0.05)], tone: (1.02, 1.02, 1.0), dur: 0.36, gap: 0.06 }, // 孤 gū
            Syl { closure: 0.05, fric: Some((2800.0, 0.04, 0.08, 0.55)), vow: &[(0.0, 0.0), (0.55, 0.0), (1.0, 2.0)], nas: &[(0.0, 0.05)], tone: (1.0, 1.0, 0.99), dur: 0.42, gap: 0.06 }, // 舟 zhōu
            Syl { closure: 0.10, fric: Some((6000.0, 0.0, 0.11, 0.50)), vow: &[(0.0, 2.0), (0.45, 2.0), (1.0, 0.0)], nas: &[(0.0, 0.05)], tone: (1.0, 1.0, 1.0), dur: 0.46, gap: 0.06 }, // 蓑 suō
            Syl { closure: 0.05, fric: None, vow: &[(0.0, 2.0), (0.3, 3.0), (1.0, 3.0)], nas: &[(0.0, 0.5), (0.2, 0.1), (1.0, 0.05)], tone: (1.10, 0.95, 0.80), dur: 0.36, gap: 0.06 }, // 笠 lì
            Syl { closure: 0.0, fric: None, vow: &[(0.0, 2.0), (0.4, 4.0), (1.0, 0.0)], nas: &[(0.0, 0.1), (0.45, 0.15), (1.0, 0.92)], tone: (1.0, 1.0, 0.97), dur: 0.52, gap: 0.5 }, // 翁 wēng — caesura
            Syl { closure: 0.05, fric: Some((3000.0, 0.04, 0.03, 0.50)), vow: &[(0.0, 2.0)], nas: &[(0.0, 0.05)], tone: (0.92, 0.96, 1.06), dur: 0.36, gap: 0.06 }, // 獨 dú
            Syl { closure: 0.05, fric: Some((3200.0, 0.04, 0.03, 0.50)), vow: &[(0.0, 3.0), (0.45, 1.0), (1.0, 0.0)], nas: &[(0.0, 0.05)], tone: (1.10, 0.94, 0.78), dur: 0.42, gap: 0.06 }, // 釣 diào
            Syl { closure: 0.05, fric: Some((1400.0, 0.0, 0.07, 0.32)), vow: &[(0.0, 1.0)], nas: &[(0.0, 0.05), (0.6, 0.1), (1.0, 0.8)], tone: (0.92, 0.95, 1.05), dur: 0.44, gap: 0.06 }, // 寒 hán
            Syl { closure: 0.06, fric: Some((3700.0, 0.0, 0.08, 0.50)), vow: &[(0.0, 3.0), (0.5, 1.0), (1.0, 1.0)], nas: &[(0.0, 0.08), (0.55, 0.15), (1.0, 0.9)], tone: (1.0, 1.0, 0.98), dur: 0.46, gap: 0.06 }, // 江 jiāng
            Syl { closure: 0.08, fric: Some((4300.0, 0.0, 0.09, 0.50)), vow: &[(0.0, 3.0), (0.5, 4.0), (1.0, 4.0)], nas: &[(0.0, 0.05)], tone: (0.88, 0.80, 0.96), dur: 0.52, gap: 0.3 }, // 雪 xuě
        ];
        let mut starts = Vec::new();
        let mut cursor = 0.4f32;
        for sy in &verse {
            starts.push(cursor);
            cursor += sy.dur + sy.gap;
        }
        let total = cursor + 0.8;
        let base_f0 = 100.0f32; // an old man, low in the chest
        let mut e = Engine::new(SR);
        let s = e.seat_add(false);
        let wav = perform(&mut e, total, |b, e| {
            let tt = t(b);
            // the old tremor: slow, slightly irregular
            let tremor = 1.0 + 0.009 * (std::f32::consts::TAU * 3.8 * tt).sin(); // old but steady
            let mut set = (0.0f32, 1.0f32, 0.05f32);
            let mut fric = (1000.0f32, 0.0f32);
            for (i, sy) in verse.iter().enumerate() {
                let ct = tt - starts[i];
                if ct < 0.0 || ct >= sy.dur + sy.gap {
                    continue;
                }
                if let Some((cf, fs, fl, lv)) = sy.fric {
                    if ct >= fs && ct < fs + fl {
                        // Hann window over the hiss so it breathes, not clicks
                        let u = (ct - fs) / fl;
                        fric = (cf, lv * (std::f32::consts::PI * u).sin());
                    }
                }
                if ct >= sy.closure && ct < sy.dur {
                    let vf = (ct - sy.closure) / (sy.dur - sy.closure);
                    let (m0, m1, m2) = sy.tone;
                    let tone = if vf < 0.5 {
                        m0 + (m1 - m0) * (vf * 2.0)
                    } else {
                        m1 + (m2 - m1) * ((vf - 0.5) * 2.0)
                    };
                    set = (base_f0 * tone * tremor, track(sy.vow, vf), track(sy.nas, vf));
                }
                break;
            }
            e.voice_set(s, set.0, set.1, set.2);
            e.voice_fric(s, fric.0, fric.1);
        });
        write_wav(&dir.join("9-poem-spoken.wav"), &wav).unwrap();
    }

    println!("聲塵 rendered → {}", dir.display());
    for f in [
        "1-om-ah-hum.wav",
        "2-rain.wav",
        "3-bird.wav",
        "4-bell.wav",
        "5-river.wav",
        "6-wind.wav",
        "7-cold-river-night.wav",
        "8-jasmine.wav",
        "9-poem-spoken.wav",
    ] {
        println!("  afplay {}", dir.join(f).display());
    }
}
