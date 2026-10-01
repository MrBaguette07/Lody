# Lody

Un petit assistant de bureau pour Windows qui vit en haut de l'écran. Lody pilote **Claude Code**, **Codex** et n'importe quel LLM (GPT, Claude API, Mistral, Gemini, Ollama…), suit tes agents en direct, te demande ton accord avant les actions sensibles et te montre le résultat.

Inspiré de [coucou](https://github.com/Louis-CFM/coucou). Construit avec Tauri 2 (Rust) et React.

## Fonctionnalités

Tout se passe dans **l'island**, le module en haut de l'écran. Pas de fenêtre à part.

- **Pilule** : Lody flotte en haut de l'écran (ou en bas à droite). Elle réagit à ce que font tes agents (réfléchit, travaille, attend, termine, erreur), suit la souris des yeux et laisse passer les clics en dehors d'elle. Fais-la glisser pour la déplacer, dépose un fichier dessus pour le joindre au chat.
- **Island dépliée** (clic sur Lody ou Ctrl+Alt+L) : Accueil, Chat, Agents, Projets, Git, Aperçu, Réglages. Échap ou un clic ailleurs la replie. L'épingle la garde ouverte.
- **Accueil, la barre de commande** : « ajoute en compta mon PDF téléchargé », « pourquoi ma CI est rouge ? », « fais un commit propre »… Lody lance Claude Code en lui donnant tes projets (chemins + dépôts GitHub) et tes derniers téléchargements. Il trouve le bon projet, lit sa procédure (README / CLAUDE.md) et agit, en demandant ton accord avant les actions sensibles. L'accueil affiche aussi les agents en cours et l'état GitHub : CI de tes dépôts actifs (avec *Réparer*), reviews demandées, tes PR ouvertes.
- **Piloter tes applis** : « mets-moi une musique chill », « ouvre Discord », « copie ça »… Lody utilise tes connecteurs claude.ai (Spotify, Gmail, ClickUp, Figma, Strava, Nodulz…) et son propre serveur MCP de bureau (`lody.exe mcp`), branché sur chaque run :
  - `open` : appli par son nom, URL, URI (`spotify:`, `mailto:`, `ms-settings:`…), fichier ou dossier ;
  - `now_playing` et `media` : ce qui joue, lecture/pause/suivant/précédent par appli, volume ;
  - `screenshot` et `click` : voir l'écran et cliquer comme un humain, pour les applis sans API (ex. le bouton Lecture de Spotify) ;
  - `windows`, `clipboard_get`, `clipboard_set`, `notify` (bulle dans l'island) et `send_keys` (soumis à validation).
- **Git** : branche, avance/retard, fichiers modifiés, commit manuel ou *Commit avec Lody*, pull/push, pull requests (checks, review, checkout, merge avec confirmation, créer, *PR avec Lody*, *Review avec Lody*), CI/CD (statuts, relancer, *Réparer*), derniers commits.
- **Validations depuis l'island** : quand Claude Code veut lancer une commande ou modifier un fichier, une carte apparaît sous Lody : *Autoriser*, *Toujours* (pour la session), *Refuser*, ou laisser le terminal demander.
- **Chat** :
  - *Claude Code* et *Codex* travaillent dans le dossier du projet choisi. Trois modes : Demander, Éditions auto, Libre. Les sessions reprennent automatiquement.
  - *Fournisseurs API* : tout endpoint compatible OpenAI (OpenAI, Ollama, LM Studio, OpenRouter, Mistral, Groq, Gemini, DeepSeek, xAI) et l'API Anthropic. Le contenu des fichiers joints est envoyé avec le message.
- **Agents** : toutes les sessions Claude Code (terminal, VS Code, lancées par Lody) avec leur statut, l'action en cours, les sous-agents, le dernier message. Raccourcis vers la conversation, VS Code et les changements.
- **Projets** : agrégés automatiquement depuis tes dossiers git, les récents de VS Code et Cursor, tes dépôts GitHub (`gh`), les serveurs MCP déclarés dans Claude Code (Nodulz par exemple), et tes sources personnalisées (dossier, commande, URL JSON). Ouvrir dans VS Code, l'Explorateur ou un terminal, cloner, discuter.
- **Aperçu** : lance le serveur de dev du projet (`npm run dev` détecté), affiche le site dans le panneau (l'URL est détectée dans les logs), et montre le diff git des changements faits par l'agent.
- **Confidentialité** : aucune télémétrie. Les clés API sont stockées dans le Gestionnaire d'identification Windows.

## Prérequis

- Windows 10/11 avec WebView2 (installé par défaut sur Windows 11)
- [Rust](https://rustup.rs) (toolchain MSVC) et les Build Tools Visual Studio
- Node.js 20+
- Facultatif : [Claude Code](https://code.claude.com), [Codex CLI](https://github.com/openai/codex) (`npm i -g @openai/codex`), [GitHub CLI](https://cli.github.com) (`gh auth login`)

Lody trouve Claude Code tout seul : dans le `PATH`, dans `~/.local/bin`, ou dans l'extension VS Code / Cursor.

## Lancer

```bash
npm install
npm run app      # mode développement
npm run bundle   # installeur NSIS dans src-tauri/target/release/bundle/
```

Raccourci global par défaut : **Ctrl+Alt+L** (modifiable dans Réglages). `lody.exe --tab git` ouvre directement un onglet (`home`, `chat`, `agents`, `projects`, `git`, `preview`, `settings`).

## Brancher Claude Code

Dans **Réglages → Hooks Claude Code**, clique sur **Installer**. Lody ajoute ses hooks dans `~/.claude/settings.json` sans toucher aux tiens et fait une sauvegarde (`settings.json.lody-backup`). Chaque hook exécute `lody.exe hook <port>`, un relais qui transmet l'événement au serveur local de Lody (`127.0.0.1:47823`). Si Lody est fermé, le relais sort sans rien faire : Claude Code fonctionne normalement.

Si tu déplaces `lody.exe` (passage du mode dev à l'app installée par exemple), clique sur **Réinstaller**.

## Ajouter une application à la liste des projets

- **Serveur MCP** (Nodulz…) : Réglages → Sources de projets → *Applications connectées*. Choisis le serveur (repris de `~/.claude.json`, authentification comprise), l'outil qui liste les éléments (`filesystem_get_tree` pour Nodulz) et éventuellement un modèle de lien (`https://…/workflows/{id}`).
- **Commande** : n'importe quel script qui affiche un tableau JSON `[{"name": "...", "path": "...", "url": "...", "description": "..."}]`.
- **URL JSON** : même format, récupéré en GET.
- **Dossier** : chaque sous-dossier git devient un projet.

## Architecture

```
src-tauri/src/
  app.rs        état, commandes, plateau système, raccourci
  hooks.rs      relais + serveur des hooks Claude Code, installation
  runner.rs     claude -p / codex exec en flux JSON
  providers.rs  chat API (OpenAI compatible, Anthropic) en streaming
  projects.rs   agrégation des projets (git, VS Code, Cursor, gh, MCP, perso)
  mcp.rs        client MCP HTTP minimal
  agents.rs     suivi des agents
  approvals.rs  demandes d'autorisation en attente
  git.rs        diff du projet
  devproc.rs    serveur de dev pour l'aperçu
  github.rs     état du dépôt, PR, CI, commit, push (git + gh)
  desktop.rs    serveur MCP de bureau : applis, musique, capture, clic
  window.rs     placement et ouverture de l'island
src/
  island/       l'island : pilule, onglets, click-through, repli
  panel/        vues : accueil, chat, agents, projets, git, aperçu, réglages
  components/   Lody (SVG animé), carte d'autorisation, statuts CI, markdown
```

Lody généraliste travaille depuis `%APPDATA%\dev.lody.app\workspace` avec accès (`--add-dir`) à tes dossiers de projets, Documents et Téléchargements.
