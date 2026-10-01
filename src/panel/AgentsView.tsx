import { useState } from "react";
import { Code2, FileDiff, MessageCircle, PlugZap, Square, X } from "lucide-react";
import Lody from "../components/Lody";
import ApprovalCard from "../components/ApprovalCard";
import { PROVIDER_LABEL } from "../components/icons";
import { api, errText } from "../lib/api";
import { useStore } from "../lib/store";
import { STATUS_LABEL, ago, duration, isActive } from "../lib/format";
import type { Agent } from "../lib/types";

function AgentCard({ a }: { a: Agent }) {
  const [expanded, setExpanded] = useState(false);
  const runConv = useStore((s) => s.runConv);
  const { setActive, setTab, setPreview } = useStore.getState();
  const convId = a.runId ? runConv[a.runId] : undefined;
  const running = isActive(a.status);
  return (
    <div className={`agent st-${a.status}`}>
      <div className="agent-top">
        <span className="dot" />
        <span className="agent-status">{STATUS_LABEL[a.status] ?? a.status}</span>
        <span className="tag">{PROVIDER_LABEL[a.provider] ?? a.provider}</span>
        <span className="tag faint">{a.source === "hook" ? "Terminal / IDE" : "Lody"}</span>
        <span className="agent-time">{running ? duration(Date.now() - a.startedAt) : ago(a.updatedAt)}</span>
      </div>
      <div className="agent-title">{a.title || "Session"}</div>
      <div className="agent-line">
        {a.project && <strong>{a.project}</strong>}
        {a.activity && <span>{a.activity}</span>}
      </div>
      {(a.subagents > 0 || a.tools > 0) && (
        <div className="agent-stats">
          {a.tools > 0 && <span>{a.tools} actions</span>}
          {a.subagents > 0 && <span>{a.subagents} sous-agent{a.subagents > 1 ? "s" : ""} actif{a.subagents > 1 ? "s" : ""}</span>}
        </div>
      )}
      {a.lastMessage && (
        <button className={`agent-msg ${expanded ? "open" : ""}`} onClick={() => setExpanded((e) => !e)}>
          {a.lastMessage}
        </button>
      )}
      <div className="agent-actions">
        {convId && (
          <button
            className="btn sm soft"
            onClick={() => {
              setActive(convId);
              setTab("chat");
            }}
          >
            <MessageCircle size={13} /> Conversation
          </button>
        )}
        {a.cwd && (
          <>
            <button className="btn sm" onClick={() => api.openIn(a.cwd!, "code")}>
              <Code2 size={13} /> VS Code
            </button>
            <button
              className="btn sm"
              onClick={() => {
                setPreview({ cwd: a.cwd!, mode: "diff" });
                setTab("preview");
              }}
            >
              <FileDiff size={13} /> Changements
            </button>
          </>
        )}
        <span className="grow" />
        {a.source === "lody" && running && (
          <button className="btn sm danger" onClick={() => api.stopRun(a.id)}>
            <Square size={11} fill="currentColor" /> Stop
          </button>
        )}
        {!running && (
          <button className="btn sm ghost icon" title="Retirer de la liste" onClick={() => api.dismissAgent(a.id)}>
            <X size={14} />
          </button>
        )}
      </div>
    </div>
  );
}

export default function AgentsView() {
  const agents = useStore((s) => s.agents);
  const approvals = useStore((s) => s.approvals);
  const hookStatus = useStore((s) => s.hookStatus);
  const showToast = useStore((s) => s.showToast);
  const [filter, setFilter] = useState<"active" | "all">("all");
  const [busy, setBusy] = useState(false);

  const list = filter === "active" ? agents.filter((a) => isActive(a.status)) : agents;
  const running = agents.filter((a) => isActive(a.status)).length;

  const install = async () => {
    setBusy(true);
    try {
      useStore.setState({ hookStatus: await api.hooksInstall() });
      showToast("Hooks installés ✨ Les nouvelles sessions Claude Code apparaîtront ici.");
    } catch (e) {
      showToast(errText(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="view">
      <div className="view-head">
        <h1>Agents</h1>
        <span className="muted">{running ? `${running} en cours` : "aucun en cours"}</span>
        <span className="grow" />
        <div className="seg">
          <button className={filter === "all" ? "on" : ""} onClick={() => setFilter("all")}>
            Tous
          </button>
          <button className={filter === "active" ? "on" : ""} onClick={() => setFilter("active")}>
            Actifs
          </button>
        </div>
      </div>
      <div className="scroll">
        {hookStatus && !hookStatus.upToDate && (
          <div className="banner">
            <PlugZap size={18} />
            <div>
              <strong>{hookStatus.installed ? "Hooks à mettre à jour" : "Branche Lody sur Claude Code"}</strong>
              <p>
                Pour voir ici toutes tes sessions Claude Code (terminal, VS Code…) et valider leurs actions depuis la
                mascotte.
              </p>
            </div>
            <button className="btn primary sm" disabled={busy} onClick={install}>
              {hookStatus.installed ? "Mettre à jour" : "Installer"}
            </button>
          </div>
        )}
        {approvals.map((a, i) => (
          <ApprovalCard key={a.id} approval={a} index={i} total={approvals.length} />
        ))}
        {list.map((a) => (
          <AgentCard key={a.id} a={a} />
        ))}
        {!list.length && (
          <div className="empty">
            <Lody mood="sleep" size={72} />
            <h2>Calme plat</h2>
            <p>Lance une tâche depuis le chat, ou ouvre Claude Code dans un terminal : je la suivrai ici.</p>
          </div>
        )}
      </div>
    </div>
  );
}
