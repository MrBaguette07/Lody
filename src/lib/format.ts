export function ago(ms?: number | null) {
  if (!ms) return "";
  const s = Math.max(0, Math.round((Date.now() - ms) / 1000));
  if (s < 45) return "à l'instant";
  const m = Math.round(s / 60);
  if (m < 60) return `il y a ${m} min`;
  const h = Math.round(m / 60);
  if (h < 24) return `il y a ${h} h`;
  const d = Math.round(h / 24);
  if (d < 31) return `il y a ${d} j`;
  return new Date(ms).toLocaleDateString("fr-FR", { day: "numeric", month: "short", year: "numeric" });
}

export function duration(ms?: number | null) {
  if (!ms) return "";
  const s = Math.round(ms / 1000);
  if (s < 60) return `${s} s`;
  return `${Math.floor(s / 60)} min ${String(s % 60).padStart(2, "0")}`;
}

export const cost = (usd?: number | null) =>
  usd ? `${usd.toLocaleString("fr-FR", { maximumFractionDigits: usd < 0.1 ? 3 : 2 })} $` : "";

const URL_RE = /https?:\/\/(?:localhost|127\.0\.0\.1|0\.0\.0\.0|\[::1\])(?::\d+)?(?:\/[^\s)'"`<>\]]*)?/g;

export function localUrls(text: string): string[] {
  const out = new Set<string>();
  for (const m of text.matchAll(URL_RE)) out.add(m[0].replace(/[.,;:]+$/, "").replace("0.0.0.0", "localhost"));
  return [...out];
}

export const stripAnsi = (s: string) => s.replace(/\x1b\[[0-9;?]*[A-Za-z]/g, "");

export const baseName = (p?: string | null) => (p ? p.split(/[\\/]/).filter(Boolean).pop() ?? p : "");

export const STATUS_LABEL: Record<string, string> = {
  idle: "En attente",
  thinking: "Réfléchit",
  working: "Travaille",
  waiting: "Attend ton accord",
  done: "Terminé",
  error: "Erreur",
  ended: "Fermée",
};

export const isActive = (status: string) => status === "thinking" || status === "working" || status === "waiting";
