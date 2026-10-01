import { useEffect, useMemo, useState, type ReactNode } from "react";
import {
  ArrowDown,
  ArrowUp,
  Check,
  Code2,
  ExternalLink,
  FileDiff,
  GitBranch,
  GitCommitHorizontal,
  GitMerge,
  GitPullRequest,
  GitPullRequestCreate,
  MessageCircle,
  RefreshCw,
  RotateCcw,
  Sparkles,
  Wrench,
} from "lucide-react";
import { ChecksIcon, CiIcon, runState } from "../components/ci";
import { api, errText } from "../lib/api";
import { isOrch, useStore } from "../lib/store";
import { ago, baseName } from "../lib/format";
import type { PullRequest, RepoStatus } from "../lib/types";
import ProjectPicker from "./ProjectPicker";

/** Bouton qui demande une seconde confirmation avant une action sensible. */
function ConfirmButton({ label, onConfirm, className = "btn sm", icon }: { label: string; onConfirm: () => void; className?: string; icon?: ReactNode }) {
  const [armed, setArmed] = useState(false);
  useEffect(() => {
    if (!armed) return;
    const t = setTimeout(() => setArmed(false), 3000);
    return () => clearTimeout(t);
  }, [armed]);
  return (
    <button
      className={`${className} ${armed ? "danger" : ""}`}
      onClick={() => {
        if (armed) {
          setArmed(false);
          onConfirm();
        } else setArmed(true);
      }}
    >
      {icon} {armed ? "Confirmer ?" : label}
    </button>
  );
}

const REVIEW_LABEL: Record<string, string> = {
  APPROVED: "Approuvée",
  CHANGES_REQUESTED: "Changements demandés",
  REVIEW_REQUIRED: "Review requise",
};

