# How we work together

These instructions come from working with this user. Carry that shared understanding forward, and update it when the user teaches us something new. Explicit user instructions take precedence.

## Our rhythm

Talk about an idea, build a small version, let the user experience it, listen closely, and refine it together.

- Let an idea breathe before building it. The user sometimes wants to explore how something should feel. Stay in that conversation until they ask for implementation. Do not turn a question into code changes or rush ahead when asked to wait.
- Follow the user's pace. Make one small piece real and let experience guide what comes next. Do not grow a small request into a larger product plan.
- Take initiative within the direction we have agreed on. Carry requested work through without repeated permission questions. Leave changes to the product's direction in the user's hands.
- Keep the conversation natural. Speak plainly, explain decisions briefly, and leave room for the user to steer. Do not turn every exchange into a formal plan or a technical checklist.

## How to listen and refine

- Learn the reason behind a correction. When the user dislikes a control, understand whether the problem is clutter, obstruction, unnecessary wording, meaning, or feel. Apply that understanding to related decisions without inventing new scope.
- Treat what feels right as part of the feature. Alignment, contrast, movement, and placement matter alongside functionality. Do not defer them as optional finishing touches.
- Keep meaning precise. An identifier and a count can look similar while serving different purposes. Understand what information the user wants before choosing its presentation.
- Build shared understanding over time. Corrections should change how we work, so the user does not have to keep explaining the same preference. Preserve accepted choices unless the user changes direction.
- Make the result available to experience. After implementing a requested change, verify it and make it ready for the user to try. Describe what changed briefly and be honest about anything still uncertain.

## The taste we have established

- Keep interfaces quiet. Show what is useful now, and avoid unnecessary text, decoration, and duplicate controls.
- Prefer rounded surfaces and spacing to establish structure. Avoid outlines, borders, and shadows unless they solve a real usability problem.
- Use deliberate, restrained contrast. Keep Light and Dark panels neutral and close in lightness so they feel like one continuous interface; use hue for focus and selection cues, plus the established user-message bubbles and file chips. The user strongly dislikes stark panel blocks and broad tinted panel fills. Keep text readable without making neighboring surfaces compete.
- Favor icons for familiar actions. Keep secondary actions out of the way until hover, focus, or selection makes them relevant. Keep them keyboard accessible and use text when an icon would be unclear.
- File and folder attachments use a horizontal card strip above the composer draft, with image previews or file/folder icons, short names along the bottom, and removal on hover or keyboard focus. Let the composer expand to fit the strip; keep long drafts scrolling inside it. Keep annotation and idea summaries distinct from attachment cards.
- Keep visible information short and meaningful. Use concise identifiers, counts, ranges, and truncated previews. Put contextual help and additional information in the bottom tooltip/status strip rather than persistent explanatory text or floating tooltips.
- Keep actions clear of the content. Controls should not obstruct reading, selection, or editing.
- Keep layouts stable. Growing content should scroll within its space rather than unexpectedly pushing other views around. Hiding and restoring a view should preserve the user's work.
- Make movement gentle and understandable. Use smooth easing and soft transitions where they help the user follow a change.
- Respect familiar interaction habits. The user should not have to relearn ordinary keyboard behavior to use the interface.
- Build features that actually work. Do not introduce pretend functionality or choices that imply capabilities we have not implemented.
- Keep migration deliberations in chat where the user can annotate them. Explicit approval of a part authorizes implementation; comments expressing criticism or uncertainty keep that part under discussion. Capture the existing UI before migrating it and use that reference during verification.
- In Full access, prompting is the user workflow: the agent changes live files and runs checks automatically, with no proposal staging or Apply step. Keep explicit review/application in modes that require it.
- Put Apply directly on the message’s changes row. Offer optional inline diffs rather than requiring a popup just to apply.

## UI foundation: Dioxus owns the interface

- Systems currently exposes two icon-only views: folder hierarchy (current level/full overview) and mental model. System diagram is disabled as a future TODO until the user agrees its design; preserve its implementation but do not expose or re-enable it. The mental model follows concepts from project purpose down to functions; it must not merely reproduce the folder tree. Reveal finer detail deliberately.
- Generated concepts populate the project's left-sidebar Concepts and select the matching chat context. Source links open a whole file or its exact associated range in the right editor. Keep descriptions selectable and persistent after clicking.
- Project explanations use the configured small model in the background, saved under the project's `.dado/`; do not surface that operation as chat messages. Validate references and return concrete errors to the small model for correction. Refresh after source edits while preserving stable concept IDs and user notes. Chat receives focused concept context, not the entire graph. Ideas remain proposals rather than generated facts.

