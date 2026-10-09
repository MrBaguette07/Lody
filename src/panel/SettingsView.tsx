import { useEffect, useRef, useState, type ReactNode } from "react";
import { open } from "../lib/dialog";
import { CircleCheck, CircleX, KeyRound, ListChecks, Plus, Power, RefreshCw, Trash2 } from "lucide-react";
import { api, errText } from "../lib/api";
import { uid, useStore } from "../lib/store";
import type { ApiProvider, CustomSource, McpSource, Settings } from "../lib/types";

const PRESETS: { name: string; kind: ApiProvider["kind"]; baseUrl: string; model: string }[] = [
  { name: "GPT (OpenAI)", kind: "openai", baseUrl: "https://api.openai.com/v1", model: "" },
  { name: "Claude (API)", kind: "anthropic", baseUrl: "https://api.anthropic.com/v1", model: "claude-opus-5-5" },
  { name: "Ollama (local)", kind: "openai", baseUrl: "http://localhost:11434/v1", model: "llama3.2" },
  { name: "LM Studio (local)", kind: "openai", baseUrl: "http://localhost:1234/v1", model: "" },
  { name: "OpenRouter", kind: "openai", baseUrl: "https://openrouter.ai/api/v1", model: "" },
  { name: "Mistral", kind: "openai", baseUrl: "https://api.mistral.ai/v1", model: "mistral-large-latest" },
  { name: "Gemini", kind: "openai", baseUrl: "https://generativelanguage.googleapis.com/v1beta/openai", model: "" },
  { name: "Groq", kind: "openai", baseUrl: "https://api.groq.com/openai/v1", model: "" },
  { name: "DeepSeek", kind: "openai", baseUrl: "https://api.deepseek.com/v1", model: "deepseek-chat" },
  { name: "xAI (Grok)", kind: "openai", baseUrl: "https://api.x.ai/v1", model: "" },
  { name: "Personnalisé", kind: "openai", baseUrl: "", model: "" },
];

function Section({ title, desc, children, action }: { title: string; desc?: string; children: ReactNode; action?: ReactNode }) {
  return (
    <section className="section">
      <div className="section-head">
        <div>
          <h2>{title}</h2>
          {desc && <p>{desc}</p>}
        </div>
        {action}
      </div>
      {children}
    </section>
  );
}

function Field({ label, children, hint }: { label: string; children: ReactNode; hint?: ReactNode }) {
  return (
    <label className="field">
      <span>{label}</span>
      {children}
      {hint && <small>{hint}</small>}
    </label>
  );
}

function Toggle({ on, onChange, label }: { on: boolean; onChange: (v: boolean) => void; label: string }) {
  return (
    <button type="button" className={`toggle ${on ? "on" : ""}`} onClick={() => onChange(!on)}>
      <i />
      <span>{label}</span>
    </button>
  );
}

