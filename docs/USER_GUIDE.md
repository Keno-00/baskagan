# User guide

## Start and open a project

A minimal Rust desktop application using Dioxus, with a white start screen and two centered, borderless actions.

- **Create Project** asks for a name and a parent folder, creates the project folder, and opens it.
- **Open Folder** opens an existing folder through the system folder picker.

Opening a folder displays its contents as a downward constellation of gray nodes on white.

The icon controls switch between **Folders** and **Mental model**. Folders retains current-level and full-overview modes; distant overview labels appear on hover, selection, or closer inspection, and generated/dependency trees are omitted from the overview.

**Future TODO — System diagram:** disabled for now, pending an agreed design. Preserve the existing implementation for future reuse.

## Navigate the systems view

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

## Chat and project workspace

Chat sits below the systems view. Hover near their shared edge and click the dots to choose a ¾ systems, balanced, or ¾ chat layout. Enter sends and Shift+Enter adds a line. The composer grows up to half the chat area, then scrolls without reducing the systems view.

The welcome screen shows only Create Project and Open Folder. Chat and side panels belong to an open project. The plus button opens searchable actions, including files and folders. Typing `/` at the beginning of a composer line opens the same actions; arrows select, Enter invokes, and Escape closes without removing the draft. Conversations, drafts, attachment references, comments, and view settings are saved in the project's `.dado/state.json`. New Chat and the conversation list live in the left panel. Opening another project loads its own state. When a project already has a `.gitignore`, the app adds `/.dado/` while preserving existing rules; it does not create an ignore file when none exists.

## Annotations

Select text in a chat message to add an annotation. Numbered markers show the comment on hover and open it for editing or deletion when clicked. In the editor, select code or click a line and use the comment icon in the right margin. Commented code stays highlighted and its range follows edits. The chat input's comment icon and count opens a scrollable list above the input, showing compact text previews or code line ranges followed by the quoted, italic comment. Annotations are saved with the project and do not modify the source file.

Saved comments automatically enter the composer as removable chips and can be sent without typing a message. Sending captures their quotes and feedback, then clears the pending badge and chips while retaining the original annotations. Sent comments stay cleared after reopening; editing a comment queues its updated version. Removing a draft chip omits that version without deleting its annotation. Including a code comment does not automatically read its file or grant permission to change it.

## Settings and motion

User preferences save automatically in `%USERPROFILE%/.dado/sde/settings.json` (or `$HOME/.dado/sde/settings.json`). General, Personalization, model/reasoning, permission mode and Appearance are shared across projects. General controls Systems visibility, automatic incremental project understanding, and reopening the last project on startup. Its warning-marked Rebuild button is the only action that starts understanding over; ordinary updates retain established concept names and IDs. Unchanged saved models reopen without another model run. Activity appears on the right of the bottom strip and gently animates the shared background, respecting reduced motion. An explicit project passed at launch takes priority. Appearance includes system/light/dark mode, font, text size and reduced motion. Chats, concepts and workspace state remain in the project. Existing project preferences are imported only when no user settings file exists.

Appearance → Motion controls background drift, intensity, navigation responses, agent activity, node changes, and system reduced-motion preferences. Particles spawn and decay within a bounded population, and navigation impulses settle with inertia. Confirmed source reads attract particles and pulse the corresponding node; edits and model updates animate locally while labels stay still. Hidden windows pause animation.

## Relationship explanations and navigation

Hold a Systems node for half a second (or use Alt+Enter) to reveal its relationships and open a deeper source-backed explanation in the side lane. Release restores the graph while keeping the explanation available. Hold for one second to pin the relationship lines, confirmed by a subtle flash. Panel interactions preserve the pinned view; selecting a related node scrolls to and briefly highlights its relationship explanation. Click the canvas, press Escape, or close the explanation to release it. The small model runs only on demand; verified explanations persist in the project’s `.dado/node-insights.json` and refresh when their supporting sources or relationships change. Existing terminology remains authoritative, and failed refreshes preserve the previous text. Terminal status messages clear after three seconds.

In the Systems canvas, Ctrl+scroll up/down zooms in/out around the cursor. Node labels scale with the zoom, with a readable minimum and a maximum size to limit crowding. Ordinary scrolling still traverses hierarchy levels, and Ctrl+plus/minus keeps its existing behavior.

Level navigation follows the selected branch with damped travel and expanding or contracting connections. Space-drag along a branch to move into its next level, or drag downward to return to its parent; smaller and sideways drags keep ordinary panning. The current zoom is preserved.
