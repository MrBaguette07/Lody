import { useEffect, useMemo, useRef, useState } from "react";
import { ChevronDown, ChevronRight, ExternalLink, Play, RefreshCw, RotateCw, ScrollText, Square } from "lucide-react";
import { api, errText } from "../lib/api";
import { useStore } from "../lib/store";
import { localUrls } from "../lib/format";
import type { GitDiff } from "../lib/types";
import ProjectPicker from "./ProjectPicker";

function DiffFile({ chunk }: { chunk: string }) {
  const [openState, setOpen] = useState(true);
  const lines = chunk.split("\n");
  const name = /^diff --git a\/(.+?) b\//.exec(lines[0])?.[1] ?? lines[0];
  const added = lines.filter((l) => l.startsWith("+") && !l.startsWith("+++")).length;
  const removed = lines.filter((l) => l.startsWith("-") && !l.startsWith("---")).length;
  return (
    <div className="diff-file">
      <button className="diff-head" onClick={() => setOpen((o) => !o)}>
        {openState ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        <span>{name}</span>
        <i className="add">+{added}</i>
        <i className="del">−{removed}</i>
      </button>
      {openState && (
        <pre className="diff-body">
          {lines
            .filter((l) => !/^(diff --git|index |--- |\+\+\+ )/.test(l))
            .map((l, i) => (
              <div
                key={i}
                className={l.startsWith("+") ? "add" : l.startsWith("-") ? "del" : l.startsWith("@@") ? "hunk" : ""}
              >
                {l || " "}
              </div>
            ))}
        </pre>
      )}
    </div>
  );
}

function DiffPane({ cwd }: { cwd: string }) {
  const [diff, setDiff] = useState<GitDiff | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const load = async () => {
    setLoading(true);
    setError("");
    try {
      setDiff(await api.gitDiff(cwd));
    } catch (e) {
      setError(errText(e));
    } finally {
      setLoading(false);
    }
  };
  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [cwd]);
  const chunks = useMemo(() => (diff?.diff ? diff.diff.split(/\n(?=diff --git )/) : []), [diff]);
  const untracked = diff?.files.filter((f) => f.status === "??") ?? [];

  return (
    <div className="scroll">
      <div className="diff-bar">
        {diff?.isRepo && <span className="tag">{diff.branch || "HEAD"}</span>}
        <span className="muted">{diff ? `${diff.files.length} fichier${diff.files.length > 1 ? "s" : ""} modifié${diff.files.length > 1 ? "s" : ""}` : ""}</span>
        <span className="grow" />
        <button className="btn sm ghost" onClick={load} disabled={loading}>
          <RefreshCw size={13} className={loading ? "spin" : ""} /> Actualiser
        </button>
      </div>
      {error && <div className="msg-error">{error}</div>}
      {diff && !diff.isRepo && <div className="empty"><p>Ce dossier n'est pas un dépôt git.</p></div>}
      {diff?.isRepo && !diff.files.length && <div className="empty"><p>Aucun changement. Tout est propre ✨</p></div>}
      {chunks.map((c, i) => (
        <DiffFile key={i} chunk={c} />
      ))}
      {untracked.length > 0 && (
        <div className="diff-file">
          <div className="diff-head static">Nouveaux fichiers non suivis</div>
          <pre className="diff-body">
            {untracked.map((f) => (
              <div key={f.path} className="add">
                + {f.path}
              </div>
            ))}
          </pre>
        </div>
      )}
      {diff?.truncated && <p className="muted center">Diff tronqué (trop volumineux).</p>}
    </div>
  );
}

function WebPane({ cwd }: { cwd?: string }) {
  const preview = useStore((s) => s.preview);
  const logs = useStore((s) => s.procLogs);
  const running = useStore((s) => s.procRunning);
  const { setPreview, setProcRunning, showToast } = useStore.getState();
  const [frameKey, setFrameKey] = useState(0);
  const [showLogs, setShowLogs] = useState(false);
  const [urlDraft, setUrlDraft] = useState(preview.url);
  const logEnd = useRef<HTMLDivElement>(null);

  useEffect(() => setUrlDraft(preview.url), [preview.url]);

  useEffect(() => {
    if (!cwd || preview.command) return;
    api.suggestCommand(cwd).then((c) => c && setPreview({ command: c }));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [cwd]);

  // Détecte l'URL du serveur de dev dans les logs.
  useEffect(() => {
    if (!running) return;
    const found = localUrls(logs.slice(-20).join("\n"))[0];
    if (found && found !== preview.url) setPreview({ url: found });
    logEnd.current?.scrollIntoView({ block: "end" });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [logs.length]);

  const start = async () => {
    if (!cwd || !preview.command.trim()) return;
    useStore.setState({ procLogs: [`$ ${preview.command}`] });
    try {
      await api.startProcess("preview", cwd, preview.command);
      setProcRunning(true);
      setShowLogs(true);
    } catch (e) {
      showToast(errText(e));
    }
  };
  const stopProc = async () => {
    await api.stopProcess("preview");
    setProcRunning(false);
  };

  const go = () => {
    let u = urlDraft.trim();
    if (u && !/^https?:\/\//.test(u)) u = `http://${u}`;
    setPreview({ url: u });
    setFrameKey((k) => k + 1);
  };

  return (
    <div className="web">
      <div className="run-bar">
        <input
          className="input mono"
          placeholder="Commande (ex : npm run dev)"
          value={preview.command}
          onChange={(e) => setPreview({ command: e.target.value })}
          disabled={running}
        />
        {running ? (
          <button className="btn danger sm" onClick={stopProc}>
            <Square size={11} fill="currentColor" /> Stop
          </button>
        ) : (
          <button className="btn primary sm" disabled={!cwd || !preview.command.trim()} onClick={start}>
            <Play size={12} fill="currentColor" /> Lancer
          </button>
        )}
        <button className={`btn sm icon ${showLogs ? "soft" : "ghost"}`} title="Logs" onClick={() => setShowLogs((s) => !s)}>
          <ScrollText size={14} />
        </button>
      </div>
      <div className="url-bar">
        <input
          className="input"
          placeholder="http://localhost:5173"
          value={urlDraft}
          onChange={(e) => setUrlDraft(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && go()}
        />
        <button className="btn sm ghost icon" title="Recharger" onClick={() => setFrameKey((k) => k + 1)}>
          <RotateCw size={14} />
        </button>
        <button className="btn sm ghost icon" title="Ouvrir dans le navigateur" disabled={!preview.url} onClick={() => api.openUrl(preview.url)}>
          <ExternalLink size={14} />
        </button>
      </div>
      {showLogs && (
        <pre className="logs">
          {logs.join("\n")}
          <div ref={logEnd} />
        </pre>
      )}
      <div className="frame">
        {preview.url ? (
          <iframe key={frameKey} src={preview.url} title="Aperçu" />
        ) : (
          <div className="empty">
            <p>
              Lance le serveur de dev du projet ou colle une URL. Les liens <code>localhost</code> donnés par les agents
              s'ouvrent aussi ici.
            </p>
          </div>
        )}
      </div>
    </div>
  );
}

export default function PreviewView() {
  const preview = useStore((s) => s.preview);
  const conv = useStore((s) => (s.activeId ? s.convs[s.activeId] : undefined));
  const setPreview = useStore((s) => s.setPreview);
  const cwd = preview.cwd ?? conv?.cwd;

  return (
    <div className="view">
      <div className="view-head">
        <h1>Aperçu</h1>
        <ProjectPicker value={cwd} onChange={(p) => setPreview({ cwd: p, command: "" })} />
        <span className="grow" />
        <div className="seg">
          <button className={preview.mode === "web" ? "on" : ""} onClick={() => setPreview({ mode: "web" })}>
            Web
          </button>
          <button className={preview.mode === "diff" ? "on" : ""} onClick={() => setPreview({ mode: "diff" })}>
            Changements
          </button>
        </div>
      </div>
      {preview.mode === "web" ? (
        <WebPane cwd={cwd} />
      ) : cwd ? (
        <DiffPane cwd={cwd} />
      ) : (
        <div className="empty">
          <p>Choisis un projet pour voir ses changements.</p>
        </div>
      )}
    </div>
  );
}
