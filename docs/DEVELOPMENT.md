# Development

## Run

    cargo run

You can also open a folder on launch with `cargo run -- "C:\path\to\project"`.

## Build

    cargo build

Windows requires the Microsoft C++ build tools and WebView2 Runtime.

The bundled editor (CodeMirror) and terminal (xterm.js) assets are checked in, so running the desktop app does not require Node.js or network access. After editing `ui/workbench.js`, regenerate them with `npm ci` and `npm run build`, then rebuild the Rust app.

The first editor supports UTF-8 text files up to 5 MB, preserves a UTF-8 BOM and CRLF endings, and rejects saves when the file's text has changed on disk since it was loaded. Panel state lasts for the current app session.

The window setup and root UI component are in `src/main.rs`.

## Quality checks

The GitHub Actions quality workflow runs these checks on Windows:

    cargo test --locked
    cargo clippy --locked -- -D warnings
    cargo build --locked
    npm ci
    npx playwright install chromium
    npm test

For the smaller standalone JavaScript checks:

    node tests/scene.test.cjs
    node tests/commands.test.cjs
    node tests/review.test.cjs
    node tests/comment_context.test.cjs

An opt-in real-runtime inspection check sends one model request using the existing SDE login. It searches and reads a disposable project's unsaved editor snapshot and verifies that disk stays unchanged:

    cargo test --bin dado-sde real_runtime_inspection_smoke -- --ignored --nocapture

Optional browser interaction checks use Playwright: run `npm ci`, then `npm test`. Install Playwright's Chromium browser first, or set `DADO_TEST_BROWSER=chrome` to use an installed Chrome browser. The full Playwright suite exercises browser interactions and the native Windows/WebView2 app.
