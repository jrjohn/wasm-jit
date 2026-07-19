//! 注音 → 發音手勢編譯器 — the methodology, distilled.
//!
//! A Mandarin syllable is a CLOSED set: 聲母(21) + 介音(3) + 韻母(13) + 聲調(5).
//! Everything the user's ear-transcriptions taught us is encoded ONCE, per
//! symbol, in these tables — locus glides for stops, closure→burst→hiss for
//! affricates, the schwa nucleus of ㄥ, ㄩ as the u/i midpoint, nasal codas —
//! and then ANY Chinese text speaks, with no per-word hand-scoring.
//!
//! Vowel line (the throat's 1-D mouth): 0=o 1=a 2=u 3=i 4=e(front).
//! 0.5 ≈ schwa (o/a midpoint), 2.5 ≈ ü (u/i midpoint) — learned from 翁 and 雪.

/// One sung/spoken syllable, ready for a conductor:
/// closure → (burst/frication window) → voiced vowel track + nasal track,
/// f0 shaped by the tone contour.
#[derive(Clone, Debug)]
pub struct Gesture {
    pub closure: f32,
    /// (centre Hz, start s, len s, level)
    pub fric: Option<(f32, f32, f32, f32)>,
    /// (voiced-fraction, vowel 0..4)
    pub vow: Vec<(f32, f32)>,
    /// (voiced-fraction, nasal 0..1)
    pub nas: Vec<(f32, f32)>,
    /// f0 multipliers (start, mid, end)
    pub tone: (f32, f32, f32),
    pub dur: f32,
    pub gap: f32,
}

/// 聲母 recipe. `locus`: where the vowel's first ~50ms glides IN from (the
/// formant-transition cue that makes a stop a stop). `back_cf`: velars burst
/// LOW before back vowels (ㄍㄨ) and high before front ones (ㄍㄧ is rare; ㄐ
/// covers that ground in Mandarin).
struct Initial {
    closure: f32,
    burst: Option<(f32, f32, f32)>, // (cf, len, lvl) at release (start = closure - 0.03)
    hiss: Option<(f32, f32, f32)>,  // (cf, len, lvl) from t=0 (fricatives) or after burst (affricates)
    nas0: f32,
    locus: Option<f32>,
    back_cf: Option<f32>, // burst cf override before u/o nuclei
}

const NO_I: Initial = Initial { closure: 0.0, burst: None, hiss: None, nas0: 0.0, locus: None, back_cf: None };

fn initial(c: char) -> Option<Initial> {
    Some(match c {
        'ㄅ' => Initial { closure: 0.09, burst: Some((900.0, 0.02, 0.70)), locus: Some(0.0), ..NO_I },
        'ㄆ' => Initial { closure: 0.09, burst: Some((900.0, 0.02, 0.70)), hiss: Some((1500.0, 0.06, 0.30)), locus: Some(0.0), ..NO_I },
        'ㄇ' => Initial { nas0: 0.85, ..NO_I },
        'ㄈ' => Initial { closure: 0.06, hiss: Some((2200.0, 0.09, 0.30)), ..NO_I },
        'ㄉ' => Initial { closure: 0.09, burst: Some((3900.0, 0.025, 0.80)), locus: Some(3.5), ..NO_I },
        'ㄊ' => Initial { closure: 0.09, burst: Some((3900.0, 0.025, 0.80)), hiss: Some((3400.0, 0.06, 0.30)), locus: Some(3.5), ..NO_I },
        'ㄋ' => Initial { nas0: 0.80, locus: Some(3.5), ..NO_I },
        'ㄌ' => Initial { closure: 0.03, nas0: 0.45, locus: Some(3.0), ..NO_I },
        'ㄍ' => Initial { closure: 0.10, burst: Some((2500.0, 0.03, 0.80)), locus: Some(3.0), back_cf: Some(1100.0), ..NO_I },
        'ㄎ' => Initial { closure: 0.10, burst: Some((2500.0, 0.03, 0.80)), hiss: Some((1600.0, 0.06, 0.30)), locus: Some(3.0), back_cf: Some(1100.0), ..NO_I },
        'ㄏ' => Initial { closure: 0.05, hiss: Some((1400.0, 0.08, 0.32)), ..NO_I },
        'ㄐ' => Initial { closure: 0.11, hiss: Some((4200.0, 0.07, 0.70)), locus: Some(3.0), ..NO_I },
        'ㄑ' => Initial { closure: 0.11, hiss: Some((4200.0, 0.11, 0.65)), locus: Some(3.0), ..NO_I },
        'ㄒ' => Initial { closure: 0.08, hiss: Some((4300.0, 0.10, 0.50)), locus: Some(3.0), ..NO_I },
        'ㄓ' => Initial { closure: 0.13, hiss: Some((3100.0, 0.10, 0.80)), locus: Some(3.3), ..NO_I },
        'ㄔ' => Initial { closure: 0.16, hiss: Some((3100.0, 0.14, 0.75)), locus: Some(3.3), ..NO_I },
        'ㄕ' => Initial { closure: 0.14, hiss: Some((3100.0, 0.12, 0.70)), locus: Some(3.3), ..NO_I },
        'ㄖ' => Initial { closure: 0.02, hiss: Some((2800.0, 0.06, 0.22)), locus: Some(3.3), ..NO_I },
        'ㄗ' => Initial { closure: 0.09, hiss: Some((5500.0, 0.08, 0.60)), locus: Some(3.5), ..NO_I },
        'ㄘ' => Initial { closure: 0.09, hiss: Some((5500.0, 0.12, 0.58)), locus: Some(3.5), ..NO_I },
        'ㄙ' => Initial { closure: 0.11, hiss: Some((6000.0, 0.13, 0.62)), locus: Some(3.5), ..NO_I },
        _ => return None,
    })
}

