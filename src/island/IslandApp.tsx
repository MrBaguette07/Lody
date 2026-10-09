import { useEffect, useMemo, useRef, useState, type PointerEvent } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import {
  Bot,
  ChevronUp,
  Eye,
  FolderGit2,
  GitBranch,
  House,
  MessageCircle,
  Pin,
  PinOff,
  Settings as Cog,
} from "lucide-react";
import Lody, { type Mood } from "../components/Lody";
import ApprovalCard from "../components/ApprovalCard";
import { api } from "../lib/api";
import { useStore } from "../lib/store";
import { sfx } from "../lib/sounds";
import { dialogOpen } from "../lib/dialog";
import { isActive, stripAnsi } from "../lib/format";
import type { Agent, Approval, RunEvent, Settings, Tab } from "../lib/types";
import HomeView from "../panel/HomeView";
import ChatView from "../panel/ChatView";
import AgentsView from "../panel/AgentsView";
import ProjectsView from "../panel/ProjectsView";
import GitView from "../panel/GitView";
import PreviewView from "../panel/PreviewView";
import SettingsView from "../panel/SettingsView";
import "../panel/panel.css";
import "./island.css";

const TABS: { id: Tab; label: string; icon: typeof Bot }[] = [
  { id: "home", label: "Accueil", icon: House },
  { id: "chat", label: "Chat", icon: MessageCircle },
  { id: "agents", label: "Agents", icon: Bot },
  { id: "projects", label: "Projets", icon: FolderGit2 },
  { id: "git", label: "Git", icon: GitBranch },
  { id: "preview", label: "Aperçu", icon: Eye },
  { id: "settings", label: "Réglages", icon: Cog },
];

const GREETINGS = [
  "Coucou ! On fait quoi ?",
  "Clique pour me parler",
  "Glisse un fichier sur moi",
  "Je surveille tes agents 👀",
  "Ctrl+Alt+L pour m'ouvrir",
];

const PRIORITY: Record<string, number> = { waiting: 0, working: 1, thinking: 2 };

interface Flash {
  title: string;
  sub: string;
  mood: Mood;
  until: number;
}

