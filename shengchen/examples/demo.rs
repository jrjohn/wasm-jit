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

    // ---- 1. 嗡阿吽 — the mantra: three mouths of one breath ----
    // 嗡 o+nasal → 阿 open → 吽 u+deep nasal; ~3s each, a breath between
    {
        let mut e = Engine::new(SR);
        let s = e.seat_add(false);
        let wav = perform(&mut e, 11.0, |b, e| {
            let tt = t(b);
            let (f0, vow, nas) = match tt {
                x if x < 3.0 => {
                    // 嗡: opens as o, closes into the nose
                    let u = (x / 3.0).min(1.0);
                    (110.0, 0.0, 0.15 + 0.75 * u)
                }
                x if x < 3.4 => (0.0, 0.0, 0.0), // breath
                x if x < 6.6 => (116.0, 1.0, 0.05), // 阿: open mouth
                x if x < 7.0 => (0.0, 1.0, 0.0),   // breath
                x if x < 10.4 => {
                    // 吽: u closing into a deep hum, pitch settling down
                    let u = ((x - 7.0) / 3.4).min(1.0);
                    (104.0 - 6.0 * u, 2.0, 0.2 + 0.75 * u)
                }
                _ => (0.0, 2.0, 0.0),
            };
            e.voice_set(s, f0, vow, nas);
        });
        write_wav(&dir.join("1-om-ah-hum.wav"), &wav).unwrap();
    }

    // ---- 2. rain — every drop a separate decision ----
    {
        let mut e = Engine::new(SR);
        let s = e.seat_add(false);
        e.breath_set(s, 0.10); // the distant hiss of a wet sky
        let wav = perform(&mut e, 6.0, |b, e| {
            // intensity swells then eases — a shower passing over
            let u = (t(b) / 6.0) * std::f32::consts::PI;
            let intensity = 0.35 + 0.5 * u.sin();
            // Poisson-ish: expected drops per block = rate * blockdur
            let expected = 90.0 * intensity * (128.0 / SR);
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
        e.breath_set(s, 0.18);
        let wav = perform(&mut e, 6.0, |_b, e| {
            let expected = 26.0 * (128.0 / SR);
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
        e.breath_set(river, 0.10);
        let mut bell_struck = false;
        let wav = perform(&mut e, 14.0, |b, e| {
            let tt = t(b);
            // river bubbles
            let expected = 18.0 * (128.0 / SR);
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
            // the fisherman hums 嗡阿吽 from t=4, one full cycle
            let ct = tt - 4.0;
            let (f0, vow, nas) = if ct < 0.0 {
                (0.0, 0.0, 0.0)
            } else {
                match ct {
                    x if x < 3.0 => (110.0, 0.0, 0.2 + 0.7 * (x / 3.0)),
                    x if x < 3.4 => (0.0, 0.0, 0.0),
                    x if x < 6.4 => (116.0, 1.0, 0.05),
                    x if x < 6.8 => (0.0, 1.0, 0.0),
                    x if x < 9.8 => (104.0, 2.0, 0.25 + 0.65 * ((x - 6.8) / 3.0)),
                    _ => (0.0, 2.0, 0.0),
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
