# DADO IDE

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
```

Optional browser interaction checks use Playwright: run `npm ci`, then `npm test`. Install Playwright's Chromium browser first, or set `DADO_TEST_BROWSER=chrome` to use an installed Chrome browser.
