import { useEffect, useState, type ReactNode } from "react";

export type Mood = "idle" | "think" | "work" | "alert" | "happy" | "error" | "sleep" | "curious";

interface Props {
  mood: Mood;
  /** Direction du regard, composantes entre -1 et 1. */
  look?: { x: number; y: number };
  size?: number;
}

const MOUTHS: Record<Mood, ReactNode> = {
  idle: <path d="M44 67 Q50 73 56 67" className="lody-stroke" />,
  curious: <ellipse cx="50" cy="69" rx="3" ry="3.4" className="lody-fill-dark" />,
  think: <path d="M45 69 Q48 67 50 69 T55 69" className="lody-stroke" />,
  work: <path d="M45 68 Q50 71 55 68" className="lody-stroke" />,
  alert: <ellipse cx="50" cy="69" rx="3.6" ry="4.4" className="lody-fill-dark" />,
  happy: (
    <g>
      <path d="M42 65 Q50 77 58 65 Z" className="lody-fill-dark" />
      <path d="M46 71 Q50 75 54 71 Q50 69 46 71 Z" fill="#fb7185" />
    </g>
  ),
  error: <path d="M44 71 Q50 65 56 71" className="lody-stroke" />,
  sleep: <path d="M46 69 L54 69" className="lody-stroke" />,
};

function Eye({ cx, mood, blink, lx, ly }: { cx: number; mood: Mood; blink: boolean; lx: number; ly: number }) {
  if (mood === "happy") return <path d={`M${cx - 6} 57 Q${cx} 49 ${cx + 6} 57`} className="lody-stroke thick" />;
  if (mood === "sleep") return <path d={`M${cx - 6} 56 Q${cx} 60 ${cx + 6} 56`} className="lody-stroke thick" />;
  if (mood === "error")
    return (
      <g transform={`translate(${lx * 0.6} ${ly * 0.6})`}>
        <ellipse cx={cx} cy={56} rx={5.6} ry={6.8} className="lody-fill-dark" />
        <path d={`M${cx - 7} ${cx < 50 ? 46 : 48} L${cx + 7} ${cx < 50 ? 48 : 46}`} className="lody-stroke" />
      </g>
    );
  const big = mood === "alert" || mood === "curious";
  return (
    <g className="lody-eye" style={{ transform: `translate(${lx}px, ${ly}px) scaleY(${blink ? 0.1 : 1})`, transformOrigin: `${cx}px 55px` }}>
      <ellipse cx={cx} cy={55} rx={big ? 7.2 : 6.5} ry={big ? 9.2 : 8.5} className="lody-fill-dark" />
      <circle cx={cx - 2.2} cy={51.5} r={big ? 2.8 : 2.4} fill="#fff" />
      <circle cx={cx + 2.4} cy={58.5} r={1} fill="#fff" opacity=".7" />
    </g>
  );
}

export default function Lody({ mood, look = { x: 0, y: 0 }, size = 64 }: Props) {
  const [blink, setBlink] = useState(false);

  useEffect(() => {
    let t: number;
    const loop = () => {
      t = window.setTimeout(() => {
        setBlink(true);
        window.setTimeout(() => setBlink(false), 130);
        loop();
      }, 2200 + Math.random() * 4200);
    };
    loop();
    return () => clearTimeout(t);
  }, []);

  const gaze = mood === "think" ? { x: 0.7, y: -0.8 } : look;
  const lx = gaze.x * 3.2;
  const ly = gaze.y * 2.6;

  return (
    <div className={`lody lody-${mood}`} style={{ width: size, height: size }}>
      <svg viewBox="0 0 100 100" width={size} height={size} overflow="visible">
        <defs>
          <linearGradient id="lody-body" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" stopColor="var(--lody-top)" />
            <stop offset="1" stopColor="var(--lody-bottom)" />
          </linearGradient>
        </defs>
        <ellipse cx="50" cy="98" rx="30" ry="4" className="lody-shadow" />
        <g className="lody-body">
          <g className="lody-leaf">
            <path d="M51 17 C51 10 53 6 57 4" stroke="#4ade80" strokeWidth="3" fill="none" strokeLinecap="round" />
            <path d="M56 5 C62 -1 74 0 77 4 C70 10 61 11 56 5 Z" fill="#86efac" />
          </g>
          <path
            d="M50 16 C80 16 93 29 93 56 C93 84 77 95 50 95 C23 95 7 84 7 56 C7 29 20 16 50 16 Z"
            fill="url(#lody-body)"
          />
          <ellipse cx="34" cy="34" rx="13" ry="7" fill="#fff" opacity=".28" transform="rotate(-22 34 34)" />
          <Eye cx={36} mood={mood} blink={blink} lx={lx} ly={ly} />
          <Eye cx={64} mood={mood} blink={blink} lx={lx} ly={ly} />
          <ellipse cx="25" cy="68" rx="6.5" ry="3.8" className="lody-cheek" />
          <ellipse cx="75" cy="68" rx="6.5" ry="3.8" className="lody-cheek" />
          <g transform={`translate(${lx * 0.4} ${ly * 0.3})`}>{MOUTHS[mood]}</g>
        </g>
        {mood === "think" && (
          <g className="lody-dots">
            <circle cx="84" cy="14" r="3.2" />
            <circle cx="93" cy="6" r="4" />
            <circle cx="104" cy="-4" r="5" />
          </g>
        )}
        {mood === "alert" && (
          <g className="lody-bang">
            <circle cx="88" cy="16" r="11" fill="var(--warn)" />
            <path d="M88 9 L88 18" stroke="#3b2500" strokeWidth="3.4" strokeLinecap="round" />
            <circle cx="88" cy="23" r="1.9" fill="#3b2500" />
          </g>
        )}
        {mood === "sleep" && (
          <g className="lody-zzz">
            <text x="80" y="20">z</text>
            <text x="90" y="8">z</text>
          </g>
        )}
        {mood === "happy" && (
          <g className="lody-sparkles">
            <path d="M12 18 l2 5 5 2 -5 2 -2 5 -2 -5 -5 -2 5 -2z" />
            <path d="M88 26 l1.6 4 4 1.6 -4 1.6 -1.6 4 -1.6 -4 -4 -1.6 4 -1.6z" />
          </g>
        )}
        {mood === "error" && <path className="lody-drop" d="M86 34 C86 34 80 42 80 46 a6 6 0 0 0 12 0 C92 42 86 34 86 34 Z" />}
      </svg>
    </div>
  );
}
