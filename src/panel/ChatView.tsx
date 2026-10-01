import { useEffect, useMemo, useRef, useState } from "react";
import { open } from "../lib/dialog";
import {
  ArrowUp,
  Check,
  ChevronDown,
  ChevronRight,
  Eye,
  LoaderCircle,
  Paperclip,
  Plus,
  Square,
  Trash2,
  TriangleAlert,
  X,
} from "lucide-react";
import Lody from "../components/Lody";
import Markdown from "../components/Markdown";
import { PROVIDER_LABEL, toolIcon } from "../components/icons";
import { useStore, isCli, isOrch, msgText } from "../lib/store";
import { ago, baseName, cost, duration, localUrls } from "../lib/format";
import type { Conversation, Mode, Msg, Part } from "../lib/types";
import ProjectPicker from "./ProjectPicker";

const LODY_SUGGESTIONS = [
  "Mets-moi une musique sympa sur Spotify",
  "Ajoute mon dernier PDF téléchargé en compta",
  "Quelles PR attendent ma review ?",
  "Résume ce que j'ai fait cette semaine sur mes projets",
  "Quel projet a une CI rouge ? Corrige-la",
];

const SUGGESTIONS = [
  "Explique-moi ce projet en quelques lignes",
  "Trouve et corrige les erreurs de build",
  "Ajoute des tests pour les fonctions principales",
  "Lance le serveur de dev et dis-moi l'URL",
];

const MODES: { id: Mode; label: string; hint: string }[] = [
  { id: "ask", label: "Demander", hint: "Lody te demande avant chaque commande ou modification" },
  { id: "edits", label: "Éditions auto", hint: "Modifs de fichiers auto, commandes à valider" },
  { id: "yolo", label: "Libre", hint: "Aucune validation : l'agent fait tout seul" },
];

function ToolRow({ part }: { part: Extract<Part, { type: "tool" }> }) {
  const [openState, setOpen] = useState(false);
  const Icon = toolIcon(part.name);
  return (
    <div className={`tool ${part.status} ${part.sub ? "sub" : ""}`}>
      <button className="tool-head" onClick={() => setOpen((o) => !o)} disabled={!part.output && !part.input}>
        <Icon size={13} />
        <span className="tool-detail">{part.detail}</span>
        {part.status === "run" ? (
          <LoaderCircle size={13} className="spin" />
        ) : part.status === "ok" ? (
          <Check size={13} className="tool-ok" />
        ) : (
          <X size={13} className="tool-err" />
        )}
        {(part.output || part.input) && (openState ? <ChevronDown size={13} /> : <ChevronRight size={13} />)}
      </button>
      {openState && <pre className="tool-out">{part.output || part.input}</pre>}
    </div>
  );
}

