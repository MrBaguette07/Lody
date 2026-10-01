import { create } from "zustand";
import { persist } from "zustand/middleware";
import { api, errText } from "./api";
import type {
  Agent,
  Approval,
  Bins,
  Conversation,
  HookStatus,
  Msg,
  Part,
  Project,
  RunEvent,
  Settings,
  Tab,
} from "./types";

export const uid = () => crypto.randomUUID();

export const isCli = (provider: string) => provider === "claude" || provider === "codex";

/** Lody généraliste : sans projet précis, il agit sur tous les projets. */
export const isOrch = (c: Conversation) => c.orchestrator ?? !c.cwd;

export const msgText = (m: Msg) =>
  m.parts
    .filter((p): p is Extract<Part, { type: "text" }> => p.type === "text")
    .map((p) => p.text)
    .join("\n\n");

interface PreviewState {
  cwd?: string;
  url: string;
  command: string;
  mode: "web" | "diff";
}

interface Store {
  tab: Tab;
  setTab: (t: Tab) => void;
  /** Island dépliée / épinglée. */
  open: boolean;
  pinned: boolean;
  setOpen: (o: boolean, tab?: Tab) => void;
  setPinned: (p: boolean) => void;
  /** Lance une demande : Lody généraliste, ou un agent dans un projet précis. */
  ask: (text: string, cwd?: string) => Promise<void>;
  gitCwd?: string;
  setGitCwd: (cwd: string) => void;

  convs: Record<string, Conversation>;
  order: string[];
  activeId: string | null;
  /** runId → conversation, pour retrouver un run depuis l'onglet Agents. */
  runConv: Record<string, string>;
  newConv: (partial?: Partial<Conversation>) => string;
  setActive: (id: string) => void;
  updateConv: (id: string, fn: (c: Conversation) => Conversation) => void;
  deleteConv: (id: string) => void;
  send: (text: string) => Promise<void>;
  stop: () => void;
  applyRunEvent: (ev: RunEvent) => void;
  pendingAttach: string[];
  setPendingAttach: (paths: string[]) => void;
  draft: string;
  setDraft: (t: string) => void;

  agents: Agent[];
  approvals: Approval[];
  settings: Settings | null;
  bins: Bins;
  hookStatus: HookStatus | null;
  setSettings: (s: Settings) => void;
  saveSettings: (s: Settings) => Promise<void>;
  refreshEnv: () => Promise<void>;

  projects: Project[];
  projectWarnings: string[];
  projectsLoading: boolean;
  projectsLoadedAt: number;
  loadProjects: () => Promise<void>;
  favorites: string[];
  toggleFav: (id: string) => void;
  recentCwds: string[];
  useProject: (path: string, newChat?: boolean) => void;

  preview: PreviewState;
  setPreview: (p: Partial<PreviewState>) => void;
  procLogs: string[];
  procRunning: boolean;
  pushProc: (line: string) => void;
  setProcRunning: (b: boolean) => void;

  toast: string | null;
  showToast: (t: string) => void;
}

const defaultProvider = () => "claude";

const LODY_RULES = `Tu es Lody, l'assistant de bureau de l'utilisateur (Windows). Tu peux agir dans tous ses projets listés ci-dessous, avec leurs chemins absolus (cd dans Bash si besoin).
- Quand une demande vise un projet, même par un surnom (ex. « compta »), identifie le bon projet, lis son README / CLAUDE.md pour suivre sa procédure, puis agis.
- Pour GitHub (PR, issues, CI/CD), utilise la CLI gh, déjà connectée. Ne pousse pas, ne fusionne pas et ne supprime rien sans que ce soit demandé.
- Tu pilotes aussi le PC et les applis : outils mcp__lody__* (ouvrir une appli, un lien ou une URI, musique et volume, fenêtres ouvertes, presse-papiers, capture d'écran, message dans l'island avec notify) et les connecteurs de l'utilisateur (Spotify, Gmail, ClickUp, Figma, Strava, Nodulz…). Pour une demande simple (« mets de la musique », « ouvre Discord »), agis directement, sans rien coder.
- Applis sans API : fais comme un humain, mcp__lody__screenshot pour voir l'écran, mcp__lody__click sur le bon bouton, puis une nouvelle capture pour vérifier. Ne clique jamais pour confirmer une suppression, un achat ou un envoi sans mon accord.
- Musique : trouve avec le connecteur Spotify (search) ce qui colle à la demande, ouvre son URI spotify:… avec mcp__lody__open (ça affiche la page sans lancer la lecture), attends 2 s, prends une capture et clique sur le gros bouton vert Lecture de la page, puis vérifie avec mcp__lody__now_playing que le titre a changé. Annonce le morceau avec mcp__lody__notify.
- N'annonce jamais un résultat que tu n'as pas vérifié : si la vérification échoue, dis-le.
- « Mon fichier téléchargé » désigne le plus récent du dossier Téléchargements correspondant au type demandé.
- Avant une action irréversible ou externe (écriture en base, envoi, paiement, push), explique ce que tu vas faire et attends mon accord.
- Réponds en français clair, phrases complètes (normal mode). Termine par un résumé de 1 à 3 phrases de ce que tu as fait.`;

