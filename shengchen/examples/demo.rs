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
            (7.0, 106.0, 2.0, 0.30), // MORPH into 吽 — lips rounding
            (9.0, 102.0, 2.0, 0.85), // 吽 sinks into the deep hum
            (10.6, 100.0, 2.0, 0.95), // ...
            (11.4, 0.0, 2.0, 0.95),  // the breath ends (only here)
        ];
        let wav = perform(&mut e, 12.0, |b, e| {
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
            let (f0, vow, nas) = if ct < 0.0 || ct > 9.6 {
                (0.0, 2.0, 0.9)
            } else {
                match ct {
                    x if x < 2.6 => (108.0, 0.0, 0.6 - 0.35 * (x / 2.6) + 0.55 * (x / 2.6) * (x / 2.6)),
                    x if x < 3.2 => (108.0 + 6.0 * ((x - 2.6) / 0.6), (x - 2.6) / 0.6, 0.1),
                    x if x < 5.8 => (114.0, 1.0, 0.08),
                    x if x < 6.4 => (114.0 - 8.0 * ((x - 5.8) / 0.6), 1.0 + (x - 5.8) / 0.6, 0.3),
                    x => (106.0 - 5.0 * ((x - 6.4) / 3.2), 2.0, 0.3 + 0.6 * ((x - 6.4) / 3.2)),
                }
            };
            e.voice_set(weng, f0, vow, nas);
        });
        write_wav(&dir.join("7-cold-river-night.wav"), &wav).unwrap();
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
    ] {
        println!("  afplay {}", dir.join(f).display());
    }
}
