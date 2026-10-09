// Petits sons synthétisés (aucun fichier audio embarqué).
let ctx: AudioContext | null = null;

function tone(freq: number, at: number, dur: number, type: OscillatorType = "sine", gain = 0.07) {
  ctx ??= new AudioContext();
  const t = ctx.currentTime + at;
  const osc = ctx.createOscillator();
  const g = ctx.createGain();
  osc.type = type;
  osc.frequency.setValueAtTime(freq, t);
  g.gain.setValueAtTime(0.0001, t);
  g.gain.exponentialRampToValueAtTime(gain, t + 0.015);
  g.gain.exponentialRampToValueAtTime(0.0001, t + dur);
  osc.connect(g).connect(ctx.destination);
  osc.start(t);
  osc.stop(t + dur + 0.05);
}

export const sfx = {
  pop() {
    tone(620, 0, 0.09, "triangle");
    tone(930, 0.05, 0.1, "triangle");
  },
  done() {
    [523, 659, 784, 1046].forEach((f, i) => tone(f, i * 0.07, 0.18, "sine", 0.06));
  },
  alert() {
    tone(880, 0, 0.12, "triangle", 0.08);
    tone(1175, 0.14, 0.16, "triangle", 0.08);
  },
  error() {
    tone(392, 0, 0.18, "sine", 0.07);
    tone(294, 0.16, 0.26, "sine", 0.07);
  },
};
