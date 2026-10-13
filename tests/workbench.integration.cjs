const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { nativeApp } = require('./native_app.cjs');

(async () => {
    const app = await nativeApp();
    const {page, project, errors} = app;
    const main = path.join(project, 'main.rs');
    const selectTab = async name => {
        await page.getByRole('tab', {name, exact: true}).click();
        await page.waitForFunction(label => document.querySelector('.tab-title[aria-selected="true"]')?.getAttribute('aria-label') === label, name);
    };
    try {
        await page.getByRole('button', {name: 'Editor', exact: true}).click();
        await page.getByRole('tab', {name: 'Open file'}).waitFor();
        assert.ok(await page.locator('.open-file-empty').isVisible());
        await page.evaluate(file => dadoWorkbench.selectFile(file), main);
        await page.getByRole('tab', {name: 'main.rs', exact: true}).waitFor();
        const content = page.locator('.tab-body:not([hidden]) .cm-content');
        await content.waitFor();
        await content.click();
        await page.keyboard.press('Control+End');
        await page.keyboard.type('// unsaved');
        await page.waitForFunction(() => document.querySelector('.dirty-dot'));
        await page.evaluate(() => {
            window.savedEditor = document.querySelector('.cm-editor');
            window.savedContent = document.querySelector('.cm-content');
        });
        await page.getByRole('button', {name: 'Toggle side panel', exact: true}).click();
        await page.getByRole('button', {name: 'Toggle side panel', exact: true}).click();
        assert.ok((await content.textContent()).includes('// unsaved'));
        assert.ok(await page.evaluate(() => savedEditor === document.querySelector('.cm-editor') && savedContent === document.querySelector('.cm-content')), 'hiding must retain the same CodeMirror DOM');

        await page.getByRole('button', {name: 'Toggle file browser'}).click();
        await page.getByRole('treeitem', {name: 'README.md', exact: true}).waitFor();
        await page.getByRole('treeitem', {name: 'nested', exact: true}).click();
        await page.getByRole('treeitem', {name: 'child.rs', exact: true}).waitFor();
        const filter = page.getByRole('textbox', {name: 'Filter files'});
        await filter.fill('child');
        await page.getByRole('treeitem', {name: 'README.md', exact: true}).waitFor({state: 'hidden'});
        assert.ok(await page.getByRole('treeitem', {name: 'README.md', exact: true}).isHidden());
        await page.getByRole('button', {name: 'Toggle file browser'}).click();
        await page.getByRole('button', {name: 'Toggle file browser'}).click();
        assert.equal(await filter.inputValue(), 'child');
        assert.ok(await page.getByRole('treeitem', {name: 'child.rs', exact: true}).isVisible());
        await filter.fill('');
        await page.getByRole('treeitem', {name: 'README.md', exact: true}).click();
        await page.getByRole('tab', {name: 'README.md', exact: true}).waitFor();
        await selectTab('main.rs');
        assert.ok((await content.textContent()).includes('// unsaved'));
        assert.ok(await page.evaluate(() => savedEditor === document.querySelector('#editor-widget-1 .cm-editor')), 'switching tabs must retain the same CodeMirror instance');
        // Switching must preserve undo history as well as the visible text.
        await content.click(); await page.keyboard.press('Control+z');
        await page.waitForFunction(() => !document.querySelector('#editor-widget-1 .cm-content').textContent.includes('// unsaved'));
        await page.keyboard.press('Control+y');
        await page.waitForFunction(() => document.querySelector('#editor-widget-1 .cm-content').textContent.includes('// unsaved'));
        await page.getByRole('button', {name: 'Save file · Ctrl+S', exact: true}).click();
        await page.waitForFunction(() => !document.querySelector('#panel-tab-1 .dirty-dot'));
        assert.ok(fs.readFileSync(main, 'utf8').includes('// unsaved'));

        // A conflicting save must leave both the external change and dirty editor intact.
        await content.click(); await page.keyboard.press('Control+End'); await page.keyboard.type(' dirty');
        await page.waitForFunction(() => document.querySelector('#panel-tab-1 .dirty-dot'));
        const diskBaseline = fs.readFileSync(main, 'utf8');
        fs.writeFileSync(main, 'external change\n');
        await page.getByRole('button', {name: 'Save file · Ctrl+S', exact: true}).click();
        await page.getByRole('alert').filter({hasText: 'changed on disk'}).waitFor();
        assert.equal(fs.readFileSync(main, 'utf8'), 'external change\n');
        assert.ok((await content.textContent()).includes(' dirty'));
        fs.writeFileSync(main, diskBaseline);
        await page.getByRole('button', {name: 'Dismiss message'}).click();

        await page.getByRole('button', {name: 'Close main.rs', exact: true}).click();
        await page.getByRole('button', {name: 'Cancel', exact: true}).click();
        assert.ok((await content.textContent()).includes(' dirty'));
        await page.getByRole('button', {name: 'Close main.rs', exact: true}).click();
        await page.getByRole('button', {name: 'Save & close', exact: true}).click();
        await page.waitForFunction(() => !document.getElementById('panel-tab-1'));
        assert.ok(fs.readFileSync(main, 'utf8').includes(' dirty'));

        await page.getByRole('button', {name: 'New tab', exact: true}).click();
        await page.getByRole('button', {name: 'Terminal', exact: true}).click();
        await page.locator('.xterm').waitFor();
        await page.evaluate(() => { window.savedTerminal = document.querySelector('.xterm'); });
        const terminalInput = page.locator('.xterm-helper-textarea');
        await terminalInput.focus();
        await page.keyboard.type('Write-Output "DIOXUS_TERMINAL_OK"');
        await page.keyboard.press('Enter');
        await page.waitForFunction(() => [...document.querySelectorAll('.xterm-rows > div')].some(row => row.textContent.trim() === 'DIOXUS_TERMINAL_OK'), null, {timeout: 15000});
        await selectTab('README.md');
        await selectTab('Terminal');
        await page.getByRole('button', {name: 'Toggle side panel', exact: true}).click();
        await page.getByRole('button', {name: 'Toggle side panel', exact: true}).click();
        assert.ok(await page.evaluate(() => savedTerminal === document.querySelector('.xterm')), 'tab/panel switches must retain xterm and its PTY');

        // Real Dioxus keyboard handlers move focus and selection together.
        const terminalTab = page.getByRole('tab', {name: 'Terminal', exact: true});
        await terminalTab.focus(); await page.keyboard.press('Home');
        await page.waitForFunction(() => document.activeElement?.textContent === 'README.md');
        assert.equal(await page.getByRole('tab', {name: 'README.md', exact: true}).getAttribute('aria-selected'), 'true');
        await page.keyboard.press('End');
        await page.waitForFunction(() => document.activeElement?.textContent === 'Terminal');

        // Check the exact icon/ghost centering that motivated shared controls.
        const geometry = await page.locator('#workbench-host .workbench-icon:visible').evaluateAll(buttons => buttons.map(button => {
            const icon = button.querySelector('svg'); if (!icon) return null;
            const b = button.getBoundingClientRect(), i = icon.getBoundingClientRect();
            return {dx: Math.abs(b.x+b.width/2-i.x-i.width/2), dy: Math.abs(b.y+b.height/2-i.y-i.height/2)};
        }).filter(Boolean));
        assert.ok(geometry.length >= 3);
        assert.ok(geometry.every(({dx,dy}) => dx < .6 && dy < .6), JSON.stringify(geometry));
        await page.screenshot({path: 'target/ui-reference/terminal-after.png'});

        await page.evaluate(file => dadoWorkbench.openFile(file), main);
        await page.getByRole('tab', {name: 'main.rs', exact: true}).waitFor();
        await page.locator('.tab-body:not([hidden]) .cm-content').waitFor();
        await page.getByRole('button', {name: 'Toggle file browser'}).click();
        await page.getByRole('treeitem', {name: 'README.md', exact: true}).waitFor();
        await page.screenshot({path: 'target/ui-reference/editor-after.png'});

        // The other chat's reviewed create/delete flows must still stage safely
        // after shell ownership moves to Dioxus. These synthetic reviews aren't
        // sent to the host's Save endpoint; that ownership gate remains intact.
        const newFile = path.join(project, 'new.rs');
        await page.evaluate(async ({project, newFile}) => {
            await dadoWorkbench.applyProposal({state: 'pending', structuralSaveAvailable: true, changes: [{
                operation: 'create', path: 'new.rs', absolutePath: newFile, diskBefore: null,
                before: '', after: '// proposed new file\n', revision: 'test-create',
            }]}, project, {requestId: 'native-ui-test'});
        }, {project, newFile});
        await page.getByRole('tab', {name: 'new.rs', exact: true}).waitFor();
        await page.waitForFunction(() => {const button=document.querySelector('[aria-label="Save new file · Ctrl+S"]'); return button && !button.disabled;});
        assert.equal(fs.existsSync(newFile), false, 'Apply must not create the live file');
        assert.ok((await page.locator('.tab-body:not([hidden]) .editor-caption').textContent()).includes('new'));
        await page.getByRole('button', {name: 'Close new.rs', exact: true}).click();
        await page.getByRole('button', {name: 'Discard', exact: true}).click();
        await page.waitForFunction(() => !document.querySelector('[aria-label="Close new.rs"]'));
        assert.equal(fs.existsSync(newFile), false);

        const readme = path.join(project, 'README.md');
        const beforeDelete = fs.readFileSync(readme, 'utf8');
        await page.evaluate(async ({project, readme, beforeDelete}) => {
            await dadoWorkbench.applyProposal({state: 'pending', structuralSaveAvailable: true, changes: [{
                operation: 'delete', path: 'README.md', absolutePath: readme, diskBefore: beforeDelete,
                before: beforeDelete, after: '', revision: 'test-delete',
            }]}, project, {requestId: 'native-ui-test'});
        }, {project, readme, beforeDelete});
        await page.waitForFunction(() => {const button=document.querySelector('[aria-label="Delete reviewed file · Ctrl+S"]'); return button && !button.disabled;});
        await selectTab('README.md');
        await page.locator('.tab-body:not([hidden]) .cm-content').click();
        await page.keyboard.type('must not change');
        const staged = await page.evaluate(file => dadoWorkbench.agentContext().buffers.find(buffer => buffer.path === file), readme);
        assert.equal(staged.text, beforeDelete, 'pending deletion must remain read-only');
        assert.equal(fs.readFileSync(readme, 'utf8'), beforeDelete, 'Apply must not delete the live file');
        await page.getByRole('button', {name: 'Close README.md', exact: true}).click();
        await page.getByRole('button', {name: 'Delete & close', exact: true}).waitFor();
        await page.getByRole('button', {name: 'Discard', exact: true}).click();
        assert.equal(fs.readFileSync(readme, 'utf8'), beforeDelete);
        assert.deepEqual(errors, []);
        console.log('Native Dioxus workbench passed: controls, tree/filter, focus, icon centering, safe saves, editor undo/DOM retention, and live terminal retention.');
    } catch (error) {
        console.error(await page.evaluate(() => ({
            active: document.querySelector('.tab-title[aria-selected="true"]')?.textContent,
            buffers: dadoWorkbench.agentContext().buffers,
            bodies: [...document.querySelectorAll('.tab-body')].map(body => ({id: body.id, hidden: body.hidden, text: body.querySelector('.cm-content')?.textContent})),
        })));
        await page.screenshot({path: 'target/ui-reference/workbench-failure.png'}).catch(() => {});
        throw error;
    } finally { await app.close(); }
})().catch(error => { console.error(error); process.exitCode = 1; });
