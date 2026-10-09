import { useMemo, useState } from "react";
import {
  Code2,
  Download,
  ExternalLink,
  Eye,
  FolderOpen,
  Lock,
  MessageCircle,
  RefreshCw,
  Search,
  SquareTerminal,
  Star,
  Workflow,
  Folder,
  GitBranch,
  TriangleAlert,
} from "lucide-react";
import { GithubMark } from "../components/icons";
import { api, errText } from "../lib/api";
import { useStore } from "../lib/store";
import { ago } from "../lib/format";
import type { Project } from "../lib/types";

const SOURCE_LABEL: Record<string, string> = {
  dossier: "Dossier",
  vscode: "VS Code",
  cursor: "Cursor",
  github: "GitHub",
};

function ProjectRow({ p }: { p: Project }) {
  const favorites = useStore((s) => s.favorites);
  const { toggleFav, useProject, setPreview, setTab, showToast, loadProjects, setGitCwd } = useStore.getState();
  const [cloning, setCloning] = useState(false);
  const fav = favorites.includes(p.id);
  const Icon = p.kind === "item" ? Workflow : p.kind === "remote" ? GithubMark : Folder;

  const clone = async () => {
    if (!p.github) return;
    setCloning(true);
    try {
      const path = await api.cloneRepo(p.github);
      showToast(`Cloné dans ${path}`);
      await loadProjects();
    } catch (e) {
      showToast(errText(e));
    } finally {
      setCloning(false);
    }
  };

  return (
    <div className="project">
      <button className={`star ${fav ? "on" : ""}`} onClick={() => toggleFav(p.id)} title="Favori">
        <Star size={14} fill={fav ? "currentColor" : "none"} />
      </button>
      <span className={`project-icon k-${p.kind}`}>
        <Icon size={15} />
      </span>
      <div className="project-main">
        <div className="project-name">
          <strong>{p.name}</strong>
          {p.private && <Lock size={11} className="muted" />}
          {p.updatedAt ? <span className="project-time">{ago(p.updatedAt)}</span> : null}
        </div>
        <div className="project-sub">
          {p.sources.map((s) => (
            <span key={s} className="tag">
              {SOURCE_LABEL[s] ?? s}
            </span>
          ))}
          <span className="project-path">{p.path ?? p.description ?? p.url}</span>
        </div>
      </div>
      <div className="project-actions">
        {p.path ? (
          <>
            <button className="btn sm soft" title="Discuter de ce projet" onClick={() => useProject(p.path!, true)}>
              <MessageCircle size={13} /> Chat
            </button>
            <button className="btn sm icon" title="Git, PR et CI" onClick={() => setGitCwd(p.path!)}>
              <GitBranch size={14} />
            </button>
            <button className="btn sm icon" title="Ouvrir dans VS Code" onClick={() => api.openIn(p.path!, "code")}>
              <Code2 size={14} />
            </button>
            <button className="btn sm icon" title="Explorateur" onClick={() => api.openIn(p.path!, "explorer")}>
              <FolderOpen size={14} />
            </button>
            <button className="btn sm icon" title="Terminal" onClick={() => api.openIn(p.path!, "terminal")}>
              <SquareTerminal size={14} />
            </button>
            <button
              className="btn sm icon"
              title="Aperçu"
              onClick={() => {
                setPreview({ cwd: p.path!, mode: "web" });
                setTab("preview");
              }}
            >
              <Eye size={14} />
            </button>
          </>
        ) : p.github ? (
          <button className="btn sm soft" disabled={cloning} onClick={clone}>
            <Download size={13} className={cloning ? "spin" : ""} /> {cloning ? "Clonage…" : "Cloner"}
          </button>
        ) : null}
        {p.url && (
          <button className="btn sm icon" title="Ouvrir le lien" onClick={() => api.openUrl(p.url!)}>
            <ExternalLink size={14} />
          </button>
        )}
      </div>
    </div>
  );
}

export default function ProjectsView() {
  const projects = useStore((s) => s.projects);
  const warnings = useStore((s) => s.projectWarnings);
  const loading = useStore((s) => s.projectsLoading);
  const favorites = useStore((s) => s.favorites);
  const loadProjects = useStore((s) => s.loadProjects);
  const [q, setQ] = useState("");
  const [source, setSource] = useState("all");
  const [showWarn, setShowWarn] = useState(false);

  const sources = useMemo(() => {
    const set = new Set<string>();
    projects.forEach((p) => p.sources.forEach((s) => set.add(s)));
    return [...set];
  }, [projects]);

  const list = useMemo(() => {
    const ql = q.toLowerCase();
    return projects
      .filter((p) => source === "all" || (source === "local" ? !!p.path : p.sources.includes(source)))
      .filter(
        (p) =>
          !ql ||
          p.name.toLowerCase().includes(ql) ||
          (p.path ?? "").toLowerCase().includes(ql) ||
          (p.description ?? "").toLowerCase().includes(ql),
      )
      .sort((a, b) => Number(favorites.includes(b.id)) - Number(favorites.includes(a.id)));
  }, [projects, q, source, favorites]);

  return (
    <div className="view">
      <div className="view-head">
        <h1>Projets</h1>
        <span className="muted">{projects.length}</span>
        <span className="grow" />
        <button className="btn ghost icon sm" title="Actualiser" onClick={loadProjects} disabled={loading}>
          <RefreshCw size={15} className={loading ? "spin" : ""} />
        </button>
      </div>
      <div className="filters">
        <div className="search">
          <Search size={14} />
          <input placeholder="Rechercher…" value={q} onChange={(e) => setQ(e.target.value)} />
        </div>
        <div className="chips">
          {["all", "local", ...sources].map((s) => (
            <button key={s} className={`chip ${source === s ? "on" : ""}`} onClick={() => setSource(s)}>
              {s === "all" ? "Tous" : s === "local" ? "Sur ce PC" : SOURCE_LABEL[s] ?? s}
            </button>
          ))}
        </div>
        {warnings.length > 0 && (
          <button className="warn-line" onClick={() => setShowWarn((w) => !w)}>
            <TriangleAlert size={13} /> {warnings.length} source{warnings.length > 1 ? "s" : ""} en erreur
            {showWarn && (
              <ul>
                {warnings.map((w) => (
                  <li key={w}>{w}</li>
                ))}
              </ul>
            )}
          </button>
        )}
      </div>
      <div className="scroll">
        {list.map((p) => (
          <ProjectRow key={p.id} p={p} />
        ))}
        {!list.length && (
          <div className="empty">
            <p>{loading ? "Je cherche tes projets…" : "Aucun projet. Ajoute des sources dans Réglages."}</p>
          </div>
        )}
      </div>
    </div>
  );
}