export default function GitView() {
  const gitCwd = useStore((s) => s.gitCwd);
  const projects = useStore((s) => s.projects);
  const recent = useStore((s) => s.recentCwds);
  const conv = useStore((s) => (s.activeId ? s.convs[s.activeId] : undefined));
  const { ask, setPreview, setTab, showToast } = useStore.getState();
  const [status, setStatus] = useState<RepoStatus | null>(null);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");

  const fallback = useMemo(
    () =>
      recent[0] ??
      projects.filter((p) => p.path && p.github).sort((a, b) => (b.updatedAt ?? 0) - (a.updatedAt ?? 0))[0]?.path ??
      undefined,
    [projects, recent],
  );
  const cwd = gitCwd ?? (conv && !isOrch(conv) ? conv.cwd : undefined) ?? fallback;

  const load = async () => {
    if (!cwd) return;
    setLoading(true);
    setError("");
    try {
      setStatus(await api.repoStatus(cwd));
    } catch (e) {
      setStatus(null);
      setError(errText(e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    setStatus(null);
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [cwd]);

  const act = async (key: string, fn: () => Promise<string>, done?: string) => {
    setBusy(key);
    try {
      const out = await fn();
      showToast(done ?? (out.trim().split("\n").pop() || "Fait ✨"));
      await load();
    } catch (e) {
      showToast(errText(e));
    } finally {
      setBusy(null);
    }
  };

  const askHere = (text: string) => cwd && ask(text, cwd);
  const failedRuns = status?.runs.filter((r) => runState(r.status, r.conclusion) === "fail") ?? [];

  return (
    <div className="view">
      <div className="view-head">
        <h1>Git</h1>
        <ProjectPicker value={cwd} onChange={(p) => p && useStore.setState({ gitCwd: p })} />
        <span className="grow" />
        {cwd && (
          <button className="btn ghost icon sm" title="Ouvrir dans VS Code" onClick={() => api.openIn(cwd, "code")}>
            <Code2 size={15} />
          </button>
        )}
        <button className="btn ghost icon sm" title="Actualiser" onClick={load} disabled={loading || !cwd}>
          <RefreshCw size={15} className={loading ? "spin" : ""} />
        </button>
      </div>
      <div className="scroll">
        {!cwd && <div className="empty"><p>Choisis un projet pour voir son état Git, ses PR et sa CI.</p></div>}
        {error && <div className="msg-error">{error}</div>}
        {loading && !status && <p className="muted center">Lecture du dépôt…</p>}

        {status && cwd && (
          <>
            <section className="git-card">
              <div className="git-branch">
                <GitBranch size={16} />
                <strong>{status.branch}</strong>
                {status.ahead != null && (
                  <span className="tag" title="Commits à pousser / à récupérer">
                    <ArrowUp size={10} /> {status.ahead} <ArrowDown size={10} /> {status.behind}
                  </span>
                )}
                {status.slug && (
                  <button className="tag link" onClick={() => api.openUrl(`https://github.com/${status.slug}`)}>
                    {status.slug}
                  </button>
                )}
                <span className="grow" />
                <button className="btn sm" disabled={!!busy} onClick={() => act("pull", () => api.gitSync(cwd, "pull"))}>
                  <ArrowDown size={13} className={busy === "pull" ? "spin" : ""} /> Pull
                </button>
                <button className="btn sm" disabled={!!busy} onClick={() => act("push", () => api.gitSync(cwd, "push"), "Poussé ✨")}>
                  <ArrowUp size={13} className={busy === "push" ? "spin" : ""} /> Push
                </button>
              </div>

              {status.files.length > 0 ? (
                <>
                  <div className="git-files">
                    {status.files.slice(0, 8).map((f) => (
                      <span key={f.path} className="git-file" title={f.path}>
                        <i className={`fs fs-${f.status === "??" ? "new" : f.status.includes("D") ? "del" : "mod"}`}>{f.status === "??" ? "+" : f.status}</i>
                        {baseName(f.path)}
                      </span>
                    ))}
                    {status.files.length > 8 && <span className="muted small">+{status.files.length - 8}</span>}
                  </div>
                  <div className="card-row">
                    <input
                      className="input"
                      placeholder={`Message de commit (${status.files.length} fichier${status.files.length > 1 ? "s" : ""})`}
                      value={message}
                      onChange={(e) => setMessage(e.target.value)}
                      onKeyDown={(e) => e.key === "Enter" && message.trim() && act("commit", () => api.gitCommit(cwd, message).then((o) => (setMessage(""), o)))}
                    />
                    <button
                      className="btn sm primary"
                      disabled={!message.trim() || !!busy}
                      onClick={() => act("commit", () => api.gitCommit(cwd, message).then((o) => (setMessage(""), o)))}
                    >
                      <GitCommitHorizontal size={13} /> Commit
                    </button>
                  </div>
                  <div className="card-row wrap">
                    <button
                      className="btn sm soft"
                      onClick={() =>
                        askHere(
                          "Regarde les changements non commités (git status / git diff), regroupe-les logiquement et fais un ou plusieurs commits avec des messages clairs (convention du dépôt). Ne pousse pas.",
                        )
                      }
                    >
                      <Sparkles size={13} /> Commit avec Lody
                    </button>
                    <button
                      className="btn sm ghost"
                      onClick={() => {
                        setPreview({ cwd, mode: "diff" });
                        setTab("preview");
                      }}
                    >
                      <FileDiff size={13} /> Voir le diff
                    </button>
                  </div>
                </>
              ) : (
                <p className="muted small">
                  <Check size={12} /> Rien à commiter.
                </p>
              )}
            </section>

            {status.slug && (
              <section className="git-card">
                <header>
                  <h3>Pull requests</h3>
                  <span className="muted small">{status.prs.length}</span>
                  <span className="grow" />
                  <button className="btn sm" disabled={!!busy} onClick={() => act("pr", () => api.ghPrCreate(cwd, false), undefined)}>
                    <GitPullRequestCreate size={13} className={busy === "pr" ? "spin" : ""} /> Créer
                  </button>
                  <button
                    className="btn sm soft"
                    onClick={() =>
                      askHere(
                        "Pousse la branche courante et ouvre une pull request avec gh : titre clair et description (contexte, changements, comment tester). Donne-moi le lien.",
                      )
                    }
                  >
                    <Sparkles size={13} /> PR avec Lody
                  </button>
                </header>
                {status.prs.map((p: PullRequest) => (
                  <div key={p.number} className="row static">
                    <ChecksIcon checks={p.checks} />
                    <button className="row-main" onClick={() => api.openUrl(p.url)}>
                      <strong>
                        #{p.number} {p.title}
                      </strong>
                      <span>
                        {p.branch} · {p.author}
                        {p.draft ? " · brouillon" : ""}
                        {p.review ? ` · ${REVIEW_LABEL[p.review] ?? p.review}` : ""} · {ago(p.updatedAt)}
                      </span>
                    </button>
                    <div className="row-actions">
                      <button className="btn sm ghost" title="Basculer sur cette branche" onClick={() => act(`co${p.number}`, () => api.ghPrAction(cwd, p.number, "checkout"), `Sur la branche ${p.branch}`)}>
                        Checkout
                      </button>
                      <button
                        className="btn sm ghost icon"
                        title="Review avec Lody"
                        onClick={() =>
                          askHere(`Fais la review de la PR #${p.number} (gh pr view ${p.number}, gh pr diff ${p.number}). Résume, liste les risques et propose des améliorations. Ne poste rien sans mon accord.`)
                        }
                      >
                        <MessageCircle size={13} />
                      </button>
                      <ConfirmButton
                        label="Merge"
                        icon={<GitMerge size={13} />}
                        className="btn sm"
                        onConfirm={() => act(`m${p.number}`, () => api.ghPrAction(cwd, p.number, "merge"), `PR #${p.number} fusionnée ✨`)}
                      />
                    </div>
                  </div>
                ))}
                {!status.prs.length && <p className="muted small">Aucune PR ouverte.</p>}
              </section>
            )}

            {status.slug && (
              <section className="git-card">
                <header>
                  <h3>CI / CD</h3>
                  {failedRuns.length > 0 && <span className="tag fail">{failedRuns.length} en échec</span>}
                </header>
                {status.runs.map((r) => {
                  const failed = runState(r.status, r.conclusion) === "fail";
                  return (
                    <div key={r.id} className="row static">
                      <CiIcon status={r.status} conclusion={r.conclusion} />
                      <button className="row-main" onClick={() => api.openUrl(r.url)}>
                        <strong>{r.title}</strong>
                        <span>
                          {r.workflow} · {r.branch} · {r.event} · {ago(r.createdAt)}
                        </span>
                      </button>
                      {failed && (
                        <div className="row-actions">
                          <button className="btn sm ghost icon" title="Relancer les jobs en échec" onClick={() => act(`r${r.id}`, () => api.ghRunRerun(cwd, r.id), "Relancé")}>
                            <RotateCcw size={13} />
                          </button>
                          <button
                            className="btn sm soft"
                            onClick={() =>
                              askHere(
                                `Le run CI ${r.id} (« ${r.workflow} », branche ${r.branch}) a échoué. Récupère les logs avec \`gh run view ${r.id} --log-failed\`, trouve la cause et corrige. Ne pousse pas sans me demander.`,
                              )
                            }
                          >
                            <Wrench size={12} /> Réparer
                          </button>
                        </div>
                      )}
                      {!failed && <ExternalLink size={13} className="muted" />}
                    </div>
                  );
                })}
                {!status.runs.length && <p className="muted small">Pas de workflow GitHub Actions.</p>}
              </section>
            )}

            <section className="git-card">
              <header>
                <h3>Derniers commits</h3>
              </header>
              {status.commits.map((c) => (
                <div key={c.hash} className="commit">
                  <code>{c.hash}</code>
                  <span className="commit-subject">{c.subject}</span>
                  <span className="muted small">{ago(c.date)}</span>
                </div>
              ))}
            </section>

            {!status.slug && <p className="muted small center"><GitPullRequest size={12} /> Pas de remote GitHub : PR et CI indisponibles.</p>}
            {status.warnings.map((w) => (
              <p key={w} className="muted small">
                ⚠ {w}
              </p>
            ))}
          </>
        )}
      </div>
    </div>
  );
}