- All ordinary UI must render through Dioxus: chat and Markdown content, panels, tabs, buttons, menus, dialogs, toolbars, settings, file browsers, title bar, and window controls. A Dioxus host containing JavaScript-built controls is not a migration.
- JavaScript is allowed for specialized widgets and browser APIs: CodeMirror, xterm.js, systems scene interaction, selection/range highlights, clipboard, focus, and geometry. Keep their surrounding UI and visible state in Dioxus. Controllers may provide data and invoke browser capabilities; they must not construct ordinary controls with DOM builders or HTML strings.
- Do not introduce another UI framework or a parallel set of controls. If a new capability appears to require an exception, explain the limitation in chat before changing the agreed foundation. Difficulty or convenience alone does not justify an exception.
- Reuse `src/ui_controls.rs` and the shared values in `src/ui_controls.css`. Extend the shared control when a reusable fix is needed; do not solve the same alignment or interaction problem independently in each panel.

## Compactness, alignment, and symmetry

- The right side panel is the reference for proportions, density, and control treatment. Carry its visual language into other surfaces while preserving their functions.
- Use the shared font, type scale, control heights, icon sizes, spacing, and radii. Current desktop defaults are 12px text, 11px secondary text, 30px controls, 24px small controls, 16px icons, 14px small icons, 4px gaps, 8px spacing, and 8px control radii. Read the tokens rather than duplicating these numbers; respect Appearance overrides. Larger content surfaces may use the established larger radii.
- Keep the desktop compact without crowding text or making targets unreliable. Generic mobile sizing recommendations do not override the approved desktop proportions.
- Center icons in their complete clickable and ghost/hover surfaces on both axes. Use flex/grid centering and consistent SVG view boxes, stroke weights, and dimensions. Do not compensate with arbitrary margins, transforms, or per-icon offsets.
- Align related labels, text baselines, icons, fields, and action groups to shared rows and edges. Equivalent controls must have equal heights and consistent padding. Match opposite insets and gaps where the layout is symmetrical; balance unequal content without forcing every layout to mirror itself.
- Settings use labels on the left and controls on the right, with consistent control widths and row spacing. At narrow widths, reflow deliberately rather than clipping or squeezing controls.
- Keep hover/focus actions within reserved space so they do not move text or neighboring controls. Menus and pickers must not enter normal layout briefly or cause a jump when opening.

## Polish is part of correctness

- Use theme values for surfaces, text, and interaction states. Check light, dark, and system modes, including active tabs, their entire background, close buttons, hover/selected/disabled states, fields, menus, dialogs, and specialized widget surroundings. No accidental white surfaces in dark mode.
- Treat theme construction as an HSL profile: hue is the user/theme seed; keep structural surfaces neutral with low saturation (Primary 0.04×, Secondary 0.05×, controls 0.08×, selected 0.12×, foreground 0.02×). Use a restrained accent (0.35×) for focus, selection markers, small status cues, user-message bubbles, and file chips; keep semantic status hues independent and never use accent hues for full panel fills. Define paired Light/Dark lightness ramps with small surface steps, role-specific floors, and contrast corrections; never mechanically use `100% - L`. Keep Primary near white in Light mode and Secondary panels close to it as soft neutrals; keep Dark surfaces close in value while retaining readable text. Validate final text contrast after the chosen hue and role saturation are applied. Muted text follows its owning surface’s contrast ramp; a global muted value must not be reused on a differently toned panel. Page role determines Primary versus Secondary ownership: the title bar and full-page main surfaces use Primary; Secondary panels and their controls derive their colors from their owner’s tokens.
- Keep readable text contrast with a restrained surface hierarchy. Prefer neutral, closely spaced panel values and minimal accents; avoid stark panel blocks, saturated broad fills, and nearly indistinguishable text. Do not use color alone to communicate state.
- Familiar actions use the shared icon style with accessible names. Secondary actions revealed on hover must also appear on keyboard focus. Preserve visible focus, logical tab order, Escape dismissal, menu arrow keys, and return focus to the originating control or editor.
- Preserve native text selection, undo/redo, editor/terminal state, drafts, expanded details, and scroll position through ordinary updates. Streaming follows the bottom only while the user has not scrolled away. Long content scrolls inside its allotted area.
- Use gentle transitions without layout jerks, and honor reduced motion in CSS and JavaScript animations. Scrolling chat content fades softly at both top and bottom; controls stay clear of reading and selection areas.
- Keep annotation summaries separate from message bubbles. Pending counts describe the current draft, clear after sending, and leave a single summary associated with the sent message.
- Settings save automatically in the user's `.dado/sde/` folder. Do not make global preferences depend on the currently open project or silently replace malformed saved data.
- Every new Settings preference must use `src/user_settings.rs` (`Settings`, defaults, validation, load/save) and the shared `updatePreferences` flow in `ui/chat.js`, saving to `<user home>/.dado/sde/settings.json`. Wire both saving and startup loading, and extend the persistence check when adding a preference. Do not use project storage or browser localStorage as the source of truth for Settings preferences. Provider-managed authentication remains with its provider.
- Model Settings contains Providers and Defaults. Chat picker choices are per conversation: save model and reasoning with the conversation in the project's `.dado/` storage and restore them on reopening. New conversations start from the user defaults; chat picker changes never rewrite those defaults. Small-model defaults are saved for later SDE use.
- Before a migration, capture the current UI. Before delivery, inspect the actual Dioxus desktop result against that reference at the relevant window sizes and themes. Check icon/ghost centering, padding symmetry, text alignment, overflow, and opening/closing transitions as well as functionality. Exercise affected native workflows; do not substitute a static HTML imitation. Report anything not verified honestly.
- Use the `ui-ux-pro-max` skill for UI work when available, applying guidance to this desktop context. The user's approved design, repository rules, and working controls take precedence over generic style or sizing recommendations.

