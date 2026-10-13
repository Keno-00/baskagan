# User guide

Use this guide in the order that matches your work. First open a project, then choose a view to understand it. From there, follow a concept to its related code, or open chat when you want to discuss that focused part. The [AI and agent workflows guide](AI_AND_AGENT_WORKFLOWS.md) explains what context chat receives and how proposed changes are reviewed.

## Start and open a project

When SDE starts, choose **Create Project** to make a project, or **Open Folder** to explore one that already exists. Create Project asks for a name and parent folder, then creates and opens the new folder. Open Folder uses the system folder picker. After a folder opens, its contents appear as nodes in the Systems view.

Use the view controls to choose **Folders** or **Mental model**. Folders shows the directory hierarchy; it has current-level and full-overview modes. In a full overview, labels appear when you hover, select, or inspect more closely. Generated and dependency trees are omitted there so the project structure stays easier to read.

**System diagram:** not available yet; its design is still under discussion.

## Navigate the systems view

Start from the selected folder, then follow its connections to move through the hierarchy. A single click sets a folder as the navigation anchor; double-clicking enters it. The parent connection above the current folder goes up one level. Scrolling also moves between levels: scroll down toward the project root, or scroll up to enter the selected folder. If nothing is selected, scrolling up enters the immediate folder nearest the pointer.

- Use **Ctrl +** and **Ctrl −** to change scale. Drag to rotate around the vertical axis and Shift-drag to pan.
- The hierarchy icon switches between the current folder and an expanded overview. The curved-arrow icon restores the camera.
- Collapsed folders with contents have downward continuation lines. Connections and nodes fade near the viewport boundaries.
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

Once the project is open, chat and side panels become available. The Systems view keeps the project map visible while you work in chat.

Chat sits below the systems view. Hover near their shared edge and click the dots to choose a ¾ systems, balanced, or ¾ chat layout. Enter sends and Shift+Enter adds a line. The composer grows up to half the chat area, then scrolls without reducing the systems view.

The welcome screen shows only Create Project and Open Folder. Chat and side panels belong to an open project. The plus button opens searchable actions, including files and folders. Typing `/` at the beginning of a composer line opens the same actions; arrows select, Enter invokes, and Escape closes without removing the draft. Conversations, drafts, attachment references, comments, and view settings are saved in the project's `.dado/state.json`. New Chat and the conversation list live in the left panel. Opening another project loads its own state. When a project already has a `.gitignore`, the app adds `/.dado/` while preserving existing rules; it does not create an ignore file when none exists.

## Annotations

Use an annotation to keep a note attached to the exact text or code that prompted it. The note stays with the project and does not change the source file.

1. **Select what you want to discuss.** In chat, select text in a message. In the editor, select code or click a line.
2. **Add the comment.** In chat, use the marker that appears for the selected text. In the editor, use the comment icon in the right margin. Numbered markers show chat comments on hover and open them for editing or deletion when clicked. Commented code stays highlighted, and its range follows edits.
3. **Review pending comments.** The comment icon and count in the composer open a scrollable list above the input. It shows a text preview or code line range, followed by the quoted text and comment. Saved comments enter the composer as removable chips and can be sent without a message.
4. **Send or adjust them.** Sending includes the quotes and feedback, then clears the pending badge and chips while keeping the original annotations. Sent comments stay cleared after reopening. Editing a comment queues its updated version. Removing a draft chip leaves the annotation saved, but omits that version from the message.

Including a code comment does not automatically read its file or give permission to change it.

## Settings and motion

SDE saves personal preferences separately from project understanding. The settings file belongs to your user account; conversations, concepts, and workspace state belong to each project.

User preferences save automatically in `%USERPROFILE%/.dado/sde/settings.json` (or `$HOME/.dado/sde/settings.json`). General, Personalization, model/reasoning, permission mode, and Appearance settings are shared across projects. General controls Systems visibility, automatic incremental project understanding, and reopening the last project on startup.

To rebuild project understanding from the start, use the warning-marked **Rebuild** action in General. This is the only action that starts over; ordinary updates keep established concept names and IDs. If a saved model has no source changes, SDE reopens it without another model run. An explicit project passed at launch takes priority over reopening the last project.

Appearance includes system/light/dark mode, font, text size, and reduced motion. Activity appears on the right of the bottom strip and gently animates the shared background while respecting reduced motion. Chats, concepts, and workspace state stay in the project. Existing project preferences are imported only when no user settings file exists.

In **Appearance → Motion**, adjust background drift and intensity, navigation response, agent activity, node changes, and the system reduced-motion setting. Particles appear and fade within a limit; navigation settles with inertia. When SDE confirms a source read, particles move toward the related node and the node pulses. Edits and model updates animate near their nodes while labels stay still. Hidden windows pause animation.

## Relationship explanations and navigation

To inspect why two parts of the project are connected, hold a Systems node for half a second or press **Alt+Enter**. SDE opens a source-backed explanation in the side lane and shows the node’s relationships. Release the node to restore the graph while keeping the explanation open.

To keep the relationship lines visible while using the panel, hold the node for one second. A brief flash confirms that the lines are pinned. Select a related node to scroll to its explanation and highlight that relationship. Click the canvas, press **Escape**, or close the explanation to release the pinned view.

The small model runs only when you ask for an explanation. Verified explanations are saved in the project’s `.dado/node-insights.json` and refresh when their supporting source or relationships change. Existing terminology stays in place during refresh; if refresh fails, SDE keeps the previous explanation. Terminal status messages clear after three seconds.

In the Systems canvas, Ctrl+scroll up/down zooms in/out around the cursor. Node labels scale with the zoom, with a readable minimum and a maximum size to limit crowding. Ordinary scrolling still traverses hierarchy levels, and Ctrl+plus/minus keeps its existing behavior.

Level navigation follows the selected branch with damped travel and expanding or contracting connections. Space-drag along a branch to move into its next level, or drag downward to return to its parent; smaller and sideways drags keep ordinary panning. The current zoom is preserved.
