# ᜊᜐ᜔ᜃᜄᜈ᜔ SDE

A minimal Rust desktop application using Dioxus, with a white start screen and two centered, borderless actions.

- **Create Project** asks for a name and a parent folder, creates the project folder, and opens it.
- **Open Folder** opens an existing folder through the system folder picker.

Opening a folder displays its contents as a downward constellation of gray nodes on white.

- Single-click a folder to select its navigation anchor; double-click to enter it.
- Scroll down to move toward the project root. Scroll up to enter the selected folder, or the closest immediate folder to the pointer when none is selected.
- Use **Ctrl +** and **Ctrl −** to change scale. Drag to rotate around the vertical axis and Shift-drag to pan.
- The hierarchy icon switches between the current folder and an expanded overview. The curved-arrow icon restores the camera.
- The connection above the current folder leads to its parent; double-click it to go up. Collapsed folders with contents have downward continuation lines. Connections and nodes fade near the viewport boundaries.
- Navigation, rotation, panning, scale changes and reset use ease-in/ease-out motion. Offscreen dots and labels are culled while connections crossing the view remain visible.
- Hover and action information appears in the bottom status strip. When no message is active, it displays the current folder's path.
- File nodes can be selected with one click. Double-click a file to open it in an editor tab.
- The panel icon beside the reset icon toggles the rounded editor/terminal panel. The systems viewport adapts to its width; hiding it preserves tabs, unsaved text, cursor/undo state, and terminal sessions.
- **+** creates a new tab with full-width **Terminal** and **Editor** choices. New editor tabs use the selected file, or show **Open file** when none is selected.
- Editor tabs provide line numbers, syntax highlighting, undo/redo, indentation, bracket matching, folding and search. Save using **Ctrl+S** or the save icon. Closing an unsaved tab offers Save & close, Discard, and Cancel.
- The folder icon inside an editor toggles a traditional project file browser with expandable folders and a name filter.
- Terminal tabs run a real interactive PowerShell session on Windows (the system shell on other platforms), starting in the project folder.
- The **File** menu contains New Project, Open Folder, and Close Project.
- The filesystem watcher updates the view automatically. Periodic reconciliation catches missed events, with polling as a fallback when native watching is unavailable.

The view projects 3D positions into flat SVG dots and lines, with distance-based opacity and no lighting. Hierarchy stays vertical during rotation.

For responsiveness, each scan displays up to 2,000 nodes and 64 folder levels, reports truncation, and does not follow symbolic links. Enter a folder to inspect its contents separately. Dense overviews may require zooming and panning to read labels.

## Run

Chat sits below the systems view. Hover near their shared edge and click the dots to choose a ¾ systems, balanced, or ¾ chat layout. Enter sends and Shift+Enter adds a line. The composer grows up to half the chat area, then scrolls without reducing the systems view.

The welcome screen shows only Create Project and Open Folder. Chat and side panels belong to an open project. The plus button opens searchable actions, including files and folders. Typing `/` at the beginning of a composer line opens the same actions; arrows select, Enter invokes, and Escape closes without removing the draft. Conversations, drafts, attachment references, comments, and view settings are saved in the project's `.dado/state.json`. New Chat and the conversation list live in the left panel. Opening another project loads its own state. When a project already has a `.gitignore`, the app adds `/.dado/` while preserving existing rules; it does not create an ignore file when none exists.

Model Settings connects to an installed Codex App Server using the SDE's own user-level login storage. `DADO_CODEX_BIN` can select the executable. Sign in before choosing from the runtime's actual model catalog. Sending to Codex transmits the message and its supplied context; attachment selection alone does not send anything. Chat mode uses supplied context only. Inspect mode adds bounded project listing, literal text search and file reads, using open editor snapshots captured at send time. Tool rows show results. Folder references list immediate entries instead of preloading descendant contents. Agent modes require the currently validated runtime protocol, `0.162.0-alpha.2`; skills and Boost are still under development. Credentials are stored outside the project.

Runs are owned by the open project. Other conversations can queue work behind the foreground turn; queued work can be stopped before execution. Closing or switching projects stops its owned backend process tree. Project identity and run metadata are saved under `.dado/project.json` and `.dado/runs/`. Unresolved work becomes interrupted after restart or disconnect and is never replayed automatically. Hiding a panel keeps its run alive.

Inspect can propose replacements of existing text files in an isolated snapshot under `.dado/workspaces/`. The change icon on the reply opens a diff. **Apply to editor** checks disk and editor baselines, preserves captured unsaved content, and leaves the result unsaved for review. Use the usual save action to write it. External changes block applying instead of overwriting your text. Additions and deletions produced by approved commands are displayed in review with explicit operation markers and conflict checks. Applying reviews containing these operations to editors is still being implemented; Apply is unavailable for those reviews. Binary changes remain outside this text review flow.