- Use compact labeled badges for annotations and ideas in both the chat composer and sent messages. Use numeric count bubbles for active annotations and ideas in the systems view, with a small circled comment/lightbulb icon overlapping the top-left. Preserve active text-selection markers. Do not turn chat summaries into bubbles.
- Dispose of temporary scripts, test fixtures, screenshots, preview binaries, and disposable WebView profiles after verification. Dedicated folders are not a substitute for cleanup. Preserve source, intentional deliverables, normal build caches, and anything still in use; do not remove another chat’s work.
- If the project executable is running and blocks a rebuild, stop that project process (`pkill`, or Windows `Stop-Process`) and rebuild the normal executable. Do not work around the lock by creating extra preview executables or disposable profiles. Scope process termination to this project's executable; preserve unrelated processes. Send temporary artifacts to the Recycle Bin after verification.

- Systems modes reuse one scene and its existing navigation, camera, selection, overview/reset controls, transitions, and animated background. Switching folder, workflow, or mental-model mode changes graph data and behavior flags; do not build separate canvases or disable the background.
- During project understanding, let real streamed concepts and connections form progressively in the shared scene. Keep unvalidated output provisional and out of saved concepts/chat context. Generate breadth across responsibilities and evidence-backed cross-connections as well as depth; do not stop at a small illustrative graph.

- Concept information belongs in a reserved side lane, clear of the graph. Keep the scene geometry stable across hovering, selecting and dismissing information, and keep previews available when moving into their source links.

- Treat established mental-model terminology and concept IDs as part of the user’s learned map. Ordinary source updates preserve them and unaffected explanations; unchanged projects do not spend model tokens again. Full re-understanding is available only from the warning-marked General settings action. Activity belongs on the right of the bottom strip, separate from contextual hints, and shares gentle, reduced-motion-aware background reactions.

- Holding a Systems node reveals its neighborhood and opens an on-demand explanation in the existing side lane. A one-second hold pins the relationship lines with a subtle confirmation flash; keep panel interactions from dismissing it. Selecting a related node scrolls to and highlights that relationship for half a second; clicking the canvas disengages. Keep it usable after release, consume the release click, cancel pending holds on drag, and support Alt+Enter. Reuse verified node explanations until their sources or relationships change; preserve terminology and previous text during refresh. Bottom-right completion, pause and failure messages are brief notifications, never persistent labels.

- Motion belongs in Appearance preferences. Use bounded particle birth/decay, velocity and damping for inertia, loose uneven gathering around activity, and confirmed source events for agent effects. Keep node labels and hit targets anchored; respect reduced motion and suspend hidden-window animation.

- Background rotation felt nauseating. Keep the atmosphere free of whole-scene rotational sway; respond to spinning with a small, capped lateral drift that settles with inertia.

- Systems navigation refinements must preserve its appearance. Travel between levels follows the chosen connection with damped inertia and meaningful scale: ancestors move above the focus and descendants spread below it. Keep the upper connection aimed at the parent's retained position, rather than replacing it with a vertical line. Space-drag can continue into an adjacent level along that rail; preserve ordinary short and sideways panning, cursor zoom, and reduced motion.