function ProviderCard({ p, onChange, onRemove }: { p: ApiProvider; onChange: (p: ApiProvider) => void; onRemove: () => void }) {
  const [key, setKey] = useState("");
  const [models, setModels] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);
  const showToast = useStore((s) => s.showToast);
  const saveKey = async () => {
    try {
      await api.setSecret(`provider:${p.id}`, key.trim());
      setKey("");
      showToast(key.trim() ? "Clé enregistrée dans le Gestionnaire d'identification Windows." : "Clé supprimée.");
    } catch (e) {
      showToast(errText(e));
    }
  };
  const loadModels = async () => {
    setLoading(true);
    try {
      const m = await api.listModels(p.id);
      setModels(m);
      if (!m.length) showToast("Aucun modèle renvoyé.");
    } catch (e) {
      showToast(errText(e));
    } finally {
      setLoading(false);
    }
  };
  const local = p.baseUrl.includes("localhost") || p.baseUrl.includes("127.0.0.1");
  return (
    <div className="card">
      <div className="card-row">
        <input className="input strong" value={p.name} onChange={(e) => onChange({ ...p, name: e.target.value })} />
        <select className="select" value={p.kind} onChange={(e) => onChange({ ...p, kind: e.target.value as ApiProvider["kind"] })}>
          <option value="openai">Compatible OpenAI</option>
          <option value="anthropic">Anthropic</option>
        </select>
        <button className="btn ghost icon sm" title="Supprimer" onClick={onRemove}>
          <Trash2 size={14} />
        </button>
      </div>
      <Field label="URL de base">
        <input className="input mono" value={p.baseUrl} onChange={(e) => onChange({ ...p, baseUrl: e.target.value })} />
      </Field>
      <Field label="Modèle">
        <div className="card-row">
          <input
            className="input mono"
            list={`models-${p.id}`}
            value={p.model}
            placeholder="nom du modèle"
            onChange={(e) => onChange({ ...p, model: e.target.value })}
          />
          <datalist id={`models-${p.id}`}>
            {models.map((m) => (
              <option key={m} value={m} />
            ))}
          </datalist>
          <button className="btn sm" onClick={loadModels} disabled={loading} title="Récupérer la liste des modèles">
            <RefreshCw size={13} className={loading ? "spin" : ""} /> Modèles
          </button>
        </div>
      </Field>
      <Field
        label="Clé API"
        hint={p.hasKey ? <span className="ok-text">● Clé enregistrée</span> : local ? "Pas de clé nécessaire en local." : "Aucune clé enregistrée."}
      >
        <div className="card-row">
          <input
            className="input mono"
            type="password"
            value={key}
            placeholder={p.hasKey ? "•••••••• (laisser vide puis Enregistrer pour supprimer)" : "sk-…"}
            onChange={(e) => setKey(e.target.value)}
          />
          <button className="btn sm" onClick={saveKey}>
            <KeyRound size={13} /> Enregistrer
          </button>
        </div>
      </Field>
    </div>
  );
}

function McpCard({ s, onChange, onRemove, servers }: { s: McpSource; onChange: (s: McpSource) => void; onRemove: () => void; servers: { name: string; url: string }[] }) {
  const [tools, setTools] = useState<{ name: string; description?: string }[]>([]);
  const [loading, setLoading] = useState(false);
  const showToast = useStore((st) => st.showToast);
  const listTools = async () => {
    setLoading(true);
    try {
      setTools(await api.mcpListTools(s));
    } catch (e) {
      showToast(errText(e));
    } finally {
      setLoading(false);
    }
  };
  return (
    <div className="card">
      <div className="card-row">
        <input className="input strong" value={s.name} onChange={(e) => onChange({ ...s, name: e.target.value })} />
        <Toggle on={s.enabled} onChange={(enabled) => onChange({ ...s, enabled })} label="Actif" />
        <button className="btn ghost icon sm" title="Supprimer" onClick={onRemove}>
          <Trash2 size={14} />
        </button>
      </div>
      <Field label="Serveur MCP" hint="Repris de ~/.claude.json (URL et en-têtes d'authentification inclus).">
        <select className="select" value={s.server} onChange={(e) => onChange({ ...s, server: e.target.value })}>
          <option value="">Choisir…</option>
          {servers.map((sv) => (
            <option key={sv.name} value={sv.name}>
              {sv.name} · {sv.url}
            </option>
          ))}
        </select>
      </Field>
      <Field label="Outil qui liste les projets">
        <div className="card-row">
          <input className="input mono" list={`tools-${s.id}`} value={s.tool} onChange={(e) => onChange({ ...s, tool: e.target.value })} />
          <datalist id={`tools-${s.id}`}>
            {tools.map((t) => (
              <option key={t.name} value={t.name}>
                {t.description}
              </option>
            ))}
          </datalist>
          <button className="btn sm" onClick={listTools} disabled={loading || !s.server}>
            <ListChecks size={13} className={loading ? "spin" : ""} /> Outils
          </button>
        </div>
      </Field>
      <Field label="Lien d'un élément" hint="{id} est remplacé par l'identifiant. Laisser vide si inutile.">
        <input
          className="input mono"
          placeholder="https://app.exemple.com/workflows/{id}"
          value={s.linkTemplate}
          onChange={(e) => onChange({ ...s, linkTemplate: e.target.value })}
        />
      </Field>
    </div>
  );
}

