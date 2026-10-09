export type AgentStatus = "idle" | "thinking" | "working" | "waiting" | "done" | "error" | "ended";
export type Mode = "ask" | "edits" | "yolo";
export type Tab = "home" | "chat" | "agents" | "projects" | "git" | "preview" | "settings";

export interface Agent {
  id: string;
  source: "hook" | "lody";
  provider: string;
  title: string;
  cwd?: string | null;
  project?: string | null;
  status: AgentStatus;
  activity: string;
  lastMessage: string;
  startedAt: number;
  updatedAt: number;
  subagents: number;
  tools: number;
  runId?: string | null;
  sessionId?: string | null;
  transcript?: string | null;
}

export interface Approval {
  id: string;
  agentKey: string;
  sessionId: string;
  toolUseId: string;
  toolName: string;
  detail: string;
  input: Record<string, unknown> | null;
  cwd?: string | null;
  project?: string | null;
  createdAt: number;
}

export interface ApiProvider {
  id: string;
  name: string;
  kind: "openai" | "anthropic";
  baseUrl: string;
  model: string;
  hasKey: boolean;
}

export interface CustomSource {
  id: string;
  name: string;
  kind: "folder" | "command" | "http";
  value: string;
  enabled: boolean;
}

export interface McpSource {
  id: string;
  name: string;
  server: string;
  url: string;
  tool: string;
  args: Record<string, unknown> | null;
  linkTemplate: string;
  enabled: boolean;
}

export interface Settings {
  hookPort: number;
  claudePath: string;
  codexPath: string;
  defaultMode: Mode;
  approvalTimeoutSecs: number;
  providers: ApiProvider[];
  systemPrompt: string;
  projectRoots: string[];
  sources: { folders: boolean; vscode: boolean; cursor: boolean; github: boolean };
  customSources: CustomSource[];
  mcpSources: McpSource[];
  sounds: boolean;
  mascotPosition: "top" | "bottom";
  mascotX?: number | null;
  mascotY?: number | null;
  shortcut: string;
}

export interface Project {
  id: string;
  name: string;
  path?: string | null;
  url?: string | null;
  sources: string[];
  description?: string | null;
  updatedAt?: number | null;
  kind: "local" | "remote" | "item";
  github?: string | null;
  private: boolean;
}

export interface HookStatus {
  installed: boolean;
  upToDate: boolean;
  settingsPath: string;
}

export interface Bins {
  claude: string | null;
  codex: string | null;
}

export interface GitDiff {
  isRepo: boolean;
  branch: string;
  files: { status: string; path: string }[];
  diff: string;
  truncated: boolean;
}

export interface RunEvent {
  runId: string;
  kind: "session" | "text_delta" | "text" | "tool" | "tool_result" | "done" | "error" | "stopped" | "exit";
  [key: string]: any;
}

export type Part =
  | { type: "text"; text: string; streaming?: boolean }
  | {
      type: "tool";
      id: string;
      name: string;
      detail: string;
      status: "run" | "ok" | "err";
      output?: string;
      input?: string;
      sub?: boolean;
    };

export interface Msg {
  id: string;
  role: "user" | "assistant";
  parts: Part[];
  /** Texte envoyé aux API (avec le contenu des fichiers joints). */
  apiText?: string;
  attachments?: string[];
  pending?: boolean;
  error?: string;
  meta?: { costUsd?: number; durationMs?: number };
  createdAt: number;
}

export interface Conversation {
  id: string;
  title: string;
  /** "claude" | "codex" | "api:<id>" */
  provider: string;
  cwd?: string;
  sessionId?: string;
  mode: Mode;
  model?: string;
  messages: Msg[];
  runId?: string | null;
  /** Lody généraliste : agit sur tous les projets depuis son espace. */
  orchestrator?: boolean;
  updatedAt: number;
}

export interface Download {
  path: string;
  name: string;
  modifiedAt: number;
  size: number;
}

export type Checks = "pass" | "fail" | "pending" | "none";

export interface PullRequest {
  number: number;
  title: string;
  branch: string;
  author: string;
  draft: boolean;
  review: string | null;
  url: string;
  updatedAt: number | null;
  checks: Checks;
}

export interface CiRun {
  id: number;
  title: string;
  workflow: string;
  branch: string;
  event: string;
  status: string;
  conclusion: string | null;
  url: string;
  createdAt: number | null;
  repo?: string;
}

export interface RepoStatus {
  slug: string | null;
  branch: string;
  ahead: number | null;
  behind: number | null;
  files: { status: string; path: string }[];
  commits: { hash: string; subject: string; author: string; date: number | null }[];
  prs: PullRequest[];
  runs: CiRun[];
  warnings: string[];
}

export interface SearchPr {
  number: number;
  title: string;
  url: string;
  repo: string;
  draft: boolean;
  updatedAt: number | null;
}

export interface GhOverview {
  myPrs: SearchPr[];
  reviews: SearchPr[];
  ci: CiRun[];
  warnings: string[];
}