export default function IslandApp() {
  const win = useMemo(() => getCurrentWindow(), []);
  const agents = useStore((s) => s.agents);
  const approvals = useStore((s) => s.approvals);
  const settings = useStore((s) => s.settings);
  const open = useStore((s) => s.open);
  const pinned = useStore((s) => s.pinned);
  const tab = useStore((s) => s.tab);
  const toast = useStore((s) => s.toast);
  const { setOpen, setPinned, setTab } = useStore.getState();

  const [hover, setHover] = useState(false);
  const [dropping, setDropping] = useState(false);
  const [look, setLook] = useState({ x: 0, y: 0 });
  const [flash, setFlash] = useState<Flash | null>(null);
  const [greeting, setGreeting] = useState(GREETINGS[0]);
  const [lastSeen, setLastSeen] = useState(Date.now());
  const [now, setNow] = useState(Date.now());

  const lodyRef = useRef<HTMLDivElement>(null);
  const ignoring = useRef<boolean | null>(null);
  const dragging = useRef(false);
  const down = useRef<{ x: number; y: number } | null>(null);
  const prevStatus = useRef<Record<string, string>>({});
  const prevApprovals = useRef(0);
  const saveTimer = useRef<number | undefined>(undefined);
  const openRef = useRef(open);
  openRef.current = open;

  const play = (k: keyof typeof sfx) => (useStore.getState().settings?.sounds ?? true) && sfx[k]();

  const expand = (t?: Tab) => {
    setOpen(true, t);
    win.setIgnoreCursorEvents(false);
    ignoring.current = false;
    win.setFocus();
  };
  const collapse = () => setOpen(false);

  // Données et événements du backend
  useEffect(() => {
    const st = useStore.getState;
    st().refreshEnv().catch(() => {});
    api.listAgents().then((agents) => useStore.setState({ agents }));
    api.listApprovals().then((a) => {
      prevApprovals.current = a.length;
      useStore.setState({ approvals: a });
    });
    st().loadProjects();

    const un = [
      listen<RunEvent>("run", (e) => st().applyRunEvent(e.payload)),
      listen<Agent[]>("agents", (e) => {
        const list = e.payload;
        for (const a of list) {
          const before = prevStatus.current[a.id];
          if (before && isActive(before) && a.status === "done") {
            setFlash({ title: "Terminé ✨", sub: a.title || a.project || "", mood: "happy", until: Date.now() + 6000 });
            play("done");
          } else if (before && before !== "error" && a.status === "error") {
            setFlash({ title: "Oups, une erreur", sub: a.project ?? a.title, mood: "error", until: Date.now() + 7000 });
            play("error");
          }
        }
        prevStatus.current = Object.fromEntries(list.map((a) => [a.id, a.status]));
        useStore.setState({ agents: list });
        setLastSeen(Date.now());
      }),
      listen<Approval[]>("approvals", (e) => {
        if (e.payload.length > prevApprovals.current) play("alert");
        prevApprovals.current = e.payload.length;
        useStore.setState({ approvals: e.payload });
        setLastSeen(Date.now());
      }),
      listen<Settings>("settings", (e) => st().setSettings(e.payload)),
      listen<string>("toast", (e) => st().showToast(e.payload)),
      // Message d'un agent via l'outil lody.notify
      listen<{ title?: string; message: string }>("say", (e) => {
        setFlash({ title: e.payload.title || "Lody", sub: e.payload.message, mood: "happy", until: Date.now() + 7000 });
        play("pop");
      }),
      listen<{ open?: boolean; toggle?: boolean; tab?: Tab | null }>("island", (e) => {
        const p = e.payload;
        if (p.toggle) {
          if (openRef.current) collapse();
          else expand();
        } else if (p.open === false) collapse();
        else expand(p.tab ?? undefined);
      }),
      listen<{ id: string; line?: string; exit?: number | null }>("proc", (e) => {
        if (e.payload.id !== "preview") return;
        if (e.payload.line !== undefined) st().pushProc(stripAnsi(e.payload.line));
        if ("exit" in e.payload) {
          st().setProcRunning(false);
          st().pushProc(`[processus terminé, code ${e.payload.exit ?? "?"}]`);
        }
      }),
      listen<{ x: number; y: number }>("cursor", (e) => onCursor(e.payload.x, e.payload.y)),
      win.onMoved(() => {
        if (!dragging.current) return;
        clearTimeout(saveTimer.current);
        saveTimer.current = window.setTimeout(() => {
          dragging.current = false;
          api.saveMascotPos();
        }, 600);
      }),
      // Clic ailleurs : l'island se replie (sauf épinglée ou sélecteur de fichier ouvert).
      win.onFocusChanged(({ payload: focused }) => {
        if (focused) return;
        setTimeout(() => {
          const s = useStore.getState();
          if (!document.hasFocus() && !s.pinned && !dialogOpen()) collapse();
        }, 180);
      }),
      getCurrentWebview().onDragDropEvent((e) => {
        const p = e.payload;
        if (p.type === "enter" || p.type === "over") setDropping(true);
        else if (p.type === "leave") setDropping(false);
        else if (p.type === "drop") {
          setDropping(false);
          play("pop");
          st().setPendingAttach([...new Set([...st().pendingAttach, ...p.paths])]);
          expand("chat");
        }
      }),
    ];
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && openRef.current && !document.querySelector(".popover")) collapse();
    };
    window.addEventListener("keydown", onKey);
    const tick = window.setInterval(() => setNow(Date.now()), 1000);
    return () => {
      un.forEach((p) => p.then((f) => f()));
      window.removeEventListener("keydown", onKey);
      clearInterval(tick);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Regard + zones cliquables : le reste de la fenêtre laisse passer la souris.
  function onCursor(x: number, y: number) {
    const el = lodyRef.current;
    if (el) {
      const r = el.getBoundingClientRect();
      const dx = x - (r.left + r.width / 2);
      const dy = y - (r.top + r.height / 2);
      const dist = Math.hypot(dx, dy) || 1;
      const k = Math.min(1, dist / 240);
      setLook({ x: (dx / dist) * k, y: (dy / dist) * k });
      if (dist < 260) setLastSeen(Date.now());
    }
    const inside = [...document.querySelectorAll("[data-hit]")].some((n) => {
      const r = n.getBoundingClientRect();
      return x >= r.left - 4 && x <= r.right + 4 && y >= r.top - 4 && y <= r.bottom + 4;
    });
    if (!dragging.current && ignoring.current !== !inside) {
      ignoring.current = !inside;
      win.setIgnoreCursorEvents(!inside);
    }
    setHover((h) => {
      if (inside && !h) setGreeting(GREETINGS[Math.floor(Math.random() * GREETINGS.length)]);
      return inside;
    });
  }

  const activeFlash = flash && flash.until > now ? flash : null;
  const active = agents.filter((a) => isActive(a.status)).sort((a, b) => PRIORITY[a.status] - PRIORITY[b.status]);
  const focus = active[0];
  const asleep = !open && !active.length && !approvals.length && !activeFlash && now - lastSeen > 4 * 60_000;

  const mood: Mood = approvals.length
    ? "alert"
    : activeFlash
      ? activeFlash.mood
      : dropping
        ? "curious"
        : focus?.status === "waiting"
          ? "alert"
          : focus?.status === "working"
            ? "work"
            : focus?.status === "thinking"
              ? "think"
              : asleep
                ? "sleep"
                : hover || open
                  ? "curious"
                  : "idle";

  let line1 = "";
  let line2 = "";
  if (approvals.length) {
    line1 = approvals.length > 1 ? `${approvals.length} demandes d'accord` : "J'ai besoin de ton accord";
    line2 = approvals[0].project ?? approvals[0].detail;
  } else if (dropping) {
    line1 = "Lâche, je prends !";
    line2 = "Le fichier ira dans le chat";
  } else if (activeFlash) {
    line1 = activeFlash.title;
    line2 = activeFlash.sub;
  } else if (focus) {
    line1 = focus.project ?? focus.title;
    line2 = focus.activity || focus.title;
  } else if (open) {
    line1 = "Lody";
    line2 = "Prête à t'aider";
  } else if (hover) {
    line1 = greeting;
    const done = agents.filter((a) => a.status === "done").length;
    line2 = done ? `${done} tâche${done > 1 ? "s" : ""} terminée${done > 1 ? "s" : ""}` : "Clique pour ouvrir";
  }
  const expandedPill = !!line1;
  const bottom = settings?.mascotPosition === "bottom";

  const onPointerDown = (e: PointerEvent) => {
    if (e.button !== 0) return;
    down.current = { x: e.screenX, y: e.screenY };
    dragging.current = false;
  };
  const onPointerMove = (e: PointerEvent) => {
    if (!down.current || !(e.buttons & 1) || dragging.current) return;
    if (Math.hypot(e.screenX - down.current.x, e.screenY - down.current.y) > 4) {
      dragging.current = true;
      down.current = null;
      win.startDragging();
    }
  };
  const onPointerUp = () => {
    if (down.current && !dragging.current) {
      play("pop");
      if (open) collapse();
      else expand(approvals.length ? "agents" : undefined);
    }
    down.current = null;
  };

  const runningCount = active.length;

  return (
    <div className={`stage ${bottom ? "bottom" : "top"}`}>
      <div className={`island ${open ? "open" : ""} ${expandedPill || open ? "wide" : ""} mood-${mood}`} data-hit>
        <div className="island-bar">
          <div
            className="island-grip"
            onPointerDown={onPointerDown}
            onPointerMove={onPointerMove}
            onPointerUp={onPointerUp}
            title={open ? "Replier (Échap)" : "Ouvrir Lody"}
          >
            <div ref={lodyRef} className="pill-lody">
              <Lody mood={mood} look={look} size={open ? 40 : 52} />
              {runningCount > 1 && <span className="pill-badge">{runningCount}</span>}
            </div>
            <div className="pill-text">
              <strong>{line1}</strong>
              <span>{line2}</span>
            </div>
          </div>
          {open && (
            <>
              <nav className="island-tabs">
                {TABS.map((t) => {
                  const badge = t.id === "agents" ? approvals.length || runningCount : 0;
                  return (
                    <button
                      key={t.id}
                      className={`island-tab ${tab === t.id ? "active" : ""}`}
                      onClick={() => setTab(t.id)}
                      title={t.label}
                    >
                      <t.icon size={17} />
                      <span>{t.label}</span>
                      {badge > 0 && <i className={approvals.length ? "warn" : ""}>{badge}</i>}
                    </button>
                  );
                })}
              </nav>
              <button
                className={`btn icon sm ${pinned ? "soft" : "ghost"}`}
                title={pinned ? "Désépingler" : "Garder ouvert"}
                onClick={() => setPinned(!pinned)}
              >
                {pinned ? <Pin size={14} /> : <PinOff size={14} />}
              </button>
              <button className="btn ghost icon sm" title="Replier (Échap)" onClick={collapse}>
                <ChevronUp size={16} />
              </button>
            </>
          )}
        </div>

        {open && (
          <div className="island-body">
            {approvals.length > 0 && tab !== "agents" && (
              <div className="island-approvals">
                <ApprovalCard key={approvals[0].id} approval={approvals[0]} compact index={0} total={approvals.length} />
              </div>
            )}
            <div className="island-view">
              {tab === "home" && <HomeView />}
              {tab === "chat" && <ChatView />}
              {tab === "agents" && <AgentsView />}
              {tab === "projects" && <ProjectsView />}
              {tab === "git" && <GitView />}
              {tab === "preview" && <PreviewView />}
              {tab === "settings" && <SettingsView />}
            </div>
          </div>
        )}
      </div>

      {!open && approvals.length > 0 && (
        <ApprovalCard key={approvals[0].id} approval={approvals[0]} compact index={0} total={approvals.length} />
      )}
      {toast && (
        <div className="toast" data-hit onClick={() => useStore.setState({ toast: null })}>
          {toast}
        </div>
      )}
    </div>
  );
}
