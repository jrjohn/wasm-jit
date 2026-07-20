// §24 the ear's skin — 寒江風聲: cold wind over the river, a temple bell far off.
// The seed runs once per audio sample (44.1kHz). Pure math IS the synth:
// FM for the wind's breath, a decaying sine for the bell, struck every 6s.

// wind: two detuned FM voices, amplitude breathing slowly
let breath = 0.5 + 0.5 * sin(t * 0.23);
let w1 = sin(6.2832 * 96.0 * t + sin(6.2832 * 41.0 * t) * 4.0);
let w2 = sin(6.2832 * 143.0 * t + sin(6.2832 * 57.0 * t) * 3.0);
let wind = (w1 * 0.6 + w2 * 0.4) * (0.08 + 0.10 * breath);

// a distant bell, struck every 6 seconds, decaying
let ph = t % 6.0;
let env = max(1.0 - ph * 0.7, 0.0);
let ring = env * env;
let bell = (sin(6.2832 * 220.0 * ph) * 0.6
          + sin(6.2832 * 329.6 * ph) * 0.3
          + sin(6.2832 * 440.0 * ph) * 0.2) * ring * 0.22;

// a low river drone underneath
let river = sin(6.2832 * 55.0 * t) * 0.05 * (0.6 + 0.4 * sin(t * 0.11));

wind + bell + river
