import { useEffect, useMemo, useRef, useState } from "react";
import { ArrowUp, ExternalLink, GitPullRequest, MessageCircle, RefreshCw, Sparkles, Wrench } from "lucide-react";
import Lody from "../components/Lody";
import { CiIcon, runState } from "../components/ci";
import { PROVIDER_LABEL } from "../components/icons";
import { api, errText } from "../lib/api";
import { useStore } from "../lib/store";
import { STATUS_LABEL, ago, isActive } from "../lib/format";
import type { Download, GhOverview } from "../lib/types";
import ProjectPicker from "./ProjectPicker";

let ghCache: { at: number; key: string; data: GhOverview } | null = null;

export default function HomeView() {
  const [text, setText] = useState("");
  const [target, setTarget] = useState<string | undefined>();
  const [gh, setGh] = useState<GhOverview | null>(ghCache?.data ?? null);
  const [ghLoading, setGhLoading] = useState(false);
  const [downloads, setDownloads] = useState<Download[]>([]);
  const agents = useStore((s) => s.agents);
  const projects = useStore((s) => s.projects);
  const runConv = useStore((s) => s.runConv);
  const { ask, setActive, setTab, showToast } = useStore.getState();
  const input = useRef<HTMLTextAreaElement>(null);

  const slugs = useMemo(
    () =>
      projects
        .filter((p) => p.path && p.github)
        .sort((a, b) => (b.updatedAt ?? 0) - (a.updatedAt ?? 0))
        .slice(0, 6)
        .map((p) => p.github!),
    [projects],
  );

  const loadGh = async (force = false) => {
    const key = slugs.join(",");
    if (!force && ghCache && ghCache.key === key && Date.now() - ghCache.at < 120_000) return;
    setGhLoading(true);
    try {
      const data = await api.ghOverview(slugs);
      ghCache = { at: Date.now(), key, data };
      setGh(data);
    } catch (e) {
      showToast(errText(e));
    } finally {
      setGhLoading(false);
    }
  };

  useEffect(() => {
    if (slugs.length) loadGh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [slugs.join(",")]);

  useEffect(() => {
    api.recentDownloads(3).then(setDownloads).catch(() => {});
    input.current?.focus();
  }, []);

  const submit = (t = text) => {
    const v = t.trim();
    if (!v) return;
    setText("");
    ask(v, target);
  };

  const latest = downloads[0];
  const suggestions = [
    "Mets-moi une musique sympa sur Spotify",
    latest && `Range « ${latest.name} » au bon endroit dans mes projets`,
    "Quelles PR attendent ma review ?",
    "Pourquoi ma dernière CI est rouge ? Corrige-la",
    "Fais un commit propre de mes changements en cours",
  ].filter(Boolean) as string[];

  const shown = agents.filter((a) => isActive(a.status) || Date.now() - a.updatedAt < 30 * 60_000).slice(0, 6);
  const ciSorted = [...(gh?.ci ?? [])].sort(
    (a, b) => Number(runState(b.status, b.conclusion) === "fail") - Number(runState(a.status, a.conclusion) === "fail"),
  );

  return (
    <div className="home scroll">
      <div className="command">
        <div className="command-box">
          <Lody mood={text ? "curious" : "idle"} size={30} />
          <textarea
            ref={input}
            rows={1}
            value={text}
            placeholder="Qu'est-ce que je fais pour toi ?"
            onChange={(e) => setText(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                submit();
              }
            }}
          />
          <button className="btn primary icon" disabled={!text.trim()} onClick={() => submit()} title="Envoyer (Entrée)">
            <ArrowUp size={17} />
          </button>
        </div>
        <div className="command-opts">
          <ProjectPicker value={target} onChange={(p) => setTarget(p || undefined)} allLabel="Tous mes projets" />
          <span className="muted small">Lody trouve le projet, code, commit, gère PR et CI.</span>
        </div>
        <div className="chips">
          {suggestions.map((s) => (
            <button key={s} className="chip" onClick={() => submit(s)}>
              <Sparkles size={12} /> {s}
            </button>
          ))}
        </div>
      </div>

      <div className="home-grid">
        <section className="home-card">
          <header>
            <h3>En cours</h3>
            <button className="btn ghost sm" onClick={() => setTab("agents")}>
              Tout voir
            </button>
          </header>
          {shown.map((a) => {
            const convId = a.runId ? runConv[a.runId] : undefined;
            return (
              <button
                key={a.id}
                className={`row st-${a.status}`}
                onClick={() => {
                  if (convId) {
                    setActive(convId);
                    setTab("chat");
                  } else setTab("agents");
                }}
              >
                <span className="dot" />
                <div className="row-main">
                  <strong>{a.title || a.project || "Session"}</strong>
                  <span>
                    {a.project} · {STATUS_LABEL[a.status]} {a.activity && isActive(a.status) ? `· ${a.activity}` : ""}
                  </span>
                </div>
                <span className="row-side">{PROVIDER_LABEL[a.provider] ?? a.provider}</span>
              </button>
            );
          })}
          {!shown.length && <p className="muted small">Rien en cours. Demande-moi quelque chose !</p>}
        </section>

        <section className="home-card">
          <header>
            <h3>GitHub</h3>
            <button className="btn ghost icon sm" title="Actualiser" onClick={() => loadGh(true)} disabled={ghLoading}>
              <RefreshCw size={14} className={ghLoading ? "spin" : ""} />
            </button>
          </header>
          {ciSorted.map((r) => {
            const failed = runState(r.status, r.conclusion) === "fail";
            const local = projects.find((p) => p.github === r.repo && p.path);
            return (
              <div key={`${r.repo}-${r.id}`} className="row static">
                <CiIcon status={r.status} conclusion={r.conclusion} />
                <button className="row-main" onClick={() => api.openUrl(r.url)}>
                  <strong>{r.repo?.split("/")[1]}</strong>
                  <span>
                    {r.workflow} · {r.branch} · {ago(r.createdAt)}
                  </span>
                </button>
                {failed && (
                  <button
                    className="btn soft sm"
                    title="Lody analyse les logs et corrige"
                    onClick={() =>
                      ask(
                        `La CI « ${r.workflow} » de ${r.repo} (run ${r.id}, branche ${r.branch}) a échoué. Récupère les logs avec \`gh run view ${r.id} -R ${r.repo} --log-failed\`, trouve la cause et corrige-la dans le projet local. Ne pousse pas sans me demander.`,
                        local?.path ?? undefined,
                      )
                    }
                  >
                    <Wrench size={12} /> Réparer
                  </button>
                )}
              </div>
            );
          })}
          {(gh?.reviews.length ?? 0) > 0 && <h4>Reviews demandées</h4>}
          {gh?.reviews.slice(0, 4).map((p) => (
            <div key={p.url} className="row static">
              <GitPullRequest size={15} className="ci-run" />
              <button className="row-main" onClick={() => api.openUrl(p.url)}>
                <strong>{p.title}</strong>
                <span>
                  {p.repo}#{p.number} · {ago(p.updatedAt)}
                </span>
              </button>
              <button
                className="btn soft sm"
                onClick={() =>
                  ask(
                    `Fais la review de la PR ${p.url} (gh pr view / gh pr diff). Donne-moi un résumé, les risques et tes suggestions. Ne poste rien sur GitHub sans mon accord.`,
                  )
                }
              >
                <MessageCircle size={12} /> Review
              </button>
            </div>
          ))}
          {(gh?.myPrs.length ?? 0) > 0 && <h4>Mes PR ouvertes</h4>}
          {gh?.myPrs.slice(0, 5).map((p) => (
            <button key={p.url} className="row" onClick={() => api.openUrl(p.url)}>
              <GitPullRequest size={15} className={p.draft ? "ci-off" : "ci-ok"} />
              <div className="row-main">
                <strong>{p.title}</strong>
                <span>
                  {p.repo}#{p.number} · {ago(p.updatedAt)}
                </span>
              </div>
              <ExternalLink size={13} className="muted" />
            </button>
          ))}
          {gh && !gh.ci.length && !gh.myPrs.length && !gh.reviews.length && (
            <p className="muted small">Tout est calme côté GitHub ✨</p>
          )}
          {!gh && !ghLoading && <p className="muted small">Connecte `gh` (gh auth login) pour voir tes PR et ta CI.</p>}
        </section>
      </div>
    </div>
  );
}
