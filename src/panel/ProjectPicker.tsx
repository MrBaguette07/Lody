import { useEffect, useMemo, useRef, useState } from "react";
import { open } from "../lib/dialog";
import { ChevronDown, FolderOpen, FolderSearch, Globe, Search } from "lucide-react";
import { useStore } from "../lib/store";
import { baseName } from "../lib/format";

/** Sélecteur de dossier de travail : projets locaux connus + parcourir. */
export default function ProjectPicker({
  value,
  onChange,
  placeholder = "Choisir un projet",
  allLabel,
}: {
  value?: string;
  /** Chemin choisi, ou "" pour l'option « tous les projets ». */
  onChange: (path: string) => void;
  placeholder?: string;
  allLabel?: string;
}) {
  const [openState, setOpen] = useState(false);
  const [q, setQ] = useState("");
  const ref = useRef<HTMLDivElement>(null);
  const projects = useStore((s) => s.projects);
  const favorites = useStore((s) => s.favorites);
  const recent = useStore((s) => s.recentCwds);

  useEffect(() => {
    if (!openState) return;
    const close = (e: MouseEvent) => !ref.current?.contains(e.target as Node) && setOpen(false);
    window.addEventListener("mousedown", close);
    return () => window.removeEventListener("mousedown", close);
  }, [openState]);

  const items = useMemo(() => {
    const local = projects.filter((p) => p.path);
    const rank = (path: string, id: string) =>
      (favorites.includes(id) ? 0 : 2) + (recent.includes(path) ? 0 : 1);
    const ql = q.toLowerCase();
    return local
      .filter((p) => !ql || p.name.toLowerCase().includes(ql) || p.path!.toLowerCase().includes(ql))
      .sort((a, b) => rank(a.path!, a.id) - rank(b.path!, b.id))
      .slice(0, 40);
  }, [projects, favorites, recent, q]);

  const browse = async () => {
    const dir = await open({ directory: true, multiple: false, defaultPath: value });
    if (typeof dir === "string") {
      onChange(dir);
      setOpen(false);
    }
  };

  return (
    <div className="picker" ref={ref}>
      <button className={`chip ${value || allLabel ? "" : "chip-empty"}`} onClick={() => setOpen((o) => !o)} title={value}>
        {!value && allLabel ? <Globe size={14} /> : <FolderOpen size={14} />}
        <span>{value ? baseName(value) : allLabel ?? placeholder}</span>
        <ChevronDown size={13} />
      </button>
      {openState && (
        <div className="popover picker-pop">
          <div className="search">
            <Search size={14} />
            <input autoFocus placeholder="Rechercher un projet…" value={q} onChange={(e) => setQ(e.target.value)} />
          </div>
          <div className="picker-list">
            {allLabel && (
              <button
                className={`picker-item ${!value ? "active" : ""}`}
                onClick={() => {
                  onChange("");
                  setOpen(false);
                }}
              >
                <strong>{allLabel}</strong>
                <span>Lody choisit le bon projet selon ta demande</span>
              </button>
            )}
            {items.map((p) => (
              <button
                key={p.id}
                className={`picker-item ${p.path === value ? "active" : ""}`}
                onClick={() => {
                  onChange(p.path!);
                  setOpen(false);
                }}
              >
                <strong>{p.name}</strong>
                <span>{p.path}</span>
              </button>
            ))}
            {!items.length && <div className="empty-mini">Aucun projet local trouvé.</div>}
          </div>
          <button className="btn ghost picker-browse" onClick={browse}>
            <FolderSearch size={15} /> Parcourir…
          </button>
        </div>
      )}
    </div>
  );
}