/// 韻母 recipe: the vowel journey (on the 1-D mouth line) + the nasal coda.
/// Fractions are of the VOICED span; the compiler prepends medials and the
/// initial's locus glide in front of these.
struct Final {
    vow: &'static [(f32, f32)],
    coda: f32, // nasal coda level (ㄢㄣ n / ㄤㄥ ng)
    long: f32, // duration bonus (diphthongs & nasals take longer)
}

fn final_(c: char) -> Option<Final> {
    Some(match c {
        'ㄚ' => Final { vow: &[(0.0, 1.0)], coda: 0.0, long: 0.0 },
        'ㄛ' => Final { vow: &[(0.0, 0.0)], coda: 0.0, long: 0.0 },
        'ㄜ' => Final { vow: &[(0.0, 0.5)], coda: 0.0, long: 0.0 },   // schwa
        'ㄝ' => Final { vow: &[(0.0, 4.0)], coda: 0.0, long: 0.0 },
        'ㄞ' => Final { vow: &[(0.0, 1.0), (0.5, 1.0), (1.0, 3.0)], coda: 0.0, long: 0.06 },
        'ㄟ' => Final { vow: &[(0.0, 4.0), (0.5, 4.0), (1.0, 3.0)], coda: 0.0, long: 0.04 },
        'ㄠ' => Final { vow: &[(0.0, 1.0), (0.5, 1.0), (1.0, 0.0)], coda: 0.0, long: 0.06 },
        'ㄡ' => Final { vow: &[(0.0, 0.0), (0.8, 0.0), (1.0, 2.0)], coda: 0.0, long: 0.04 },
        'ㄢ' => Final { vow: &[(0.0, 1.0)], coda: 0.80, long: 0.06 },
        'ㄣ' => Final { vow: &[(0.0, 0.5)], coda: 0.75, long: 0.04 },
        'ㄤ' => Final { vow: &[(0.0, 1.0)], coda: 0.88, long: 0.08 },
        'ㄥ' => Final { vow: &[(0.0, 0.42)], coda: 0.90, long: 0.08 }, // schwa nucleus (learned from 翁)
        'ㄦ' => Final { vow: &[(0.0, 0.6)], coda: 0.0, long: 0.04 },
        _ => return None,
    })
}

/// 介音: the on-glide, as a vowel-line point (ㄩ = the u/i midpoint, from 雪).
fn medial(c: char) -> Option<f32> {
    match c {
        'ㄧ' => Some(3.0),
        'ㄨ' => Some(2.0),
        'ㄩ' => Some(2.5),
        _ => None,
    }
}

/// 聲調 contours (f0 multipliers), recitation-gentle (a caricature contour is
/// the robot voice — learned the hard way).
fn tone_contour(t: u8) -> (f32, f32, f32) {
    match t {
        1 => (1.06, 1.06, 1.04),
        2 => (0.92, 0.96, 1.08),
        3 => (0.88, 0.80, 0.96),
        4 => (1.10, 0.94, 0.78),
        _ => (0.96, 0.92, 0.88), // 輕聲
    }
}