The compact permission selector beside the plus button offers **Chat**, **Inspect**, **On approval** and **Full access**. Full access shows an orange hazard icon and enables native Codex file creation, edits and commands in the live project without approval. It has no proposal-only tools or Apply to editor step. Clean open editors refresh when a run ends; unsaved buffers are preserved. A changed selection applies to the next run; an active run retains its original permissions.

On approval requests show the exact command and reason with **Run once** and **Decline**. Each grant belongs to one pending command, run and project generation, expires after five minutes and cannot be replayed after stopping, switching projects or restarting. Approved commands start in the owned working copy, run with your OS permissions and have a 1–120 second timeout and 64 KB output cap per stream. The working copy is **not an OS sandbox**. Text edits, additions and deletions become reviewable proposals; binary, oversized and excess changes stay in the working copy with explicit notes. Applying additions/deletions to editors remains unavailable. Absence is distinct from an empty file, so concurrent creation or edits block the reviewed change. Dependency, generated and private files are excluded from the copy, so checks requiring those may need Full access. Native command and file-edit activity appears in expandable chat rows. Repeated terminal events cannot restart finished activity; failed exits and unconfirmed outcomes remain distinct, and interrupted work is never presented as successful.

Run metadata also records context provenance: resource identities, hashes, sources, line ranges and inclusion reasons. Captured editor buffers remain distinct from disk. Root and applicable nested `AGENTS.md` conventions are supplied with their revisions; those conventions cannot widen tool permissions. The provenance manifest stores metadata rather than copies of file contents.

Select text in a chat message to add an annotation. Numbered markers show the comment on hover and open it for editing or deletion when clicked. In the editor, select code or click a line and use the comment icon in the right margin. Commented code stays highlighted and its range follows edits. The chat input's comment icon and count opens a scrollable list above the input, showing compact text previews or code line ranges followed by the quoted, italic comment. Annotations are saved with the project and do not modify the source file.

Saved comments automatically enter the composer as removable chips and can be sent without typing a message. Sending captures their quotes and feedback, then clears the pending badge and chips while retaining the original annotations. Sent comments stay cleared after reopening; editing a comment queues its updated version. Removing a draft chip omits that version without deleting its annotation. Including a code comment does not automatically read its file or grant permission to change it.

**Skills** (`/skills`) browses runtime skills and private project skills. Select one to add a chip to the next message; Enter in the picker selects without sending. Sending captures its instructions and supporting files, rejects changed instruction revisions, and records provenance. Selected instructions cannot grant permissions. Inspect and On approval can list and read the selected skill's captured supporting files through scoped read-only tools. Chat receives instructions only; Full access uses native tools. Reads retain their captured hashes in provenance. The picker validates declared dependencies and invocation policy in `agents/openai.yaml`; skills requiring external tools are disabled until those tools are integrated. Changes to that configuration invalidate an earlier selection. Snapshot lifecycle handling and further integration checks remain in development.

**Create skill** (`/create-skill`) asks for a name, purpose, activation conditions and scope. The selected model drafts an instruction-only skill without execution tools, then opens the validated draft in the editor. Review or edit it, reopen Create skill, and use **Publish** beside the draft. Publication checks for external edits and never overwrites an existing skill. Private skills go to `.dado/skills`; shared skills go to `.agents/skills`. Unpublished drafts survive reopening under `.dado/skill-drafts` and remain unavailable to discovery. Creating a skill preserves the current message draft and pending attachments/comments. Published private skills also appear here with a Share icon: it copies the skill and supporting resources to `.agents/skills`, preserves the private copy, and rejects stale revisions or an existing shared skill.

```sh
cargo run
```

You can also open a folder on launch with `cargo run -- "C:\path\to\project"`.

## Build

```sh
cargo build
```

Windows requires the Microsoft C++ build tools and WebView2 Runtime.

The bundled editor (CodeMirror) and terminal (xterm.js) assets are checked in, so running the desktop app does not require Node.js or network access. After editing `ui/workbench.js`, regenerate them with `npm ci` and `npm run build`, then rebuild the Rust app.

The first editor supports UTF-8 text files up to 5 MB, preserves a UTF-8 BOM and CRLF endings, and rejects saves when the file's text has changed on disk since it was loaded. Panel state lasts for the current app session.

The window setup and root UI component are in `src/main.rs`.

## Checks

```sh
cargo test
cargo clippy -- -D warnings
node tests/scene.test.cjs
node tests/commands.test.cjs
node tests/review.test.cjs
node tests/comment_context.test.cjs
```

An opt-in real-runtime inspection check sends one model request using the existing SDE login. It searches and reads a disposable project's unsaved editor snapshot and verifies that disk stays unchanged:

```sh
cargo test --bin dado-sde real_runtime_inspection_smoke -- --ignored --nocapture
```

Optional browser interaction checks use Playwright: run `npm ci`, then `npm test`. Install Playwright's Chromium browser first, or set `DADO_TEST_BROWSER=chrome` to use an installed Chrome browser.