function CustomCard({ s, onChange, onRemove }: { s: CustomSource; onChange: (s: CustomSource) => void; onRemove: () => void }) {
  const hint =
    s.kind === "folder"
      ? "Chaque sous-dossier git devient un projet."
      : s.kind === "command"
        ? 'Commande qui affiche un tableau JSON : [{"name": "...", "path": "...", "url": "..."}]'
        : "URL qui renvoie un tableau JSON : [{\"name\": \"...\", \"url\": \"...\"}]";
  return (
    <div className="card">
      <div className="card-row">
        <input className="input strong" value={s.name} onChange={(e) => onChange({ ...s, name: e.target.value })} />
        <select className="select" value={s.kind} onChange={(e) => onChange({ ...s, kind: e.target.value as CustomSource["kind"] })}>
          <option value="folder">Dossier</option>
          <option value="command">Commande</option>
          <option value="http">URL JSON</option>
        </select>
        <Toggle on={s.enabled} onChange={(enabled) => onChange({ ...s, enabled })} label="Actif" />
        <button className="btn ghost icon sm" title="Supprimer" onClick={onRemove}>
          <Trash2 size={14} />
        </button>
      </div>
      <Field label={s.kind === "folder" ? "Dossier" : s.kind === "command" ? "Commande" : "URL"} hint={hint}>
        <div className="card-row">
          <input className="input mono" value={s.value} onChange={(e) => onChange({ ...s, value: e.target.value })} />
          {s.kind === "folder" && (
            <button
              className="btn sm"
              onClick={async () => {
                const d = await open({ directory: true });
                if (typeof d === "string") onChange({ ...s, value: d });
              }}
            >
              Parcourir
            </button>
          )}
        </div>
      </Field>
    </div>
  );
}