/** Contexte donné à Lody généraliste : projets connus et derniers téléchargements. */
async function lodyContext(s: Store, full: boolean): Promise<string> {
  const downloads = await api.recentDownloads(8).catch(() => []);
  const dl = downloads
    .map((d) => `- ${d.name} (${new Date(d.modifiedAt).toLocaleString("fr-FR")}) : ${d.path}`)
    .join("\n");
  if (!full) return dl ? `[Téléchargements récents]\n${dl}\n\n` : "";
  const rank = (id: string, path: string) =>
    (s.favorites.includes(id) ? 0 : 2) + (s.recentCwds.includes(path) ? 0 : 1);
  const projects = s.projects
    .filter((p) => p.path)
    .sort((a, b) => rank(a.id, a.path!) - rank(b.id, b.path!) || (b.updatedAt ?? 0) - (a.updatedAt ?? 0))
    .slice(0, 120)
    .map((p) => `- ${p.name} : ${p.path}${p.github ? ` (GitHub ${p.github})` : ""}${p.description ? ` : ${p.description}` : ""}`)
    .join("\n");
  return `[Contexte Lody]\n${LODY_RULES}\n\nProjets :\n${projects || "(aucun)"}\n\nTéléchargements récents :\n${dl || "(aucun)"}\n[/Contexte]\n\n`;
}