function Message({ m, onPreview }: { m: Msg; onPreview: (url: string) => void }) {
  if (m.role === "user") {
    return (
      <div className="msg user">
        <div className="bubble">
          <div className="md">{msgText(m)}</div>
          {!!m.attachments?.length && (
            <div className="attach-list">
              {m.attachments.map((a) => (
                <span key={a} className="attach" title={a}>
                  <Paperclip size={11} /> {baseName(a)}
                </span>
              ))}
            </div>
          )}
        </div>
      </div>
    );
  }
  const text = msgText(m);
  const urls = localUrls(text + "\n" + m.parts.map((p) => (p.type === "tool" ? p.output ?? "" : "")).join("\n"));
  return (
    <div className="msg assistant">
      <div className="msg-avatar">
        <Lody mood={m.pending ? (m.parts.length ? "work" : "think") : m.error ? "error" : "idle"} size={26} />
      </div>
      <div className="msg-body">
        {m.parts.map((p, i) =>
          p.type === "text" ? <Markdown key={i} text={p.text} /> : <ToolRow key={p.id + i} part={p} />,
        )}
        {m.pending && !m.parts.length && (
          <div className="typing">
            <i />
            <i />
            <i />
          </div>
        )}
        {m.error && (
          <div className="msg-error">
            <TriangleAlert size={14} />
            <span>{m.error}</span>
          </div>
        )}
        {!m.pending && (urls.length > 0 || m.meta?.costUsd || m.meta?.durationMs) && (
          <div className="msg-meta">
            {urls.slice(0, 3).map((u) => (
              <button key={u} className="btn soft sm" onClick={() => onPreview(u)}>
                <Eye size={12} /> {u.replace(/^https?:\/\//, "")}
              </button>
            ))}
            <span>{[duration(m.meta?.durationMs), cost(m.meta?.costUsd)].filter(Boolean).join(" · ")}</span>
          </div>
        )}
      </div>
    </div>
  );
}

function ConvMenu({ conv }: { conv?: Conversation }) {
  const [openState, setOpen] = useState(false);
  const convs = useStore((s) => s.convs);
  const order = useStore((s) => s.order);
  const { setActive, deleteConv, newConv } = useStore.getState();
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!openState) return;
    const close = (e: MouseEvent) => !ref.current?.contains(e.target as Node) && setOpen(false);
    window.addEventListener("mousedown", close);
    return () => window.removeEventListener("mousedown", close);
  }, [openState]);
  return (
    <div className="conv-menu" ref={ref}>
      <button className="conv-title" onClick={() => setOpen((o) => !o)}>
        <span>{conv?.title ?? "Nouvelle discussion"}</span>
        <ChevronDown size={14} />
      </button>
      <button className="btn ghost icon sm" title="Nouvelle discussion" onClick={() => newConv()}>
        <Plus size={16} />
      </button>
      {openState && (
        <div className="popover conv-pop">
          {order.map((id) => {
            const c = convs[id];
            if (!c) return null;
            return (
              <div key={id} className={`conv-item ${id === conv?.id ? "active" : ""}`}>
                <button
                  onClick={() => {
                    setActive(id);
                    setOpen(false);
                  }}
                >
                  <strong>{c.title}</strong>
                  <span>
                    {PROVIDER_LABEL[c.provider] ?? c.provider.replace("api:", "")}
                    {c.cwd ? ` · ${baseName(c.cwd)}` : ""} · {ago(c.updatedAt)}
                  </span>
                </button>
                <button className="btn ghost icon sm" title="Supprimer" onClick={() => deleteConv(id)}>
                  <Trash2 size={13} />
                </button>
              </div>
            );
          })}
          {!order.length && <div className="empty-mini">Pas encore de discussion.</div>}
        </div>
      )}
    </div>
  );
}