export default function SettingsView() {
  const saved = useStore((s) => s.settings);
  const bins = useStore((s) => s.bins);
  const hookStatus = useStore((s) => s.hookStatus);
  const { saveSettings, refreshEnv, showToast, loadProjects } = useStore.getState();
  const [draft, setDraft] = useState<Settings | null>(saved);
  const [servers, setServers] = useState<{ name: string; url: string }[]>([]);
  const [version, setVersion] = useState("");
  const timer = useRef<number | undefined>(undefined);
  const dirty = useRef(false);

  useEffect(() => {
    if (!dirty.current) setDraft(saved);
  }, [saved]);

  useEffect(() => {
    api.claudeMcpServers().then(setServers).catch(() => {});
    api.appInfo().then((i) => setVersion(i.version));
  }, []);

  if (!draft) return null;

  const update = (patch: Partial<Settings>) => {
    const next = { ...draft, ...patch };
    setDraft(next);
    dirty.current = true;
    clearTimeout(timer.current);
    timer.current = window.setTimeout(async () => {
      await saveSettings(next);
      dirty.current = false;
    }, 500);
  };

  const hookAction = async (install: boolean) => {
    try {
      const st = install ? await api.hooksInstall() : await api.hooksUninstall();
      useStore.setState({ hookStatus: st });
      showToast(install ? "Hooks installés dans ~/.claude/settings.json (sauvegarde créée)." : "Hooks retirés.");
    } catch (e) {
      showToast(errText(e));
    }
  };

  const setProvider = (i: number, p: ApiProvider) => update({ providers: draft.providers.map((x, j) => (j === i ? p : x)) });
  const setMcp = (i: number, s: McpSource) => update({ mcpSources: draft.mcpSources.map((x, j) => (j === i ? s : x)) });
  const setCustom = (i: number, s: CustomSource) => update({ customSources: draft.customSources.map((x, j) => (j === i ? s : x)) });

  return (
    <div className="view">
      <div className="view-head">
        <h1>Réglages</h1>
        <span className="muted">enregistrement automatique</span>
      </div>
      <div className="scroll settings">
        <Section title="Agents de code" desc="Lody pilote les CLI installées sur ton PC.">
          {(["claude", "codex"] as const).map((k) => (
            <Field
              key={k}
              label={k === "claude" ? "Claude Code" : "Codex"}
              hint={
                bins[k] ? (
                  <span className="ok-text">
                    <CircleCheck size={12} /> {bins[k]}
                  </span>
                ) : (
                  <span className="err-text">
                    <CircleX size={12} /> Introuvable{k === "codex" ? " : npm i -g @openai/codex" : ""}
                  </span>
                )
              }
            >
              <input
                className="input mono"
                placeholder="Chemin auto-détecté (laisser vide)"
                value={k === "claude" ? draft.claudePath : draft.codexPath}
                onChange={(e) => update(k === "claude" ? { claudePath: e.target.value } : { codexPath: e.target.value })}
              />
            </Field>
          ))}
          <div className="card-row">
            <Field label="Mode par défaut">
              <select className="select" value={draft.defaultMode} onChange={(e) => update({ defaultMode: e.target.value as Settings["defaultMode"] })}>
                <option value="ask">Demander avant d'agir</option>
                <option value="edits">Éditions automatiques</option>
                <option value="yolo">Libre (aucune validation)</option>
              </select>
            </Field>
            <Field label="Délai de validation (s)">
              <input
                className="input"
                type="number"
                min={10}
                value={draft.approvalTimeoutSecs}
                onChange={(e) => update({ approvalTimeoutSecs: Number(e.target.value) || 600 })}
              />
            </Field>
          </div>
          <button className="btn sm ghost" onClick={() => refreshEnv()}>
            <RefreshCw size={13} /> Redétecter
          </button>
        </Section>

        <Section
          title="Hooks Claude Code"
          desc="Relie toutes tes sessions Claude Code (terminal, VS Code) à Lody : suivi en direct et validations depuis la mascotte. Sans effet quand Lody est fermé."
        >
          <div className="card-row">
            <span className={hookStatus?.upToDate ? "ok-text" : "muted"}>
              {hookStatus?.upToDate ? "● Installés et à jour" : hookStatus?.installed ? "● Installés, à mettre à jour" : "○ Non installés"}
            </span>
            <span className="grow" />
            <button className="btn sm primary" onClick={() => hookAction(true)}>
              {hookStatus?.installed ? "Réinstaller" : "Installer"}
            </button>
            {hookStatus?.installed && (
              <button className="btn sm ghost" onClick={() => hookAction(false)}>
                Retirer
              </button>
            )}
          </div>
          <Field label="Port local" hint="Redémarre Lody puis réinstalle les hooks après un changement.">
            <input className="input" type="number" value={draft.hookPort} onChange={(e) => update({ hookPort: Number(e.target.value) || 47823 })} />
          </Field>
        </Section>

        <Section
          title="Fournisseurs IA"
          desc="Pour discuter directement avec GPT, Claude, Mistral, un modèle local… Les clés sont stockées dans le Gestionnaire d'identification Windows."
          action={
            <select
              className="select"
              value=""
              onChange={(e) => {
                const preset = PRESETS[Number(e.target.value)];
                if (preset) update({ providers: [...draft.providers, { id: uid().slice(0, 8), hasKey: false, ...preset }] });
              }}
            >
              <option value="">+ Ajouter…</option>
              {PRESETS.map((p, i) => (
                <option key={p.name} value={i}>
                  {p.name}
                </option>
              ))}
            </select>
          }
        >
          {draft.providers.map((p, i) => (
            <ProviderCard
              key={p.id}
              p={p}
              onChange={(np) => setProvider(i, np)}
              onRemove={() => update({ providers: draft.providers.filter((_, j) => j !== i) })}
            />
          ))}
          <Field label="Instructions système (chat API)">
            <textarea className="input" rows={3} value={draft.systemPrompt} onChange={(e) => update({ systemPrompt: e.target.value })} />
          </Field>
        </Section>

        <Section title="Sources de projets" desc="D'où Lody récupère tes projets." action={<button className="btn sm ghost" onClick={loadProjects}><RefreshCw size={13} /> Actualiser</button>}>
          <div className="toggles">
            <Toggle on={draft.sources.folders} onChange={(folders) => update({ sources: { ...draft.sources, folders } })} label="Dossiers git" />
            <Toggle on={draft.sources.vscode} onChange={(vscode) => update({ sources: { ...draft.sources, vscode } })} label="Récents VS Code" />
            <Toggle on={draft.sources.cursor} onChange={(cursor) => update({ sources: { ...draft.sources, cursor } })} label="Récents Cursor" />
            <Toggle on={draft.sources.github} onChange={(github) => update({ sources: { ...draft.sources, github } })} label="GitHub (gh)" />
          </div>
          <Field label="Dossiers scannés" hint="Vide = Documents\GitHub, source\repos, Projects, dev, code.">
            <div className="list">
              {draft.projectRoots.map((r) => (
                <div key={r} className="list-item">
                  <span className="mono">{r}</span>
                  <button className="btn ghost icon sm" onClick={() => update({ projectRoots: draft.projectRoots.filter((x) => x !== r) })}>
                    <Trash2 size={13} />
                  </button>
                </div>
              ))}
              <button
                className="btn sm ghost"
                onClick={async () => {
                  const d = await open({ directory: true });
                  if (typeof d === "string" && !draft.projectRoots.includes(d)) update({ projectRoots: [...draft.projectRoots, d] });
                }}
              >
                <Plus size={13} /> Ajouter un dossier
              </button>
            </div>
          </Field>

          <h3>Applications connectées (MCP)</h3>
          <p className="muted small">Nodulz ou tout serveur MCP déclaré dans Claude Code : Lody appelle un outil qui liste tes projets / workflows.</p>
          {draft.mcpSources.map((s, i) => (
            <McpCard key={s.id} s={s} servers={servers} onChange={(ns) => setMcp(i, ns)} onRemove={() => update({ mcpSources: draft.mcpSources.filter((_, j) => j !== i) })} />
          ))}
          <div className="card-row wrap">
            {servers
              .filter((sv) => !draft.mcpSources.some((m) => m.server === sv.name))
              .map((sv) => (
                <button
                  key={sv.name}
                  className="btn sm soft"
                  onClick={() =>
                    update({
                      mcpSources: [
                        ...draft.mcpSources,
                        {
                          id: uid().slice(0, 8),
                          name: sv.name.charAt(0).toUpperCase() + sv.name.slice(1),
                          server: sv.name,
                          url: "",
                          tool: sv.name.toLowerCase().includes("nodulz") ? "filesystem_get_tree" : "",
                          args: {},
                          linkTemplate: "",
                          enabled: true,
                        },
                      ],
                    })
                  }
                >
                  <Plus size={13} /> {sv.name}
                </button>
              ))}
            {!servers.length && <span className="muted small">Aucun serveur MCP HTTP trouvé dans ~/.claude.json.</span>}
          </div>

          <h3>Sources personnalisées</h3>
          {draft.customSources.map((s, i) => (
            <CustomCard key={s.id} s={s} onChange={(ns) => setCustom(i, ns)} onRemove={() => update({ customSources: draft.customSources.filter((_, j) => j !== i) })} />
          ))}
          <button
            className="btn sm ghost"
            onClick={() =>
              update({
                customSources: [...draft.customSources, { id: uid().slice(0, 8), name: "Ma source", kind: "folder", value: "", enabled: true }],
              })
            }
          >
            <Plus size={13} /> Ajouter une source
          </button>
        </Section>

        <Section title="Mascotte">
          <div className="toggles">
            <Toggle on={draft.sounds} onChange={(sounds) => update({ sounds })} label="Sons" />
          </div>
          <div className="card-row">
            <Field label="Position">
              <select
                className="select"
                value={draft.mascotPosition}
                onChange={async (e) => {
                  await saveSettings({ ...draft, mascotPosition: e.target.value as Settings["mascotPosition"] });
                  setDraft({ ...draft, mascotPosition: e.target.value as Settings["mascotPosition"] });
                  api.resetMascotPos();
                }}
              >
                <option value="top">En haut de l'écran</option>
                <option value="bottom">En bas à droite</option>
              </select>
            </Field>
            <Field label="Raccourci global" hint="Pris en compte au prochain démarrage.">
              <input className="input mono" value={draft.shortcut} onChange={(e) => update({ shortcut: e.target.value })} />
            </Field>
          </div>
          <div className="card-row">
            <button className="btn sm" onClick={() => api.resetMascotPos()}>
              Replacer la mascotte
            </button>
            <button className="btn sm ghost" onClick={() => api.setMascotVisible(false)}>
              Masquer
            </button>
            <button className="btn sm ghost" onClick={() => api.setMascotVisible(true)}>
              Afficher
            </button>
          </div>
        </Section>

        <div className="about">
          <span>Lody {version}</span>
          <button className="btn sm danger" onClick={() => api.quit()}>
            <Power size={13} /> Quitter Lody
          </button>
        </div>
      </div>
    </div>
  );
}