export const useStore = create<Store>()(
  persist(
    (set, get) => ({
      tab: "home",
      setTab: (tab) => set({ tab, open: true }),
      open: false,
      pinned: false,
      setOpen: (open, tab) => set(tab ? { open, tab } : { open }),
      setPinned: (pinned) => set({ pinned }),
      ask: async (text, cwd) => {
        get().newConv({ cwd, orchestrator: !cwd, provider: "claude", sessionId: undefined, model: undefined });
        set({ tab: "chat", open: true });
        await get().send(text);
      },
      gitCwd: undefined,
      setGitCwd: (gitCwd) => set({ gitCwd, tab: "git", open: true }),

      convs: {},
      order: [],
      activeId: null,
      runConv: {},
      newConv: (partial) => {
        const s = get();
        const prev = s.activeId ? s.convs[s.activeId] : undefined;
        const conv: Conversation = {
          id: uid(),
          title: "Nouvelle discussion",
          provider: prev?.provider ?? defaultProvider(),
          cwd: prev?.cwd,
          orchestrator: prev ? isOrch(prev) : true,
          mode: prev?.mode ?? s.settings?.defaultMode ?? "ask",
          model: prev?.model,
          messages: [],
          updatedAt: Date.now(),
          ...partial,
        };
        set({ convs: { ...s.convs, [conv.id]: conv }, order: [conv.id, ...s.order], activeId: conv.id });
        return conv.id;
      },
      setActive: (id) => set({ activeId: id }),
      updateConv: (id, fn) =>
        set((s) => (s.convs[id] ? { convs: { ...s.convs, [id]: fn(s.convs[id]) } } : {})),
      deleteConv: (id) =>
        set((s) => {
          const convs = { ...s.convs };
          delete convs[id];
          const order = s.order.filter((x) => x !== id);
          return { convs, order, activeId: s.activeId === id ? order[0] ?? null : s.activeId };
        }),

      pendingAttach: [],
      setPendingAttach: (pendingAttach) => set({ pendingAttach }),
      draft: "",
      setDraft: (draft) => set({ draft }),

      send: async (text) => {
        let s = get();
        let id = s.activeId && s.convs[s.activeId] ? s.activeId : s.newConv();
        const conv = get().convs[id];
        if (conv.runId) return;
        const attachments = get().pendingAttach;
        const runId = uid();
        const title = conv.messages.length === 0 ? text.replace(/\s+/g, " ").slice(0, 60) : conv.title;

        let apiText = text;
        if (!isCli(conv.provider) && attachments.length) {
          const blocks = await Promise.all(
            attachments.map(async (p) => {
              try {
                return `\n\n--- ${p} ---\n\`\`\`\n${await api.readTextFile(p)}\n\`\`\``;
              } catch (e) {
                return `\n\n(${p} : ${errText(e)})`;
              }
            }),
          );
          apiText += blocks.join("");
        }
        const userMsg: Msg = {
          id: uid(),
          role: "user",
          parts: [{ type: "text", text }],
          apiText,
          attachments,
          createdAt: Date.now(),
        };
        const asst: Msg = { id: uid(), role: "assistant", parts: [], pending: true, createdAt: Date.now() };
        get().updateConv(id, (c) => ({
          ...c,
          title,
          runId,
          messages: [...c.messages, userMsg, asst],
          updatedAt: Date.now(),
        }));
        set((st) => ({
          pendingAttach: [],
          runConv: { ...st.runConv, [runId]: id },
          order: [id, ...st.order.filter((x) => x !== id)],
        }));

        try {
          if (isCli(conv.provider)) {
            if (!conv.cwd && !isOrch(conv))
              throw new Error("Choisis d'abord un projet : c'est le dossier où l'agent va travailler.");
            let prompt = attachments.length
              ? `${text}\n\nFichiers joints :\n${attachments.map((a) => `- ${a}`).join("\n")}`
              : text;
            if (isOrch(conv)) prompt = `${await lodyContext(get(), !conv.sessionId)}${prompt}`;
            await api.runCli({
              runId,
              provider: conv.provider,
              prompt,
              cwd: isOrch(conv) ? "" : conv.cwd ?? "",
              resume: conv.sessionId ?? null,
              mode: conv.mode,
              model: conv.model ?? null,
              title,
              orchestrator: isOrch(conv),
            });
          } else {
            const history = [...conv.messages, userMsg]
              .filter((m) => !m.pending && !m.error)
              .map((m) => ({ role: m.role, content: m.apiText ?? msgText(m) }))
              .filter((m) => m.content.trim());
            await api.chatApi({
              runId,
              providerId: conv.provider.replace(/^api:/, ""),
              messages: history,
              model: conv.model ?? null,
              title,
            });
          }
        } catch (e) {
          get().applyRunEvent({ runId, kind: "error", message: errText(e) });
          get().applyRunEvent({ runId, kind: "exit" });
        }
      },

      stop: () => {
        const s = get();
        const conv = s.activeId ? s.convs[s.activeId] : undefined;
        if (conv?.runId) api.stopRun(conv.runId);
      },

      applyRunEvent: (ev) => {
        const s = get();
        const convId = s.runConv[ev.runId];
        const conv = convId ? s.convs[convId] : undefined;
        if (!conv || conv.runId !== ev.runId) return;
        get().updateConv(conv.id, (c) => {
          if (ev.kind === "session") return { ...c, sessionId: ev.sessionId ?? c.sessionId };
          const msgs = [...c.messages];
          const i = msgs.length - 1;
          if (i < 0 || msgs[i].role !== "assistant") return c;
          const m: Msg = { ...msgs[i], parts: [...msgs[i].parts] };
          const n = m.parts.length;
          const last = m.parts[n - 1];
          const closeText = () => {
            if (last?.type === "text" && last.streaming) m.parts[n - 1] = { ...last, streaming: false };
          };
          let next: Conversation = c;
          switch (ev.kind) {
            case "text_delta":
              if (last?.type === "text" && last.streaming) m.parts[n - 1] = { ...last, text: last.text + ev.text };
              else m.parts.push({ type: "text", text: ev.text ?? "", streaming: true });
              break;
            case "text":
              if (last?.type === "text" && last.streaming) m.parts[n - 1] = { type: "text", text: ev.text };
              else if (!(last?.type === "text" && last.text === ev.text)) m.parts.push({ type: "text", text: ev.text });
              break;
            case "tool":
              closeText();
              m.parts.push({
                type: "tool",
                id: String(ev.id ?? uid()),
                name: ev.name ?? "outil",
                detail: ev.detail ?? ev.name ?? "",
                input: ev.input,
                status: "run",
                sub: !!ev.sub,
              });
              break;
            case "tool_result": {
              const j = m.parts.findIndex((p) => p.type === "tool" && p.id === String(ev.id));
              if (j >= 0) {
                const t = m.parts[j] as Extract<Part, { type: "tool" }>;
                m.parts[j] = { ...t, status: ev.isError ? "err" : "ok", output: ev.output };
              }
              break;
            }
            case "done":
              closeText();
              m.meta = { costUsd: ev.costUsd ?? undefined, durationMs: ev.durationMs ?? undefined };
              if (ev.isError) m.error = ev.result || "L'agent a rencontré une erreur.";
              else if (ev.result && !m.parts.some((p) => p.type === "text")) m.parts.push({ type: "text", text: ev.result });
              if (ev.sessionId) next = { ...next, sessionId: ev.sessionId };
              break;
            case "error":
              closeText();
              m.error = ev.message || "Erreur inconnue";
              break;
            case "stopped":
              closeText();
              m.error = m.error ?? "Arrêté.";
              break;
            case "exit":
              closeText();
              m.pending = false;
              m.parts = m.parts.map((p) =>
                p.type === "tool" && p.status === "run" ? { ...p, status: m.error ? "err" : "ok" } : p,
              );
              next = { ...next, runId: null };
              break;
          }
          msgs[i] = m;
          return { ...next, messages: msgs, updatedAt: Date.now() };
        });
      },

      agents: [],
      approvals: [],
      settings: null,
      bins: { claude: null, codex: null },
      hookStatus: null,
      setSettings: (settings) => set({ settings }),
      saveSettings: async (s) => {
        set({ settings: s });
        try {
          set({ settings: await api.saveSettings(s) });
        } catch (e) {
          get().showToast(errText(e));
        }
      },
      refreshEnv: async () => {
        const [settings, bins, hookStatus] = await Promise.all([
          api.getSettings(),
          api.detectBins(),
          api.hooksStatus(),
        ]);
        set({ settings, bins, hookStatus });
      },

      projects: [],
      projectWarnings: [],
      projectsLoading: false,
      projectsLoadedAt: 0,
      loadProjects: async () => {
        if (get().projectsLoading) return;
        set({ projectsLoading: true });
        try {
          const r = await api.listProjects();
          set({ projects: r.projects, projectWarnings: r.warnings, projectsLoadedAt: Date.now() });
        } catch (e) {
          set({ projectWarnings: [errText(e)] });
        } finally {
          set({ projectsLoading: false });
        }
      },
      favorites: [],
      toggleFav: (id) =>
        set((s) => ({
          favorites: s.favorites.includes(id) ? s.favorites.filter((x) => x !== id) : [id, ...s.favorites],
        })),
      recentCwds: [],
      useProject: (path, newChat = false) => {
        const s = get();
        set({ recentCwds: [path, ...s.recentCwds.filter((p) => p !== path)].slice(0, 8), tab: "chat", open: true });
        const active = s.activeId ? s.convs[s.activeId] : undefined;
        if (newChat || !active || active.messages.length > 0) {
          get().newConv({ cwd: path, orchestrator: false, sessionId: undefined, title: "Nouvelle discussion", messages: [] });
        } else {
          get().updateConv(active.id, (c) => ({ ...c, cwd: path, orchestrator: false, sessionId: undefined }));
        }
      },

      preview: { url: "", command: "", mode: "web" },
      setPreview: (p) => set((s) => ({ preview: { ...s.preview, ...p } })),
      procLogs: [],
      procRunning: false,
      pushProc: (line) => set((s) => ({ procLogs: [...s.procLogs.slice(-499), line] })),
      setProcRunning: (procRunning) => set({ procRunning }),

      toast: null,
      showToast: (toast) => {
        set({ toast });
        setTimeout(() => get().toast === toast && set({ toast: null }), 5000);
      },
    }),
    {
      name: "lody-panel",
      version: 1,
      partialize: (s) => ({
        tab: s.tab,
        pinned: s.pinned,
        gitCwd: s.gitCwd,
        convs: Object.fromEntries(
          s.order.slice(0, 60).flatMap((id) => {
            const c = s.convs[id];
            if (!c) return [];
            // Pas de run en cours après redémarrage ; sorties d'outils allégées.
            const messages = c.messages.slice(-200).map((m) => ({
              ...m,
              pending: false,
              parts: m.parts.map((p) => (p.type === "tool" ? { ...p, output: p.output?.slice(0, 1500), input: undefined } : p)),
            }));
            return [[id, { ...c, runId: null, messages }]];
          }),
        ),
        order: s.order.slice(0, 60),
        activeId: s.activeId,
        favorites: s.favorites,
        recentCwds: s.recentCwds,
        preview: s.preview,
      }),
    },
  ),
);
