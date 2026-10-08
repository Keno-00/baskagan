const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const fs = require('node:fs');

(async () => {
    const browser = await chromium.launch({ headless: true, channel: process.env.DADO_TEST_BROWSER || undefined });
    try {
        const page = await browser.newPage({ viewport: { width: 1200, height: 800 } });
        const errors = [];
        page.on('pageerror', error => errors.push(error.message));
        await page.setContent(`<style>${fs.readFileSync('src/style.css', 'utf8')}${fs.readFileSync('assets/workbench.css', 'utf8')}</style><div id="main"><div class="workspace panel-open"><main>Systems view</main><aside id="side-panel" class="side-panel"><div id="workbench-host"></div></aside></div></div>`);
        await page.addScriptTag({ path: 'assets/workbench.js' });
        await page.evaluate(() => {
            window.files = { 'C:\\project\\main.rs': 'fn main() {\n    println!("hello");\n}\n', 'C:\\project\\README.md': '# Sample\n' };
            window.requests = [];
            startWorkbench(message => {
                requests.push(message);
                let result;
                if (message.action === 'read') result = {path: message.path, text: files[message.path]};
                if (message.action === 'list') result = Object.keys(files).map(path => ({path, label: path.split('\\').pop(), directory: false}));
                if (message.action === 'save') { files[message.path] = message.text; result = null; }
                if (message.action.startsWith('terminal_')) result = null;
                if (message.action === 'terminal_open') dadoWorkbench.terminalOutput([[message.tab, [27, 91, 54, 110]]]);
                if (message.id) queueMicrotask(() => dadoWorkbench.reply({id: message.id, result}));
            });
            dadoWorkbench.setRoot('C:\\project'); dadoWorkbench.setVisible(true);
        });
        const typeEditor = page.getByRole('button', {name: 'Editor', exact: true});
        await typeEditor.click();
        await page.getByRole('tab', {name: 'Open file'}).waitFor();
        assert.ok(await page.locator('.open-file-empty').isVisible());
        await page.evaluate(() => dadoWorkbench.selectFile('C:\\project\\main.rs'));
        await page.getByRole('tab', {name: 'main.rs', exact: true}).waitFor();
        assert.ok(await page.locator('.cm-lineNumbers').isVisible());
        await page.locator('.cm-content').click();
        await page.keyboard.press('Control+End');
        await page.keyboard.type('// unsaved');
        assert.equal(await page.locator('.dirty-dot').count(), 1);
        await page.evaluate(() => { dadoWorkbench.setVisible(false); document.querySelector('.workspace').classList.remove('panel-open'); });
        await page.evaluate(() => { document.querySelector('.workspace').classList.add('panel-open'); dadoWorkbench.setVisible(true); });
        assert.ok((await page.locator('.cm-content').innerText()).includes('// unsaved'), 'hidden panel must preserve edits');
        await page.locator('.cm-content').click(); await page.keyboard.press('Control+s');
        await page.waitForTimeout(100);
        assert.ok((await page.evaluate(() => files['C:\\project\\main.rs'])).includes('// unsaved'));
        assert.equal(await page.locator('.dirty-dot').count(), 0);
        await page.getByRole('button', {name: 'Toggle file browser'}).click();
        await page.locator('.file-row[aria-label="README.md"]').waitFor();
        await page.locator('.file-row[aria-label="README.md"]').click();
        await page.getByRole('tab', {name: 'README.md', exact: true}).waitFor();
        await page.getByRole('tab', {name: 'main.rs', exact: true}).click();
        assert.ok((await page.locator('.tab-body:not([hidden]) .cm-content').innerText()).includes('// unsaved'), 'switching tabs must preserve document');
        await page.getByRole('button', {name: 'New tab', exact: true}).click();
        await page.getByRole('button', {name: 'Terminal', exact: true}).click();
        await page.locator('.xterm').waitFor();
        await page.waitForTimeout(100);
        assert.equal(await page.evaluate(() => requests.filter(r => r.action === 'terminal_open').length), 1);
        assert.ok(await page.evaluate(() => requests.some(r => r.action === 'terminal_input' && /\u001b\[\d+;\d+R/.test(r.text))), 'xterm must answer the startup cursor query');
        await page.evaluate(() => { dadoWorkbench.setVisible(false); dadoWorkbench.setVisible(true); });
        assert.equal(await page.evaluate(() => requests.filter(r => r.action === 'terminal_open').length), 1, 'toggle must not restart terminal');
        assert.deepEqual(errors, []);
        await page.screenshot({path: 'target/workbench-preview.png'});
        console.log('Workbench tests passed: editor, line numbers, saving, browser, tab persistence, and terminal persistence.');
    } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exitCode = 1; });
