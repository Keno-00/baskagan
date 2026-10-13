# Development

Use this guide to run SDE, rebuild its checked-in web assets when needed, and run the same quality checks as CI. Start with the run instructions. If you change JavaScript assets, follow the asset step before rebuilding the Rust app. The [user guide](USER_GUIDE.md) explains the app workflows; the [AI and agent workflows guide](AI_AND_AGENT_WORKFLOWS.md) explains project understanding and change review.

## Run

    cargo run

To open a project as soon as SDE starts, pass its folder after `--`:

    cargo run -- "C:\path\to\project"

## Build

    cargo build

Before you build on Windows, install the Microsoft C++ build tools and WebView2 Runtime.

The editor (CodeMirror) and terminal (xterm.js) assets are checked into the repository. Because of that, running the desktop app does not require Node.js or network access. If you edit `ui/workbench.js`, regenerate the bundled assets first, then rebuild the Rust app:

    npm ci
    npm run build
    cargo build

The first editor supports UTF-8 text files up to 5 MB, preserves a UTF-8 BOM and CRLF endings, and rejects saves when the file's text has changed on disk since it was loaded. Panel state lasts for the current app session.

The window setup and root UI component are in `src/main.rs`.

## Quality checks

Before opening a pull request, run the checks used by the GitHub Actions **quality** workflow. CI runs them on Windows in this order:

    cargo test --locked
    cargo clippy --locked -- -D warnings
    cargo build --locked
    npm ci
    npx playwright install chromium
    npm test

To run only the smaller standalone JavaScript checks, use:

    node tests/scene.test.cjs
    node tests/commands.test.cjs
    node tests/review.test.cjs
    node tests/comment_context.test.cjs

There is also an opt-in check for the real Codex runtime. It sends one model request using the existing SDE login, then searches and reads an unsaved editor snapshot in a disposable project. The check verifies that the file on disk stays unchanged. Run it only when you want to make that request:

    cargo test --bin dado-sde real_runtime_inspection_smoke -- --ignored --nocapture

The full Playwright suite checks browser interactions and the native Windows/WebView2 app. To run it, install dependencies, install Playwright's Chromium browser, then run the tests. If Chrome is already installed, set `DADO_TEST_BROWSER=chrome` instead of installing Chromium:

    npm ci
    npx playwright install chromium
    npm test
