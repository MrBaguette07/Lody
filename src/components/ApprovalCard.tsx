import { useState } from "react";
import { Check, CheckCheck, SquareTerminal, X } from "lucide-react";
import { api } from "../lib/api";
import type { Approval } from "../lib/types";
import { toolIcon } from "./icons";

function preview(a: Approval): { label?: string; code?: string } {
  const i = (a.input ?? {}) as Record<string, any>;
  const lines = (s: string, n: number) => {
    const all = String(s ?? "").split("\n");
    return all.slice(0, n).join("\n") + (all.length > n ? `\n… (+${all.length - n} lignes)` : "");
  };
  switch (a.toolName) {
    case "Bash":
    case "PowerShell":
      return { label: i.description, code: lines(i.command, 8) };
    case "Write":
      return { label: i.file_path, code: lines(i.content, 8) };
    case "Edit":
      return {
        label: i.file_path,
        code: `${lines(i.old_string, 4).replace(/^/gm, "- ")}\n${lines(i.new_string, 4).replace(/^/gm, "+ ")}`,
      };
    case "MultiEdit":
      return { label: `${i.file_path} · ${(i.edits ?? []).length} modifications` };
    case "WebFetch":
      return { label: i.url, code: i.prompt };
    default:
      return { code: lines(JSON.stringify(i, null, 2), 8) };
  }
}

export default function ApprovalCard({
  approval,
  compact = false,
  index,
  total,
}: {
  approval: Approval;
  compact?: boolean;
  index?: number;
  total?: number;
}) {
  const [busy, setBusy] = useState(false);
  const Icon = toolIcon(approval.toolName);
  const p = preview(approval);
  const decide = async (d: "allow" | "always" | "deny" | "pass") => {
    setBusy(true);
    await api.resolveApproval(approval.id, d);
  };
  return (
    <div className={`approval ${compact ? "compact" : ""}`} data-hit>
      <div className="approval-head">
        <span className="approval-icon">
          <Icon size={15} />
        </span>
        <div className="approval-title">
          <strong>{approval.toolName}</strong>
          <span>{approval.project ?? "Claude Code"} veut ton accord</span>
        </div>
        {total && total > 1 ? <span className="approval-count">{(index ?? 0) + 1}/{total}</span> : null}
      </div>
      {p.label && <div className="approval-label">{p.label}</div>}
      {p.code && <pre className="approval-code">{p.code}</pre>}
      <div className="approval-actions">
        <button className="btn ok" disabled={busy} onClick={() => decide("allow")}>
          <Check size={14} /> Autoriser
        </button>
        <button className="btn soft" disabled={busy} onClick={() => decide("always")} title="Autoriser cet outil pour toute la session">
          <CheckCheck size={14} /> Toujours
        </button>
        <button className="btn danger" disabled={busy} onClick={() => decide("deny")}>
          <X size={14} /> Refuser
        </button>
        <button className="btn ghost icon" disabled={busy} onClick={() => decide("pass")} title="Laisser Claude Code demander dans le terminal">
          <SquareTerminal size={14} />
        </button>
      </div>
    </div>
  );
}
