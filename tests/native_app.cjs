const { chromium } = require('playwright');
const fs = require('node:fs');
const path = require('node:path');
const net = require('node:net');
const { spawn } = require('node:child_process');

// Exercise the actual Dioxus event handlers and native widget hosts. A static
// HTML imitation of the shell would miss mount, focus and persistence bugs.
async function nativeApp(options = {}) {
    if (process.platform !== 'win32') throw new Error('Native desktop UI checks currently require Windows/WebView2.');
    const executable = path.resolve(process.env.DADO_TEST_EXE || 'target/debug/dado-sde.exe');
    if (!fs.existsSync(executable)) throw new Error(`Build the desktop app first: ${executable}`);
    const fixture = fs.mkdtempSync(path.resolve('target/native-ui-'));
    const project = path.join(fixture, 'project');
    fs.mkdirSync(project);
    fs.writeFileSync(path.join(project, 'main.rs'), 'fn main() {\n    println!("hello");\n}\n');
    fs.writeFileSync(path.join(project, 'README.md'), '# Sample\n');
    fs.mkdirSync(path.join(project, 'nested'));
    fs.writeFileSync(path.join(project, 'nested', 'child.rs'), '// nested file\n');
    options.setup?.(project, fixture);
    const port = await new Promise((resolve, reject) => {
        const server = net.createServer();
        server.on('error', reject);
        server.listen(0, '127.0.0.1', () => { const port = server.address().port; server.close(() => resolve(port)); });
    });
    const child = spawn(executable, options.launchProject===false?[]:[options.launchProject || project], {
        windowsHide: true, stdio: 'ignore',
        env: { ...process.env,
            WEBVIEW2_USER_DATA_FOLDER: path.join(fixture, 'webview'),
            DADO_USER_HOME: process.env.DADO_USER_HOME || path.join(fixture,'user'),
            WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}`,
            // These checks don't need account access or an agent process.
            DADO_CODEX_BIN: path.join(fixture, 'no-agent.exe'),
        },
    });
    let launchError;
    child.on('error', error => { launchError = error; });
    let browser;
    async function close() {
        if (browser) await browser.close().catch(() => {});
        if (child.exitCode === null && child.pid) {
            child.kill();
            await Promise.race([new Promise(resolve => child.once('exit', resolve)), new Promise(resolve => setTimeout(resolve, 2000))]);
        }
        // Keep the disposable fixture and screenshots under ignored target/ for
        // diagnosing a failure; do not touch the user's project or app instance.
    }
    try {
        const endpoint = `http://127.0.0.1:${port}`;
        const deadline = Date.now() + 20000;
        while (true) {
            if (launchError) throw launchError;
            if (child.exitCode !== null) throw new Error(`Desktop app exited during launch: ${child.exitCode}`);
            try { const response = await fetch(`${endpoint}/json/version`); if (response.ok) break; } catch {}
            if (Date.now() > deadline) throw new Error('WebView2 debugging endpoint did not become available.');
            await new Promise(resolve => setTimeout(resolve, 100));
        }
        browser = await chromium.connectOverCDP(endpoint);
        const context = browser.contexts()[0];
        let page;
        while (!page) {
            for (const candidate of context.pages()) {
                if (await candidate.locator('#workbench-host').count()) { page = candidate; break; }
            }
            if (Date.now() > deadline) throw new Error('Dioxus did not mount the desktop workbench.');
            if (!page) await new Promise(resolve => setTimeout(resolve, 100));
        }
        const errors = [];
        page.on('pageerror', error => errors.push(error.message));
        await page.waitForFunction(() => window.dadoWorkbench && window.dadoChat).catch(async error => {console.error('Native startup errors:',errors);await page.screenshot({path:'target/ui-reference/native-startup-failure.png'});throw error;});
        await page.locator(options.launchProject===false ? '.welcome, .chat-composer textarea' : '.chat-composer textarea').first().waitFor({state:'attached'});
        await page.waitForFunction(() => document.querySelector('.welcome') || (window.dadoSidebar && window.dadoChat.snapshot().id && !document.querySelector('.chat-composer textarea')?.disabled));
        await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
        if(options.openPanel!==false && await page.getByRole('button', {name: 'Toggle side panel', exact: true}).isVisible()){
            await page.getByRole('button', {name: 'Toggle side panel', exact: true}).click();
            await page.getByRole('button', {name: 'Editor', exact: true}).waitFor();
        }
        return { page, project, fixture, errors, close };
    } catch (error) { await close(); throw error; }
}

module.exports = { nativeApp };