export default function ChatView() {
  const activeId = useStore((s) => s.activeId);
  const conv = useStore((s) => (s.activeId ? s.convs[s.activeId] : undefined));
  const settings = useStore((s) => s.settings);
  const bins = useStore((s) => s.bins);
  const attach = useStore((s) => s.pendingAttach);
  const draft = useStore((s) => s.draft);
  const { send, stop, updateConv, newConv, setPendingAttach, setDraft, setPreview, setTab } = useStore.getState();
  const scroller = useRef<HTMLDivElement>(null);
  const ta = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (!activeId) newConv();
  }, [activeId, newConv]);

  const providers = useMemo(
    () => [
      { value: "claude", label: "Claude Code", hint: bins.claude ? "agent de code" : "introuvable" },
      { value: "codex", label: "Codex", hint: bins.codex ? "agent de code" : "introuvable" },
      ...(settings?.providers ?? []).map((p) => ({
        value: `api:${p.id}`,
        label: p.name,
        hint: p.kind === "openai" && p.baseUrl.includes("localhost") ? "local" : p.hasKey ? "API" : "clé manquante",
      })),
    ],
    [settings, bins],
  );

  const lastLen = conv?.messages.at(-1)?.parts.length ?? 0;
  const lastText = conv?.messages.at(-1) ? msgText(conv.messages.at(-1)!).length : 0;
  useEffect(() => {
    const el = scroller.current;
    if (el && el.scrollHeight - el.scrollTop - el.clientHeight < 160) el.scrollTop = el.scrollHeight;
  }, [conv?.messages.length, lastLen, lastText]);

  useEffect(() => {
    scroller.current?.scrollTo({ top: scroller.current.scrollHeight });
    ta.current?.focus();
  }, [activeId]);

  useEffect(() => {
    const t = ta.current;
    if (!t) return;
    t.style.height = "auto";
    t.style.height = Math.min(t.scrollHeight, 180) + "px";
  }, [draft]);

  if (!conv) return null;
  const running = !!conv.runId;
  const cli = isCli(conv.provider);

  const submit = () => {
    const text = draft.trim();
    if (!text || running) return;
    setDraft("");
    send(text);
  };

  const pickFiles = async () => {
    const files = await open({ multiple: true, directory: false });
    if (Array.isArray(files)) setPendingAttach([...new Set([...attach, ...files])]);
  };

  const preview = (url: string) => {
    setPreview({ url, cwd: conv.cwd, mode: "web" });
    setTab("preview");
  };

  return (
    <div className="chat">
      <div className="view-head">
        <ConvMenu conv={conv} />
      </div>
      <div className="chat-ctx">
        <select
          className="select"
          value={conv.provider}
          onChange={(e) => updateConv(conv.id, (c) => ({ ...c, provider: e.target.value, sessionId: undefined, model: undefined }))}
        >
          {providers.map((p) => (
            <option key={p.value} value={p.value}>
              {p.label} ({p.hint})
            </option>
          ))}
        </select>
        <ProjectPicker
          value={isOrch(conv) && cli ? undefined : conv.cwd}
          placeholder={cli ? "Choisir un projet" : "Sans projet"}
          allLabel={cli ? "Tous mes projets" : undefined}
          onChange={(cwd) =>
            updateConv(conv.id, (c) => ({ ...c, cwd: cwd || undefined, orchestrator: !cwd, sessionId: undefined }))
          }
        />
        {cli && (
          <div className="seg" role="radiogroup">
            {MODES.map((m) => (
              <button
                key={m.id}
                className={conv.mode === m.id ? "on" : ""}
                title={m.hint}
                onClick={() => updateConv(conv.id, (c) => ({ ...c, mode: m.id }))}
              >
                {m.label}
              </button>
            ))}
          </div>
        )}
      </div>

      <div className="messages" ref={scroller}>
        {conv.messages.length === 0 ? (
          <div className="empty">
            <Lody mood="idle" size={88} />
            <h2>Coucou, moi c'est Lody !</h2>
            <p>
              {cli
                ? isOrch(conv)
                  ? "Demande-moi n'importe quoi : je trouve le bon projet, je code, je gère tes PR et ta CI."
                  : conv.cwd
                  ? `Je travaille dans ${baseName(conv.cwd)} avec ${PROVIDER_LABEL[conv.provider]}. Qu'est-ce qu'on fait ?`
                  : "Choisis un projet juste au-dessus, puis dis-moi quoi coder."
                : "Pose-moi une question, je réponds avec le modèle choisi."}
            </p>
            <div className="suggestions">
              {(cli && isOrch(conv) ? LODY_SUGGESTIONS : SUGGESTIONS).map((s) => (
                <button key={s} className="chip" onClick={() => setDraft(s)}>
                  {s}
                </button>
              ))}
            </div>
          </div>
        ) : (
          conv.messages.map((m) => <Message key={m.id} m={m} onPreview={preview} />)
        )}
      </div>

      <div className="composer">
        {attach.length > 0 && (
          <div className="attach-list">
            {attach.map((a) => (
              <span key={a} className="attach" title={a}>
                <Paperclip size={11} /> {baseName(a)}
                <button onClick={() => setPendingAttach(attach.filter((x) => x !== a))}>
                  <X size={11} />
                </button>
              </span>
            ))}
          </div>
        )}
        <div className="composer-box">
          <button className="btn ghost icon" title="Joindre des fichiers" onClick={pickFiles}>
            <Paperclip size={17} />
          </button>
          <textarea
            ref={ta}
            rows={1}
            value={draft}
            placeholder={running ? "Lody travaille…" : cli ? "Demande-moi de coder quelque chose…" : "Écris ton message…"}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                submit();
              }
            }}
          />
          {running ? (
            <button className="btn danger icon" title="Arrêter" onClick={stop}>
              <Square size={14} fill="currentColor" />
            </button>
          ) : (
            <button className="btn primary icon" title="Envoyer (Entrée)" disabled={!draft.trim()} onClick={submit}>
              <ArrowUp size={17} />
            </button>
          )}
        </div>
        <div className="composer-hint">
          {conv.sessionId && cli ? "Session reprise automatiquement · " : ""}Entrée pour envoyer · Maj+Entrée pour une nouvelle ligne
        </div>
      </div>
    </div>
  );
}