/// Compile a zhuyin string into gestures.
/// Syllables separated by spaces; `,` `，` `、` insert a caesura; tone marks
/// ˊ ˇ ˋ ˙ follow the syllable (unmarked = tone 1).
pub fn compile(text: &str) -> Vec<Gesture> {
    let mut out = Vec::new();
    for word in text.split_whitespace() {
        if matches!(word, "," | "，" | "、" | "。") {
            if let Some(last) = out.last_mut() {
                let g: &mut Gesture = last;
                g.gap = g.gap.max(0.5); // the pause lives after the previous word
            }
            continue;
        }
        let mut init: Option<Initial> = None;
        let mut med: Option<f32> = None;
        let mut fin: Option<Final> = None;
        let mut tone = 1u8;
        for c in word.chars() {
            match c {
                'ˊ' => tone = 2,
                'ˇ' => tone = 3,
                'ˋ' => tone = 4,
                '˙' => tone = 5,
                'ˉ' => tone = 1,
                _ => {
                    if init.is_none() && med.is_none() && fin.is_none() {
                        if let Some(i) = initial(c) {
                            init = Some(i);
                            continue;
                        }
                    }
                    if fin.is_none() {
                        if let Some(m) = medial(c) {
                            if med.is_none() {
                                med = Some(m);
                                continue;
                            }
                        }
                    }
                    if let Some(f) = final_(c) {
                        fin = Some(f);
                    }
                }
            }
        }
        // a bare medial is its own nucleus: ㄧ=i ㄨ=u ㄩ=ü
        let fin = fin.unwrap_or_else(|| match med {
            Some(m) if (m - 3.0).abs() < 0.1 => Final { vow: &[(0.0, 3.0)], coda: 0.0, long: 0.0 },
            Some(m) if (m - 2.5).abs() < 0.1 => Final { vow: &[(0.0, 2.5)], coda: 0.0, long: 0.0 },
            _ => Final { vow: &[(0.0, 2.0)], coda: 0.0, long: 0.0 },
        });
        let is_bare_medial_nucleus = fin.vow.len() == 1 && med.map_or(false, |m| (m - fin.vow[0].1).abs() < 0.6);
        let init = init.unwrap_or(NO_I);

        // ---- assemble the vowel journey: locus glide → medial → final ----
        let mut vow: Vec<(f32, f32)> = Vec::new();
        let mut cursor = 0.0f32;
        // the locus cue only helps when the road is SHORT: our mouth is a 1-D
        // line (o a u i e), so a far glide detours through foreign vowels —
        // ㄓㄡ's 3.3→0 passed the a-region and 舟 was heard as 阿
        let first_target = med
            .filter(|_| !is_bare_medial_nucleus)
            .unwrap_or(fin.vow[0].1);
        if let Some(l) = init.locus {
            if (l - first_target).abs() <= 1.6 {
                vow.push((0.0, l));
                cursor = 0.14; // the transition cue: ~50ms of a 0.4s syllable
            }
        }
        if let (Some(m), false) = (med, is_bare_medial_nucleus) {
            vow.push((cursor, m));
            cursor += 0.14;
        }
        for &(f, v) in fin.vow {
            vow.push((cursor + f * (1.0 - cursor), v));
        }
        if vow.is_empty() {
            vow.push((0.0, 1.0));
        }

        // ---- nasal track: onset (ㄇㄋㄌ colour) + coda (ㄢㄣㄤㄥ) ----
        let mut nas: Vec<(f32, f32)> = Vec::new();
        if init.nas0 > 0.0 {
            nas.push((0.0, init.nas0));
            nas.push((0.22, 0.04));
        } else {
            nas.push((0.0, 0.02));
        }
        if fin.coda > 0.0 {
            nas.push((0.62, 0.06));
            nas.push((1.0, fin.coda));
        }

        // ---- the unvoiced window: burst and/or hiss ----
        // fricatives hiss from t=0 (closure covers them); affricate/stop bursts
        // fire just before release; aspiration follows the burst
        let backish = vow
            .iter()
            .find(|(t, _)| *t >= init.locus.map_or(0.0, |_| 0.14))
            .map_or(false, |(_, v)| *v < 2.3 && *v != 1.0 || (*v - 2.0).abs() < 0.3);
        let fric = match (init.burst, init.hiss) {
            (Some((cf, len, lvl)), None) => {
                let cf = if backish { init.back_cf.unwrap_or(cf) } else { cf };
                Some((cf, (init.closure - 0.03).max(0.0), len + 0.01, lvl))
            }
            (Some((bcf, blen, blvl)), Some((_hcf, hlen, hlvl))) => {
                // stop + aspiration: one window, burst-loud fading to breath
                let cf = if backish { init.back_cf.unwrap_or(bcf) } else { bcf };
                Some((cf, (init.closure - 0.03).max(0.0), blen + hlen, blvl.max(hlvl)))
            }
            (None, Some((cf, len, lvl))) => Some((cf, 0.0, len, lvl)),
            (None, None) => None,
        };

        let dur = 0.40 + fin.long + if init.closure > 0.08 { 0.02 } else { 0.0 };
        out.push(Gesture {
            closure: init.closure,
            fric,
            vow,
            nas,
            tone: tone_contour(tone),
            dur,
            gap: 0.06,
        });
    }
    // phrase-final lengthening — the last word of an utterance breathes
    if let Some(last) = out.last_mut() {
        last.dur += 0.10;
        last.gap = last.gap.max(0.3);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poem_compiles_to_ten_gestures() {
        let g = compile("ㄍㄨ ㄓㄡ ㄙㄨㄛ ㄌㄧˋ ㄨㄥ ， ㄉㄨˊ ㄉㄧㄠˋ ㄏㄢˊ ㄐㄧㄤ ㄒㄩㄝˇ");
        assert_eq!(g.len(), 10, "十個字十個手勢");
        // 孤: velar burst goes LOW before u (the back_cf rule)
        let gu = &g[0];
        assert!(gu.fric.map_or(false, |(cf, _, _, _)| cf < 1500.0), "ㄍㄨ burst not low: {:?}", gu.fric);
        // 翁 caesura carried by the previous word's gap
        assert!(g[4].gap >= 0.5, "the , after 翁 must breathe: {}", g[4].gap);
        // 雪 starts at the ü midpoint
        let xue = &g[9];
        assert!(xue.vow.iter().any(|(_, v)| (*v - 2.5).abs() < 0.11), "ㄒㄩㄝ lost its ü: {:?}", xue.vow);
        // 江 has an ng coda
        assert!(g[8].nas.last().unwrap().1 > 0.8, "ㄐㄧㄤ lost its ng");
    }

    #[test]
    fn far_locus_is_skipped_near_locus_kept() {
        // 舟 ㄓㄡ: retroflex locus 3.3 vs o(0) — a 1-D detour through a → SKIP
        let zhou = &compile("ㄓㄡ")[0];
        assert!(zhou.vow[0].1 < 1.0, "ㄓㄡ must start at o, not the far locus: {:?}", zhou.vow);
        // 獨 ㄉㄨ: alveolar locus 3.5 vs u(2) — short road, the stop cue stays
        let du = &compile("ㄉㄨˊ")[0];
        assert!(du.vow[0].1 > 3.0, "ㄉㄨ must keep its locus glide: {:?}", du.vow);
    }

    #[test]
    fn every_symbol_compiles() {
        for c in "ㄅㄆㄇㄈㄉㄊㄋㄌㄍㄎㄏㄐㄑㄒㄓㄔㄕㄖㄗㄘㄙ".chars() {
            assert!(initial(c).is_some(), "initial {c} missing");
        }
        for c in "ㄚㄛㄜㄝㄞㄟㄠㄡㄢㄣㄤㄥㄦ".chars() {
            assert!(final_(c).is_some(), "final {c} missing");
        }
        for c in "ㄧㄨㄩ".chars() {
            assert!(medial(c).is_some(), "medial {c} missing");
        }
        // and a sweep of whole syllables must produce sane gestures
        let g = compile("ㄅㄚ ㄆㄧ ㄇㄛ ㄈㄤˊ ㄊㄡˋ ㄋㄩˇ ㄎㄜ ㄑㄩ ㄕㄨㄟˇ ㄖㄣˊ ㄘㄞˋ ㄦˊ");
        assert_eq!(g.len(), 12);
        for ge in &g {
            assert!(ge.dur > 0.3 && ge.dur < 0.8);
            assert!(!ge.vow.is_empty());
        }
    }
}
