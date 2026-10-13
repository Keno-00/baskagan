# ᜊᜐ᜔ᜃᜄᜈ᜔ SDE: project agent harness

Implementation plan, originally drafted for discussion on 9 October 2026. The user subsequently authorized implementation of this plan. The requirements below retain their full scope; the progress notes record evidence rather than redefine completion.

## Implementation evidence — 9 October 2026

- The AI slice is integrated into this checkout. Authentication, catalog, streaming, stop, shared plus/slash actions, and initial inspection tools are implemented. Inspection provides immediate directory listings, bounded literal search, and reads from captured editor buffers with SHA-256 revisions. Each run has a 64-call/2 MB project-tool budget.
- `cargo test --bin dado-sde`: 67 deterministic tests pass; four credential-dependent smoke tests are ignored by default. Clippy with warnings denied and the JavaScript command, annotation-context and review checks pass. These are partial checks, not verification of every requirement below.
- The explicit real-runtime smoke test passed against the installed Codex runtime. A real model used both search and read tools on an unsaved buffer snapshot, answered with its content, and left the different disk baseline unchanged. The test is available as `cargo test --bin dado-sde real_runtime_inspection_smoke -- --ignored --nocapture`.
- Real-runtime testing exposed and fixed a catalog refresh race. Catalog replacement is now atomic for the same account; old account pages cannot restore models after logout. The earlier code-mode-host restriction was removed so the pinned model can invoke the scoped tools.
- Native desktop checks from the integration pass verified plus search, New Chat invocation, slash discovery, empty-result Enter suppression and Escape preserving the draft. No Playwright checks were run in this chat. The complete keyboard, IME and project-switch matrix still needs verification.
- Rust now owns a `ProjectRuntime`, persistent UUID in `.dado/project.json`, session generation, run map and queue. Atomic `.dado/runs/*.json` records retain identities, lifecycle state and backend thread/turn references, without persisting executable requests or grants. Restart marks unresolved records interrupted; reconnect does not replay them. A supervisor schedules one foreground Codex turn, with up to 32 queued requests, and stops/joins the old backend before activating another project. The backend's individual turn slot remains serial until the Boost stage.
- Deterministic supervisor checks cover queued cancellation before execution, project switching, stale generations after reopening the same path, terminal-event idempotence, disconnect and manual reconnect without replay. A Windows process test verifies termination of an owned parent and descendant while an unrelated process remains alive. The supervisor holds a shared Job handle so it can terminate the tree before joining a blocked backend worker.
- Native verification completed an actual project inspection through the supervisor, observed tool rows and durable completed thread/turn metadata, then closed the project. The welcome screen showed only the centered project actions; the SDE-owned Codex process was gone. UI loading now waits for the project-owner acknowledgement so old-run saves precede loading a reopened project.
- Inspect now exposes `sde_propose_file`. The first proposal creates a bounded owned snapshot including ordinary untracked files and captured editor buffers; the live project remains untouched. Existing UTF-8 files can be replaced with an expected content revision. SHA-256 baselines, run/project ownership and proposal manifests survive reopening; external disk changes are surfaced as conflicts. Incomplete cancelled construction removes only its owned temporary workspace.
- A compact reply icon opens a line-numbered change review. Apply preflights every disk/editor baseline and locks affected editors during the handoff, then applies into editor buffers with undo history. Saving remains explicit through the existing conflict-checking save flow. New/deleted files, merging overlapping changes, execution/checks and a complete race-resistant filesystem boundary remain outstanding.
- Native desktop verification used a real model to propose changing a disposable `main.rs` from `Hello` to `Reviewed change`. The diff preserved an unsaved comment. Applying it changed the editor while the live disk file still printed `Hello`. The explicit save step was not verified in this pass because desktop interaction was interrupted by user activity.
- Windows runtime startup now creates Codex suspended, attaches its process to the owned Job and only then resumes its primary thread. A deterministic test immediately spawns a descendant, verifies membership in that exact Job, and verifies both processes terminate. The separate unrelated-process survival test remains. This establishes tree ownership, not a command-execution sandbox.
- After the startup/transport changes, the opt-in real-runtime inspection smoke passed again. The 32 deterministic Rust tests, Clippy with warnings denied, JavaScript command/review checks, syntax check and diff whitespace check passed; the desktop binary rebuilt successfully.
- JSONL transport now bounds each frame to 2 MB and queues at most 32 decoded frames. Each backend polling pass handles at most 64 frames before servicing cancellation/deadlines. Fragmented UTF-8, CRLF, adjacent frames, malformed/oversized frames and mid-frame EOF have deterministic coverage. Supervisor tests verify broken framing preserves partial output, interrupts once and does not replay on reconnect.
- Runs now persist a versioned bounded context manifest in their owned run record. Resource identities, sources, SHA-256 revisions, line ranges, inclusion reasons and whether content was supplied, available through tools or observed are recorded without duplicating file/message contents. Explicit attachments prefer the actual captured editor bytes across equivalent path spellings. Folder references record only the immediate listing. Tool results, including searches with zero matches, retain result hashes; resumed conversations retain an honest backend-history reference rather than claiming a known history hash.
- The selected file reference now crosses the UI bridge. Root and applicable nested `AGENTS.md` files are supplied with path/scope/source/revision provenance; instructions use captured editor text when available, remain bounded to 64 KB per applicable scope and cannot grant permissions. Explicit project attachments include their applicable conventions, and file reads carry the root-to-file instructions. Instruction revisions remain fixed in an isolated proposal snapshot after the context switches there.
- Tests cover equivalent attachment paths, unsaved versus disk content, metadata without raw contents, duplicate inclusion reasons, revision changes, provenance bounds, excluded sibling instructions, instruction size limits, empty searches and durable context after reopening. The real-runtime smoke also verifies that the model follows a disposable project's instruction while inspecting an unsaved buffer and leaving disk unchanged.
- Saved annotations now automatically queue in the composer, following the user's correction that the comment badge already represents message content. Compact removable chips preserve chat previews or code line ranges. Comments can be sent without typed text; sending clears their pending badge and chips while retaining the source annotations. Sent/dismissed versions remain cleared after reopening, and editing queues a changed version. Sending freezes the quote, feedback and anchors under stable identities independent of visible comment numbering. The bounded backend validates those snapshots and records hashes, sources and range units without treating annotations as permission grants or implicit file reads. Fresh backend threads receive previously sent annotation snapshots with history; resumed threads do not replay them.
- Deterministic tests verify annotation-only requests cross the protocol boundary, unrelated comments and file contents stay out, durable run metadata excludes raw quotes, and history/resume behavior is distinct. Queue tests cover automatic inclusion, clearing after send, reopening, editing, dismissal and migration of previously sent snapshots. A native desktop test sent two comments with no typed text and received an actual Codex answer; the pending badge and chips cleared while both source annotations remained stored. An earlier live attempt had returned a usage-limit error.
- The permission selector beside the attachment action now provides Chat, Inspect, On approval and Full access using Dioxus Select. Full access has an orange hazard icon. Permissions are captured per run; changing the selection affects the next turn. Approved commands use a deferred host tool, a one-time run/project/generation-bound token and an owned working copy. The UI states that commands run with user OS permissions, rather than claiming an OS sandbox. Existing text changes join the review flow; unsupported changes are reported. Stop and project retirement invalidate pending grants, and running commands use the owned App Server process tree. Full access enables native live edits and shell tools under dangerFullAccess/never; the backend verifies the runtime-confirmed policy before starting a turn.
- Real runtime smoke tests passed for inspection, an approved command and Full access. They verified captured-buffer reads, command execution only after an explicit grant without changing the live baseline, and a native live file edit followed by a native verification command. Deterministic tests cover stale/replayed grants, decline, stopping a pending approval, command bounds and review reconciliation. No Playwright checks were run.
- Corrected Full access exposing proposal-only tools: this mode now advertises only native runtime tools and rejects obsolete dynamic-tool calls. Its conversation tool version changes so existing chats start a fresh backend thread while preserving visible history. The real smoke test now uses an ordinary request with no tool hints and verifies both an existing-file edit and a new nested file on live disk, followed by a command check and no proposal workspace. Clean editor tabs refresh after a Full access run; concurrent/unsaved editor changes are preserved.
- Added a per-turn typed item reducer for assistant messages, native command execution and file edits. Terminal item states cannot regress after duplicate completions or late starts; a nonzero exit is failed even when the backend item says completed, and missing exit/outcome evidence stays unknown. Final turn item snapshots reconcile missed completion messages. Disconnect/stop settles unfinished native activity as interrupted, and the chat also settles pending custom-tool rows. Item count, aggregate assistant text and rendered tool output are bounded. Unit and protocol-fixture tests cover these cases; this is not complete backend reconciliation or coverage of every protocol item kind.
- Skills now have runtime discovery plus explicit private `.dado/skills` discovery, searchable selection, removable composer chips and per-turn instruction/resource snapshots. Identity distinguishes scope/path collisions and changed instruction revisions fail before inference. Runtime discovery is refreshed when opening the picker and sending a selected skill; runtime change notifications dismiss stale menus. The pinned runtime accepted a frozen private skill reference without loading it, so the adapter supplies the exact captured instructions explicitly alongside the native reference. An opt-in real runtime test verifies the instructions affect the response and have provenance. Inspect and On approval now have scoped read-only tools for selected supporting resources, using bytes captured in memory for the run. Availability and reads record their hashes; excluded private/generated entries are not copied. A real runtime test listed/read a supporting rule and used its phrase. Chat remains instructions-only and Full access uses native tools. Complete prerequisite validation and snapshot lifecycle remain outstanding.
- Create skill is now a plus/slash action asking for purpose, activation conditions and private/shared scope. The selected model drafts instruction-only SKILL.md in Chat mode, with no execution tools. A validated draft opens in the editor and remains outside discovery until explicit Publish. Publication uses the captured editor text, checks its original disk baseline in Rust, rejects malformed metadata/local supporting references/name collisions and writes to `.dado/skills` or `.agents/skills`. Drafts survive restarting and published records retain their review copy. A native disposable-project check created a real agent draft, restarted before publication, reviewed/published it, restarted again, discovered it, selected it using Enter without sending, and received its expected phrase from the real model. No Playwright checks were run. Unit/protocol tests cover unsaved reviewed text, stale disk baselines, invalid scopes/paths/references, collisions and stale project generations.
- Private skill sharing now copies captured resources to `.agents/skills` explicitly, preserves the private copy, rejects stale instructions and existing targets, and writes SKILL.md last. A native desktop check used the Share icon in a disposable project and verified identical hashes for the private/shared instructions. Open private editor tabs no longer block ordinary inspection or enter proposal snapshots.
- Skill configuration now validates bounded `agents/openai.yaml` for private and runtime-discovered skills. Declared external tool dependencies, malformed declarations and ChatGPT-only policies disable selection with status help; dependency-free explicit invocation remains available. Configuration revisions are pinned in composer selections, revalidated before inference and against captured resources, and recorded in provenance. Unit tests cover adding/changing/removing configuration, malformed/oversized input and runtime dependency shapes; the owned protocol test rejects a changed configuration before the run. The real runtime smoke also passed with explicit-only invocation policy and an empty dependency list, reading its selected supporting rule. Host validation has its own provenance exposure, distinct from content actually supplied to or read by the model. This follows the [official optional skill metadata format](https://learn.chatgpt.com/docs/build-skills#optional-metadata). Filesystem race hardening and broader lifecycle gates remain outstanding.
- Approved command reconciliation now captures text additions and deletions alongside edits, retaining absence independently of empty content, with a shared 16-file review cap. Reviews survive reopening and verify both work-copy revisions and the live baseline, including concurrent creation/deletion conflicts. Operation markers and complete added/removed lines are shown in review. Apply is intentionally unavailable for mixed reviews containing additions/deletions until their editor/Save handoff is implemented; the host also rejects marking those reviews applied. A real runtime approved-command test edited, created a nested file and deleted a file in the disposable work copy, verified all three review operations and proved the live originals remained unchanged. Unit tests cover empty files, reopening, tampering, bounds and removal of previously captured additions. JavaScript checks cover operation diffs and application availability. No Playwright or native visual check was run for this review increment.
- Added the Windows host Save path for reviewed additions/deletions, bound to the owned run and current project session. It requires an editor-staged review, verifies the captured proposal and live baseline, records saved revisions atomically, and rejects replay after reopening. Additions publish without overwriting a competing file. Parent directory handles deny replacement while saving; deletions verify and mark the same file handle for deletion while rejecting active writers. Disposable filesystem tests cover content conflicts, saved-state recovery, path traversal/private/device paths, ancestor replacement and an active writer; protocol tests reject unstaged saves and retired generations. The implementation follows [Windows disposition-by-handle](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-setfileinformationbyhandle) and [Rust Windows share-mode semantics](https://doc.rust-lang.org/std/os/windows/fs/trait.OpenOptionsExt.html). Editor staging/Save wiring supports additions and read-only pending deletions on Windows. Unsaved additions are available to agent reads, searches and directory listings without creating live files. All 68 deterministic tests and the real Full access smoke test pass. A native UI pass and the full filesystem race/isolation gate remain pending. Non-Windows structural Save is explicitly unavailable.
- Still required: complete typed item/event reduction and backend reconciliation; remaining context/provenance cases; race-resistant filesystem boundaries; remaining command/cancellation race cases, new/deleted files and full conflict integration; remaining skill prerequisites, export failure recovery and snapshot lifecycle cases; bounded Boost; and the remaining protocol, desktop and isolation gates. The full change/check milestone, complete skills milestone and Boost are not claimed complete.

## Direction

Make the agent work inside the user's project and existing interface: inspect relevant structure, use selected files and annotations, propose or apply changes according to permissions, run checks, and report useful results. The systems view, editor, chat, and agent must share an accurate view of the project.

Use Codex App Server as the first execution engine. SDE owns project identity, context selection, command discovery, permissions, editor integration, persistence, and the presentation of work. Keep a small adapter boundary so another engine could be added later without rewriting those parts.

Do not build a second model/tool loop around an already-running Codex loop. Each run has one execution owner. Pi supplies useful design references now; a Pi adapter is a separate later decision.

## Evidence and the actual starting point

The primary checkout has project-scoped persistence, chat, attachment actions, annotations, and the Dioxus model selector. The integration worktree at `C:/Users/Admin/.codex/worktrees/85c3/IDE` additionally contains `src/ai.rs` and `src/ai_process.rs`. These are separate working copies; integration must reconcile their final changes before extending them.

The inspected AI slice intentionally disables shell execution, file access, and multi-agent features. Its instructions prohibit tool use, and its request handler declines execution and file-change approvals. It also uses a single active-run structure. It is a chat foundation, not yet the project agent described here. Removing those restrictions alone would leave important UI and lifecycle gaps.

Codex documents a structured App Server interface for threads, turns, streaming items, approval requests, skills, and experimental client tools. Use that protocol rather than reading CLI screen output. Validate the installed runtime's schema and supported features before relying on a method. [Official App Server documentation](https://developers.openai.com/codex/app-server).

Pi exposes extension commands and model-callable tools as distinct capabilities, and documents subprocess RPC for custom clients. Its extension code executes with the host process's permissions. Borrow the registration and lifecycle ideas; do not assume loading a third-party extension creates isolation. [Pi extensions](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/extensions.md), [Pi RPC](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/rpc.md).

Codex's source is Apache-2.0 and Pi's source is MIT. When adapting code, record the exact revision and origin, preserve required notices, and check the specific files and dependencies being copied. [Codex license](https://github.com/openai/codex/blob/main/LICENSE), [Pi license](https://github.com/earendil-works/pi/blob/main/LICENSE).

- Follow-up coverage verifies Windows path normalization for absent editor files: a captured new-file buffer survives proposal snapshot/review and explicit structural Save. Selected unsaved files remain supplied references without silently preloading their contents. This closes a skipped-buffer edge case, not the remaining Boost, merge, lifecycle or native UI gates.

- Extended typed item activity for native MCP calls, web search and collaboration calls against schemas generated by pinned Codex 0.162.0-alpha.2 (`codex app-server generate-json-schema`). MCP error payloads override a completed label and sensitive arguments/errors are omitted from activity rows. Child call completion retains reported child states rather than claiming the children finished. Unit tests cover duplicate/late events and interruption; a fake-backend protocol test recovers MCP/search completion from final snapshots. This observes delegation only; Boost caps, ownership and controls are still unimplemented. Schema generation and lifecycle reference: [App Server documentation](https://developers.openai.com/codex/app-server).

- Inspection/approval proposal tools now accept explicit `expectedRevision: "absent"` for new Windows text files and parent folders. The host verifies live absence, publishes only into the owned work copy without overwrite, keeps absence distinct from empty content, supports revision-checked subsequent edits and retains review/apply/save ownership. Snapshot and supervisor protocol tests verify creation, stale revisions, protected paths, reopening and competing live creation. Approval-mode instructions now describe the actual structural Save capability; conversation tool versions refresh for the expanded tool contract. Boost remains disabled while child ownership/cancellation/cap enforcement is being assessed against the pinned runtime.

- The opt-in real-runtime new-file proposal smoke passed using an ordinary request with no tool hints. The authenticated model invoked the proposal tool; review verified a new nested text file and no live parent folder existed before Save. The test then staged through the host API and explicitly saved the exact reviewed bytes, preserving an unrelated file. This is real backend/host evidence, not a native editor UI pass. Reproduce with `cargo test real_runtime_new_file_proposal_smoke -- --ignored --nocapture`. The pinned executable reports native `multi_agent` and `multi_agent_v2` as stable; SDE still needs verified hard limits, ownership and cancellation before exposing Boost.

- Added native child ownership tracking under `agent/swarm`, used by the active backend turn. Spawn notifications associate child/descendant identities with an observed owned parent; unrelated child turn events cannot become Stop targets. Known child turns receive interrupts before the lead, and a lead ending with observed running children retires its owned runtime. Tracking and seen-turn history are bounded; duplicate and late notifications cannot revive a known finished turn or settle a newer turn. A mock protocol test verifies owned-child cancellation and ignores an unrelated thread; unit tests cover descendants and stale events. This is cancellation groundwork, not enabled Boost: native spawn caps, early/out-of-order ownership discovery, persistence/recovery and real-runtime child cancellation still need implementation and verification.

- Child turn starts/completions arriving before their spawn notification now enter a bounded deferred queue. They become cancellation targets only after an owned-parent spawn establishes their identity. Promotion replays observed order and preserves terminal history; unrelated early events stay unowned. The cancellation protocol fixture now sends the child start before spawn and repeats it afterward; Stop still interrupts that child before the lead. Unit coverage includes completion-before-start and queue overflow. Final lead snapshots also reconcile spawn ownership before checking unfinished observed children. Nested spawn notifications arriving before parent ownership and broader native/recovery guarantees remain pending.

- Native `thread/started` metadata now supplies a second ownership path using pinned-schema `parentThreadId` or `source.subAgent.thread_spawn.parent_thread_id`. Bounded parent links can arrive before their parent; only a chain reaching the current lead promotes a thread and replays its deferred turn events. Conflicting sources, reparenting and cycles fail explicitly. Unit tests cover out-of-order descendants and unrelated roots. The Stop protocol test now runs both spawn-notification ownership and parent-metadata-only ownership; neither cancels an unrelated thread. Boost remains disabled: hard native spawn limits, policy inheritance, run recovery and real child lifecycle are not yet verified.

## 1. One project runtime

Introduce an owned Rust `ProjectRuntime` with a stable project ID and a session generation. It owns the backend connection, conversation mappings, run registry, context service, pending approvals, command registry, and any child processes or worktrees.

UI visibility is independent of ownership: hiding a panel keeps its state and does not stop a run. Closing a project cancels its runs and resolves outstanding approvals as cancelled. Switching projects initially stops the old project's work before activating the new runtime; background work across projects can be designed later.

Every message crossing the Rust/UI boundary carries project ID, conversation ID, run ID, and the relevant turn/item/request IDs. Late events from an old project cannot update a newly opened project. Replace the integration slice's single active run with an explicit run map before introducing concurrent agents.

Proposed modules:

| Module | Responsibility |
| --- | --- |
| `agent/runtime` | Project lifecycle, run ownership, cancellation, queues |
| `agent/backend/codex` | Protocol transport, schema adaptation, authentication and model catalog |
| `agent/events` | Typed events and an idempotent state reducer |
| `agent/context` | Selected resources, buffers, annotations, instruction provenance |
| `agent/policy` | Permissions, approval lifetimes, workspace boundaries |
| `agent/commands` | Shared action registry and validated dispatch |
| `agent/skills` | Discovery, validation, invocation, creation workflow |
| `agent/swarm` | Boost scheduling, child ownership, integration of results |

Avoid a generalized plugin framework in the first milestone. These boundaries can start as small modules around the existing bridge.

## 2. A real agent lifecycle

Represent runs as `queued`, `starting`, `running`, `waiting_for_approval`, `stopping`, and a terminal state: `completed`, `failed`, or `cancelled`. Connection state is separate from run state.

Render assistant output incrementally using stable message/item IDs. Tool activity becomes compact, expandable rows showing what happened and whether it succeeded. Detailed output belongs in the relevant editor or terminal tab. Keep the bottom status strip for contextual help; do not hide pending approval decisions there.

Treat request acceptance as acceptance, not completion. Reconcile terminal item/turn state, tolerate repeated events, retain partial output on failure, and do not replay a write or shell action automatically after reconnecting. A disconnected active run becomes interrupted/unknown until reconciliation establishes its state.

Stop must cancel the active turn, child work, SDE-owned tools, and their process trees. Process cleanup must be scoped to recorded ownership, never to every process with the same executable name. Esc in a popup dismisses that popup; stopping an agent uses its explicit stop control or a separately documented shortcut.

Authentication and model availability remain owned by the Codex adapter. Keep credentials in user-level credential storage/runtime home, outside `.dado`. Populate model and reasoning choices from the authenticated runtime's actual catalog. Map the current reasoning preference to a supported value explicitly; never silently send an unsupported effort.

## 3. Accurate project context

Build a bounded context manifest for each turn. It records resource identity, source, relative path, content hash or buffer revision, selected range, and why the item was included. Inputs can include the selected node, chosen files/folders, current Concept context, and comments explicitly included by the user.

Folder attachments are resource references, not permission to recursively send every file. Enumerate within limits, respect exclusions, and let the agent request relevant files. Do not preload secrets, dependency trees, generated output, or `.dado` history. An existing user selection can override an ordinary exclusion deliberately; it does not override operating-system access or authorization boundaries.

Read unsaved editor buffers through an SDE context service rather than pretending disk is current. The service must identify the buffer revision. Run instructions and project instruction files must retain provenance; file text, comments, and tool output cannot change permission settings.

The current graph is a filesystem structure. Do not label inferred architecture relationships as established dependencies. A later language-aware index can distinguish observed imports, inferred relationships, and directory containment.

For the first writing agent, prefer an isolated project snapshot/worktree with an explicit baseline. Include selected dirty-buffer snapshots there and return a patch. Applying it to the live project compares the baseline against disk and current editor revisions, preserving existing changes and surfacing conflicts. Git worktrees alone do not copy uncommitted changes; snapshot construction must handle tracked edits, untracked files, and dirty buffers intentionally.

If the provider's built-in tools bypass the editor service, isolation is required before promising buffer-safe writes. Direct live-workspace editing can come later, with buffer locking/reconciliation proven end to end.

## 4. Permissions are an execution setting

Suggested initial profiles:

| Profile | Effective behavior |
| --- | --- |
| Chat | Supplied context only; no project tools |
| Inspect | Scoped read/search; propose changes without applying them |
| Project agent | Work and run checks within an enforced project environment; review/apply changes through SDE |
| Full access | Explicit broader access, only when the runtime actually supports and enforces it |

Recommended first agent default: Inspect, followed by the isolated Project agent flow. The user can change this product decision before implementation.

Compile the selected profile into runtime sandbox settings and host-tool enforcement. Keep filesystem scope, network access, approval policy, and process execution as distinct controls internally. A confirmation dialog or prompt instruction does not implement a sandbox.

An approval names the requesting agent, operation, target, scope, and actual change/command. Bind it to a specific request and generation. Provide Allow once and Decline first; introduce broader grants only with clear boundaries. Cancelling, expiry, disconnect, or closing the project invalidates it. Pending approvals survive hiding the panel but cannot survive as executable grants after restart.

Do not use a user shell-command endpoint as the generic agent executor. The current App Server documentation explicitly distinguishes its user-initiated shell endpoint from sandboxed agent work. [Shell-command behavior](https://developers.openai.com/codex/app-server#run-a-thread-shell-command).

On Windows, validate actual sandbox availability and path behavior, including junctions, symlinks, UNC paths, casing, traversal, and new-file parents. If a promised profile cannot be enforced, report it unavailable rather than falling back to unrestricted execution.

## 5. Plus and slash open the same command system

Replace the hard-coded attachment rows with one Rust-backed registry. Clicking plus and typing slash are two entry points to the same actions, not two implementations.

Each action has a stable ID, short label, icon, slash alias, search keywords, group, argument schema, execution kind, capability requirements, permission requirements, and availability predicate. UI metadata is separate from executable handlers. Validate the selected action and arguments again in Rust at invocation time.

Distinguish three kinds of operation: a local UI action, a backend session operation, and a workflow or skill that contributes structured input to a turn. A slash command is not automatically a model tool. Do not send raw CLI command text to App Server and assume it will execute as a command.

Suggested interaction contract:

| Input | Behavior |
| --- | --- |
| Click plus | Open the action panel above the composer, focused on its search field |
| Type `/` at the beginning of a composer line | Open the same panel; characters after slash filter actions |
| Type more text | Search labels, aliases, then keywords; keep ordering stable |
| Up / Down | Move the active result, scroll it into view, skip disabled results |
| Enter while panel is open | Invoke the active result; never also submit the chat |
| Enter with no matching action | Keep the panel open; do not send an accidental message |
| Escape | Dismiss the panel and preserve the draft |
| Tab | Leave/dismiss the panel without sending or invoking |
| Shift+Enter | Preserve multiline input behavior |

Slash completion must respect IME composition, pasted text, code blocks, URLs, escaped slashes, and filesystem paths. Use a defined slash-query span; replacing that span must preserve the rest of the draft, attachments, selection, and undo history. After Escape, ordinary text entry remains possible. Unknown slash text is never converted into a shell command.

For slash invocation, keep keyboard focus in the composer with accessible active-result information. For plus invocation, use the panel search field. Pointer selection and keyboard selection dispatch the same action ID. Results show icon + label; contextual descriptions go to the bottom strip. The panel scrolls within a maximum height and never resizes the systems view.

Ship only completed actions. Hide unavailable backend features; when a discovered item has a resolvable missing prerequisite, it can appear disabled with the reason in the status strip.

Proposed action catalog, released progressively:

| Label | Alias | Intended operation |
| --- | --- | --- |
| Add files / Add folders | `/file`, `/folder` | Existing native pickers |
| Models | `/model` | Open the existing Dioxus selector |
| Permissions | `/permissions` | Open effective execution settings |
| New chat | `/new` | Create a project conversation |
| Skills | `/skills` | Browse and select discovered skills |
| Create skill | `/skill-create` | Open the skill creation workflow |
| Boost | `/boost` | Enable/disable bounded delegation for this chat |
| Agent activity | `/agents` | Inspect parent and child progress |
| Review changes | `/diff` | Open actual proposed/live changes |
| Compact context | `/compact` | Invoke supported history compaction |
| Model Settings | `/model-settings` | Open the Model settings page |

CLI command lists are useful interaction references, but each SDE action requires a supported backend or local handler. Model, permissions, and agent controls remain meaningful during an active turn: local views can open immediately; changes that affect execution apply at the next safe boundary and show whether they are pending. [Codex command reference](https://developers.openai.com/codex/cli/slash-commands).

## 6. Skills: create, discover, invoke

Use the standard skill directory shape: `SKILL.md` with name/description, plus optional references, scripts, and assets. Codex uses metadata first and loads full instructions when needed; repository `.agents/skills` is a conventional location. [Official skill documentation](https://developers.openai.com/codex/skills).

Offer project-local skills in `.dado/skills/<name>/` by default, consistent with the user's private project state. Register that explicit root with the adapter when supported. Offer an explicit export/publish operation to `.agents/skills` for version-controlled team sharing. User-wide skills belong in user storage. Do not change global configuration silently to make project skills work.

Use canonical resource IDs incorporating scope and resolved path, rather than skill name alone. Detect collisions and malformed metadata. Show missing prerequisites through status help. Discovery must honor the runtime's actual roots and change notifications, with explicit refresh as a backend operation rather than another permanent toolbar button.

Selecting a skill inserts a structured skill reference into the next message, shown as a compact chip. Enter in the actions panel selects the skill; it does not immediately send the message. Resolve the exact path and revision at send time and record what was used. Skill activation never grants additional permissions.

Create skill opens a small workflow asking what it should do, when it should run, and its scope. The agent drafts it in an editor tab, validates metadata and referenced paths, and shows a concrete reviewable result before publication. Start with instruction-only skills; scripts require the existing execution policy. Creating a script is distinct from running it. Edits made to an active skill apply to a subsequent run, not midway through a running tool call.

Use the backend's discovery/invocation support where available. If an experimental API is required, gate it on the pinned runtime version. Do not pretend a directory becomes usable merely because SDE can list it.

## 7. Boost: bounded agent delegation

Proposed meaning: Boost allows the current lead agent to divide a sufficiently independent task among child agents. It is a per-chat execution preference, visibly active while enabled. It does not mean higher reasoning, a faster service tier, or broader permissions.

Codex documents native subagent support and configurable concurrency. Pi describes subagents as something to add through customization rather than an assumed core feature. Use Codex's native delegation first when the pinned App Server exposes the lifecycle and controls SDE needs. [Codex subagents](https://developers.openai.com/codex/multi-agent), [Pi project](https://github.com/earendil-works/pi).

Suggested first limit: one lead and up to two active children, with one delegation level. Count every descendant against a hard cap. Start by supporting parallel inspection/review and one writer. Broader parallel editing comes after isolated change integration works.

The lead defines a child objective, required inputs, scope, expected result, and whether it can write. Children inherit the project's effective permissions and explicitly resolved model/skills settings. They receive relevant context rather than an uncontrolled copy of every conversation. Only the lead integrates the answer shown in the main chat.

Keep one delegation owner. If Codex owns the child tree, SDE observes and controls that tree instead of spawning a competing second tree. If native delegation cannot satisfy SDE's ownership/cancellation requirements, prototype an SDE-owned scheduler behind the adapter; explicitly disable native delegation for that mode.

For independent writers, assign separate worktrees/snapshots and a known baseline. Return patches with provenance, integrate serially, check overlapping changes, then run checks on the integrated result. Tests passing separately do not establish that combined changes work. In non-Git projects, initially permit parallel readers and one writer; temporary snapshot isolation can be added explicitly later.

Boost off prevents new child work. Existing children stop spawning descendants and either drain or are cancelled through a visible Stop all action. Stopping the lead stops the owned tree. Child failure does not discard another child's work or trigger unlimited retries. Enforce concurrency, tool timeouts, and available usage limits; unavailable token/cost totals must be shown as unavailable rather than fabricated or claimed as hard limits.

Present a compact activity indicator that opens the child list, with objective, state, changes, and checks. Keep ordinary replies readable. Do not crowd the filesystem constellation with agent nodes in the first version; that is a separate design discussion.

## 8. Persistence and recovery

Keep a versioned project manifest under `.dado`, preserving existing chat IDs, comments, drafts, Concepts, and layout. Store references to backend thread/session IDs, effective execution settings, skill references, and Boost preferences. Use append-only run events or separate run files once volume makes rewriting all chat state inappropriate; retain atomic manifest writes.

Persist operation identity and outcomes, not executable approval grants, credential tokens, or an instruction to rerun a previous side effect. On restart, reconcile backend state and mark unresolved work interrupted. Reconnect and resume require matching project identity and current policy. Never silently attach a conversation to a different project because a path was reused.

Worktrees/snapshots are lifecycle resources, not chat messages. Keep their ownership and recovery metadata. Cancel processes before cleanup; preserve unresolved patches and conflicts. Only remove a recorded owned workspace after verifying its resolved target and recoverability.

## 9. Delivery sequence and completion gates

1. **Reconcile the existing AI slice.** Preserve primary UI and branding; bring in the worktree's authentication, catalog, streaming, and stop behavior. Gate: an authenticated request completes or fails honestly, without losing project state. An unauthenticated catalog stays empty.
2. **Shared actions and slash search.** Implement registry, keyboard contract, and current local actions first. Gate: plus and slash invoke identical handlers; Enter cannot accidentally send; no path/IME regressions.
3. **Inspection agent.** Add project context, selected dirty-buffer snapshots, scoped read/search, and visible tool lifecycle. Gate: answer an actual project question using observable reads, with no unauthorized writes.
4. **Change-and-check agent.** Add isolated execution, approval requests, patch review/apply, and test output. Gate: implement one small change, preserve unrelated dirty edits, detect an external edit conflict, and stop a running subprocess tree.
5. **Skills.** Discovery, explicit invocation, creation in an editor, validation, and publication. Gate: create a skill, reopen the project, discover and invoke it, and verify it cannot widen permissions.
6. **Boost.** Native child lifecycle first, strict caps, parallel readers, then isolated writers. Gate: complete a two-part task, integrate results, surface a conflict, and cancel all owned work without touching unrelated processes.
7. **Further actions and engines.** Add review/compaction/session operations only where supported; consider Pi or other engines after the common contracts are stable.

The first milestone that qualifies as a useful agent is inspection plus a controlled change/check cycle. Boost depends on that foundation; it is not a substitute for it.

## 10. Verification that matters

Use deterministic fake-backend protocol tests for fragmented JSONL, concurrent requests, stale events, duplicate terminal events, approval cancellation, disconnect, and restart reconciliation. Add versioned protocol fixtures and an opt-in real-runtime smoke test; never require live credentials in routine tests.

Exercise action keyboard navigation, selection during streaming, empty results, escaped slashes, paths, IME, and multiline drafts. Verify UI state survives panel toggles and project changes. Use native checks for the desktop experience; respect the user's reservation of Playwright for the other chat.

Use disposable projects for path boundaries, dirty buffers, overlapping patches, non-Git folders, symlinks/junctions, and stopped process trees. For Boost, verify caps under nested spawn attempts, child failures, lead cancellation, and partial integration. Show what was actually verified before calling a milestone complete.

## Decisions to settle while discussing the plan

- Whether Inspect should be the first default, and whether the first writing agent always returns a patch for review.
- Whether Boost is sticky for a chat or enabled for one turn; this draft recommends per-chat, off by default.
- Initial child concurrency and whether per-child model selection belongs in the first Boost UI; this draft recommends two children using the selected model unless configured otherwise.
- Whether private `.dado/skills` plus explicit team export matches the user's intended skill sharing.
- Distribution and runtime pinning: detect a compatible installed Codex first, or ship a managed runtime. Do not silently depend on another application's private executable layout as the long-term distribution contract.

These choices do not block writing the plan. They should be resolved before the corresponding feature becomes active.
