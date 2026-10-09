import { invoke } from "@tauri-apps/api/core";
import type {
  Agent,
  Approval,
  Bins,
  Download,
  GhOverview,
  GitDiff,
  HookStatus,
  McpSource,
  Project,
  RepoStatus,
  Settings,
} from "./types";

export const api = {
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<Settings>("save_settings", { settings }),
  setSecret: (key: string, value: string) => invoke<void>("set_secret", { key, value }),
  detectBins: () => invoke<Bins>("detect_bins"),
  hooksStatus: () => invoke<HookStatus>("hooks_status"),
  hooksInstall: () => invoke<HookStatus>("hooks_install"),
  hooksUninstall: () => invoke<HookStatus>("hooks_uninstall"),

  listAgents: () => invoke<Agent[]>("list_agents"),
  dismissAgent: (id: string) => invoke<void>("dismiss_agent", { id }),
  listApprovals: () => invoke<Approval[]>("list_approvals"),
  resolveApproval: (id: string, decision: "allow" | "always" | "deny" | "pass") =>
    invoke<boolean>("resolve_approval", { id, decision }),

  runCli: (args: {
    runId: string;
    provider: string;
    prompt: string;
    cwd: string;
    resume: string | null;
    mode: string;
    model: string | null;
    title: string;
    orchestrator: boolean;
  }) => invoke<void>("run_cli", { args }),
  chatApi: (args: {
    runId: string;
    providerId: string;
    messages: { role: string; content: string }[];
    model: string | null;
    title: string;
  }) => invoke<void>("chat_api", { args }),
  stopRun: (runId: string) => invoke<boolean>("stop_run", { runId }),
  listModels: (providerId: string) => invoke<string[]>("list_models", { providerId }),
  readTextFile: (path: string) => invoke<string>("read_text_file", { path }),

  listProjects: () => invoke<{ projects: Project[]; warnings: string[] }>("list_projects"),
  cloneRepo: (slug: string) => invoke<string>("clone_repo", { slug, destRoot: null }),
  claudeMcpServers: () => invoke<{ name: string; url: string; kind: string }[]>("claude_mcp_servers"),
  mcpListTools: (source: McpSource) =>
    invoke<{ name: string; description?: string }[]>("mcp_list_tools", { source }),

  gitDiff: (cwd: string) => invoke<GitDiff>("git_diff", { cwd }),
  startProcess: (id: string, cwd: string, command: string) => invoke<void>("start_process", { id, cwd, command }),
  stopProcess: (id: string) => invoke<boolean>("stop_process", { id }),
  suggestCommand: (cwd: string) => invoke<string | null>("suggest_command", { cwd }),

  openUrl: (url: string) => invoke<void>("open_url", { url }),
  openIn: (path: string, target: "code" | "cursor" | "explorer" | "terminal") =>
    invoke<void>("open_in", { path, target }),
  focusMascot: () => invoke<void>("focus_mascot"),
  recentDownloads: (limit = 10) => invoke<Download[]>("recent_downloads", { limit }),

  repoStatus: (cwd: string) => invoke<RepoStatus>("repo_status", { cwd }),
  ghOverview: (slugs: string[]) => invoke<GhOverview>("gh_overview", { slugs }),
  gitCommit: (cwd: string, message: string) => invoke<string>("git_commit", { cwd, message }),
  gitSync: (cwd: string, action: "pull" | "push" | "fetch") => invoke<string>("git_sync", { cwd, action }),
  ghPrAction: (cwd: string, number: number, action: "merge" | "checkout" | "ready" | "close") =>
    invoke<string>("gh_pr_action", { cwd, number, action }),
  ghPrCreate: (cwd: string, draft: boolean) => invoke<string>("gh_pr_create", { cwd, draft }),
  ghRunRerun: (cwd: string, id: number) => invoke<string>("gh_run_rerun", { cwd, id }),
  saveMascotPos: () => invoke<void>("save_mascot_pos"),
  resetMascotPos: () => invoke<void>("reset_mascot_pos"),
  setMascotVisible: (visible: boolean) => invoke<void>("set_mascot_visible", { visible }),
  appInfo: () => invoke<{ version: string; exe: string }>("app_info"),
  quit: () => invoke<void>("quit_app"),
};

export const errText = (e: unknown) => (typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e));
