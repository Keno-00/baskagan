const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const root = 'C:\\test';
const node = (path, directory, parent, position, has_children = false) => ({ path, label: path.split('\\').pop(), directory, parent, position, has_children });
const rootScene = {
    current: root, parent: null, overview: false,
    tree: { notices: [], nodes: [
        node(root, true, null, [0, 0, 0], true),
        node(root + '\\docs', true, 0, [-150, 215, 50], true),
        node(root + '\\src', true, 0, [150, 245, -50], true),
        node(root + '\\README.md', false, 0, [300, 195, 20]),
    ] },
};
const docsScene = {
    current: root + '\\docs', parent: root, overview: false,
    tree: { notices: [], nodes: [
        node(root + '\\docs', true, null, [0, 0, 0], true),
        node(root + '\\docs\\notes', true, 0, [-70, 215, 50]),
        node(root + '\\docs\\design.md', false, 0, [70, 245, -50]),
    ] },
};
(async () => {
    const browser = await chromium.launch({ headless: true, channel: process.env.DADO_TEST_BROWSER || undefined });
    const page = await browser.newPage({ viewport: { width: 1200, height: 800 } });
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.setContent(`<html><head><style>${fs.readFileSync('src/style.css', 'utf8')}</style></head><body><div id="main"><main><section class="constellation"><svg id="folder-scene" class="graph" viewBox="-600 -300 1200 650" tabindex="0"><g id="scene-edges"></g><g id="scene-nodes"></g></svg></section></main></div></body></html>`);
    await page.evaluate(() => { window.messages = []; window.dioxus = { send: message => messages.push(message) }; });
    await page.addScriptTag({ path: 'src/scene_math.js' });
    await page.addScriptTag({ path: 'src/scene.js' });
    await page.evaluate(graph => dadoScene.setGraph(graph), rootScene);
    await page.waitForTimeout(60);
    const docs = page.locator('g[aria-label="docs"]');
    await docs.click();
    assert.equal(await page.evaluate(() => messages.filter(m => m.action === 'enter').length), 0, 'single click must not enter');
    assert.equal(await docs.getAttribute('aria-pressed'), 'true');
    const file = page.locator('g[aria-label="README.md"]');
    await file.click();
    await page.waitForFunction(() => document.querySelector('g[aria-label="README.md"]')?.getAttribute('aria-pressed') === 'true');
    assert.equal(await file.getAttribute('aria-pressed'), 'true', 'file nodes must be selectable');
    await file.dblclick();
    assert.equal(await page.evaluate(() => messages.filter(m => m.action === 'open_file').at(-1).path), root + '\\README.md');
    await docs.dblclick();
    assert.equal(await page.evaluate(() => messages.filter(m => m.action === 'enter').at(-1).path), root + '\\docs');
    await page.evaluate(graph => dadoScene.setGraph(graph), docsScene);
    await page.waitForTimeout(720);
    assert.ok(await page.locator('g[aria-label="docs"]').isVisible(), 'new central folder must remain visible');
    await page.evaluate(() => dadoScene.goHistory(-1));
    assert.equal(await page.evaluate(() => messages.filter(m => m.action === 'enter').at(-1).path), root, 'back must navigate to previous folder');
    await page.evaluate(graph => dadoScene.setGraph(graph), rootScene);
    await page.waitForTimeout(720);
    await page.evaluate(() => dadoScene.goHistory(1));
    assert.equal(await page.evaluate(() => messages.filter(m => m.action === 'enter').at(-1).path), root + '\\docs', 'forward must restore folder');
    await page.evaluate(graph => dadoScene.setGraph(graph), docsScene);
    await page.waitForTimeout(720);
    await page.mouse.move(600, 400);
    await page.mouse.wheel(0, 120);
    assert.equal(await page.evaluate(() => messages.filter(m => m.action === 'enter').at(-1).path), root, 'scroll down must go to parent');
    await page.evaluate(graph => dadoScene.setGraph(graph), rootScene);
    await page.waitForTimeout(720);
    await page.locator('g[aria-label="src"]').click();
    await page.mouse.move(600, 400);
    await page.mouse.wheel(0, -120);
    assert.equal(await page.evaluate(() => messages.filter(m => m.action === 'enter').at(-1).path), root + '\\src', 'scroll up must prefer selected anchor');
    await page.evaluate(() => dadoScene.cancelNavigation());
    const before = await page.locator('g[aria-label="docs"]').getAttribute('transform');
    await page.keyboard.press('Control+=');
    await page.waitForTimeout(350);
    const larger = await page.locator('g[aria-label="docs"]').getAttribute('transform');
    assert.notEqual(before, larger, 'Ctrl-plus must scale the scene');
    await page.keyboard.press('Control+-');
    await page.waitForTimeout(350);
    const restored = await page.locator('g[aria-label="docs"]').getAttribute('transform');
    assert.equal(before, restored, 'Ctrl-minus must undo one scale step');
    await page.mouse.move(580, 350);
    await page.mouse.down();
    await page.mouse.move(690, 350, { steps: 8 });
    await page.mouse.up();
    await page.waitForTimeout(250);
    assert.notEqual(before, await page.locator('g[aria-label="docs"]').getAttribute('transform'), 'drag must rotate');
    await page.evaluate(() => dadoScene.reset());
    await page.waitForTimeout(500);
    assert.equal(before, await page.locator('g[aria-label="docs"]').getAttribute('transform'), 'reset must ease back to default camera');
    await page.evaluate(() => document.querySelector('.constellation').remove());
    await page.waitForTimeout(50);
    assert.equal(await page.evaluate(() => Boolean(window.dadoScene)), false, 'unmount must release handlers and animation');
    assert.deepEqual(errors, []);
    await browser.close();
    console.log('Controller integration passed: selection, double-click, scroll navigation, scaling, rotation, reset, and cleanup.');
})().catch(error => { console.error(error); process.exitCode = 1; });
