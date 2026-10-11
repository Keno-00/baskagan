import { EditorView, basicSetup } from 'codemirror';
import { keymap } from '@codemirror/view';
import { Decoration, ViewPlugin } from '@codemirror/view';
import { StateEffect, Compartment, EditorState } from '@codemirror/state';
import './annotations.js';
import './review.js';
import { undo, redo, selectAll, deleteCharForward, indentWithTab } from '@codemirror/commands';
import { rust } from '@codemirror/lang-rust';
import { javascript } from '@codemirror/lang-javascript';
import { json } from '@codemirror/lang-json';
import { python } from '@codemirror/lang-python';
import { markdown } from '@codemirror/lang-markdown';
import { html } from '@codemirror/lang-html';
import { css } from '@codemirror/lang-css';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import '@xterm/xterm/css/xterm.css';

const paths = {
    terminal: 'M4 5h16v14H4zM7 9l3 3-3 3m5 0h4',
    editor: 'M7 3h7l4 4v14H7zM14 3v5h4M10 12h5m-5 4h5',
    folder: 'M3 7h7l2 2h9v11H3zM3 7V4h7l2 3',
    save: 'M5 4h13l2 2v14H4V4zM8 4v6h8V4M8 20v-7h8v7',
    plus: 'M12 5v14M5 12h14',
    close: 'M7 7l10 10M17 7L7 17',
    chevron: 'M9 5l7 7-7 7',
};
function icon(name) {
    const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    svg.setAttribute('viewBox', '0 0 24 24'); svg.setAttribute('aria-hidden', 'true');
    const path = document.createElementNS(svg.namespaceURI, 'path'); path.setAttribute('d', paths[name]);
    svg.append(path); return svg;
}
function element(tag, className, text) {
    const node = document.createElement(tag);
    if (className) node.className = className;
    if (text !== undefined) node.textContent = text;
    return node;
}
function language(path) {
    const extension = path.split('.').pop().toLowerCase();
    if (extension === 'rs') return rust();
    if (['js', 'jsx', 'ts', 'tsx'].includes(extension)) return javascript({ typescript: extension.startsWith('t'), jsx: extension.endsWith('x') });
    return { json, py: python, md: markdown, html, css }[extension]?.() || [];
}
const editorTheme = EditorView.theme({
    '&': { height: '100%', fontSize: '12px', backgroundColor: '#fafafa' },
    '.cm-scroller': { fontFamily: 'Consolas, "Cascadia Code", monospace', lineHeight: '1.7', overflow: 'auto' },
    '.cm-content': { padding: '12px 0', caretColor: '#333' },
    '.cm-line': { padding: '0 16px 0 8px' },
    '.cm-gutters': { backgroundColor: '#fafafa', color: '#aaa', border: 'none', paddingLeft: '8px' },
    '.cm-activeLine, .cm-activeLineGutter': { backgroundColor: '#f0f0f0' },
    '&.cm-focused': { outline: 'none' },
    '.cm-selectionBackground, &.cm-focused .cm-selectionBackground': { backgroundColor: '#ddd' },
    '.cm-panels': { backgroundColor: '#f2f2f2', border: 'none', fontFamily: '"Segoe UI", sans-serif' },
});

window.startWorkbench = send => {
    window.dadoWorkbench?.dispose();
    const host = document.getElementById('workbench-host');
    if (!host) return;
    let sequence = 0, requestId = 0, active = null, root = null, selectedFile = null, visible = false;
    let hintTimer = null, disposed = false;
    const tabs = [], pending = new Map();
    const hint = (text, duration = 0) => {
        if (hintTimer) clearTimeout(hintTimer);
        send({ action: 'hint', text: text || null });
        if (duration) hintTimer = setTimeout(() => hint(null), duration);
    };
    const rpc = (action, data = {}) => new Promise((resolve, reject) => {
        const id = ++requestId;
        pending.set(id, { resolve, reject }); send({ action, id, ...data });
    });
    const button = (name, label, action, text = false) => {
        const node = element('button', text ? 'type-button' : 'workbench-icon');
        node.type = 'button'; node.setAttribute('aria-label', label); node.append(icon(name));
        if (text) node.append(element('span', '', label));
        node.addEventListener('click', action);
        node.addEventListener('mouseenter', () => hint(label));
        node.addEventListener('mouseleave', () => hint(null));
        node.addEventListener('focus', () => hint(label));
        node.addEventListener('blur', () => hint(null));
        return node;
    };
    const bar = element('div', 'tab-bar'), tabList = element('div', 'tab-list');
    tabList.setAttribute('role', 'tablist');
    const plus = button('plus', 'New tab', () => newChoice());
    bar.append(tabList, plus);
    const notices = element('div', 'workbench-notices');
    const bodies = element('div', 'tab-bodies');
    host.append(bar, notices, bodies);

    function report(error) {
        notices.replaceChildren();
        const row = element('div', 'editor-notice');
        row.setAttribute('role', 'alert'); row.append(element('span', '', String(error.message || error)));
        row.append(button('close', 'Dismiss message', () => notices.replaceChildren())); notices.append(row);
        hint(String(error.message || error), 4000);
    }
    function renderTabs() {
        tabList.replaceChildren();
        for (const tab of tabs) {
            const row = element('div', tab.id === active ? 'panel-tab active' : 'panel-tab');
            const title = element('button', 'tab-title'); title.type = 'button';
            title.setAttribute('role', 'tab'); title.setAttribute('aria-selected', String(tab.id === active));
            title.setAttribute('aria-controls', `tab-body-${tab.id}`);
            title.append(icon(tab.type === 'terminal' ? 'terminal' : 'editor'));
            title.append(element('span', 'tab-name', tab.title || 'New tab'));
            if (tab.dirty) title.append(element('span', 'dirty-dot', '•'));
            title.addEventListener('click', () => activate(tab));
            title.addEventListener('mouseenter', () => hint(tab.path || tab.title || 'Choose a tab type'));
            title.addEventListener('mouseleave', () => hint(null));
            row.append(title, button('close', `Close ${tab.title || 'tab'}`, () => closeTab(tab)));
            tabList.append(row);
        }
    }
    function activate(tab) {
        active = tab.id;
        for (const entry of tabs) entry.body.hidden = entry.id !== active;
        renderTabs(); notices.replaceChildren();
        if (tab.path) { selectedFile = tab.path; send({action: 'selected', path: tab.path}); }
        requestAnimationFrame(() => { tab.view?.requestMeasure(); fitTerminal(tab); });
    }
    function createTab() {
        const tab = { id: ++sequence, type: 'choice', title: 'New tab', dirty: false, browser: false, path: null };
        tab.body = element('section', 'tab-body'); tab.body.id = `tab-body-${tab.id}`;
        tab.body.setAttribute('role', 'tabpanel'); bodies.append(tab.body); tabs.push(tab); activate(tab); return tab;
    }
    function newChoice() {
        const tab = createTab();
        const chooser = element('div', 'tab-chooser');
        chooser.append(button('terminal', 'Terminal', () => createTerminal(tab), true));
        chooser.append(button('editor', 'Editor', () => createEditor(tab, selectedFile), true));
        tab.body.append(chooser); return tab;
    }
    async function save(tab) {
        if (!tab.view || !tab.path || tab.saving || tab.agentLocked) return false;
        tab.saving = true;
        const snapshot = tab.view.state.doc.toString();
        const text = (tab.bom ? '\ufeff' : '') + snapshot.replace(/\n/g, tab.newline);
        try {
            await rpc('save', { path: tab.path, text, expected: tab.original });
            tab.original = text; tab.savedText = snapshot;
            tab.dirty = tab.view.state.doc.toString() !== snapshot; renderTabs();
            hint(`Saved ${tab.path}`, 1800); return true;
        } catch (error) { report(error); return false; }
        finally { tab.saving = false; }
    }
    function closeTab(tab) {
        if(tab.agentLocked) return;
        if (tab.dirty) {
            notices.replaceChildren();
            const row = element('div', 'unsaved-notice');
            row.append(element('span', '', `Save changes to ${tab.title}?`));
            const saveClose = element('button', '', 'Save & close');
            saveClose.onclick = async () => { if (await save(tab) && !tab.dirty) removeTab(tab); };
            const discard = element('button', '', 'Discard'); discard.onclick = () => removeTab(tab);
            const cancel = element('button', '', 'Cancel'); cancel.onclick = () => notices.replaceChildren();
            row.append(saveClose, discard, cancel); notices.append(row); return;
        }
        removeTab(tab);
    }
    function removeTab(tab) {
        tab.closed = true;
        tab.commentCleanup?.();
        tab.view?.destroy(); tab.terminal?.dispose(); tab.terminalResize?.dispose();
        if (tab.type === 'terminal') rpc('terminal_close', { tab: tab.id }).catch(report);
        const index = tabs.indexOf(tab); tabs.splice(index, 1); tab.body.remove(); notices.replaceChildren();
        if (tab.id === active) {
            if (tabs.length) activate(tabs[Math.min(index, tabs.length - 1)]);
            else newChoice();
        } else renderTabs();
    }
    async function pick(tab) {
        try {
            const path = await rpc('pick');
            if (path) {
                if (tab.path) openFile(path);
                else loadFile(tab, path);
            }
        } catch (error) { report(error); }
    }
    function createEditor(tab, path) {
        tab.type = 'editor'; tab.title = 'Open file'; tab.body.replaceChildren();
        const toolbar = element('div', 'editor-toolbar');
        tab.caption = element('div', 'editor-caption', 'Open file');
        tab.commentListRect = () => document.querySelector('.chat-composer')?.getBoundingClientRect() || tab.body.getBoundingClientRect();
        toolbar.append(tab.caption, button('editor', 'Open file', () => pick(tab)));
        tab.saveButton = button('save', 'Save file · Ctrl+S', () => save(tab)); tab.saveButton.disabled = true;
        toolbar.append(tab.saveButton, button('folder', 'Toggle file browser', () => {
            tab.browser = !tab.browser; tab.explorer.hidden = !tab.browser;
            tab.view?.requestMeasure(); if (tab.browser) renderExplorer(tab);
        }));
        const content = element('div', 'editor-layout');
        tab.editorHost = element('div', 'editor-host');
        const empty = element('div', 'open-file-empty');
        empty.append(button('folder', 'Open file', () => pick(tab), true)); tab.editorHost.append(empty);
        tab.explorer = element('aside', 'file-explorer'); tab.explorer.hidden = true;
        content.append(tab.editorHost, tab.explorer); tab.body.append(toolbar, content); renderTabs();
        if (path) loadFile(tab, path);
    }
    async function loadFile(tab, path) {
        if(tab.agentLocked) return;
        tab.loading = true;
        try {
            const file = await rpc('read', { path });
            if (tab.closed) return;
            tab.path = file.path; tab.title = file.path.split(/[\\/]/).pop();
            tab.original = file.text; tab.bom = file.text.startsWith('\ufeff');
            tab.newline = file.text.includes('\r\n') ? '\r\n' : '\n';
            const content = file.text.replace(/^\ufeff/, '').replace(/\r\n/g, '\n');
            tab.savedText = content; tab.dirty = false;
            tab.caption.textContent = tab.title; tab.saveButton.disabled = false;
            tab.view?.destroy(); tab.editorHost.replaceChildren();
            tab.commentCleanup?.();
            const refreshComments = StateEffect.define();
            const annotations = window.dadoComments;
            const commentPlugin = ViewPlugin.fromClass(class {
                constructor(view) { this.decorations = this.build(view); }
                build(view) {
                    return Decoration.set(annotations.comments.filter(item => item.source === path && item.to > item.from && item.to <= view.state.doc.length)
                        .map(item => Decoration.mark({class: 'comment-code-highlight', attributes: {'data-comment': String(item.id)}}).range(item.from, item.to)), true);
                }
                update(update) {
                    if (update.docChanged) annotations.comments.filter(item => item.source === path).forEach(item => { item.from = update.changes.mapPos(item.from, 1); item.to = update.changes.mapPos(item.to, -1); if (item.to < item.from) item.to = item.from; });
                    if (update.docChanged) queueMicrotask(() => annotations.changed());
                    annotations.comments.filter(item => item.source === path).forEach(item => { item.lineFrom = update.state.doc.lineAt(Math.min(item.from, update.state.doc.length)).number; item.lineTo = update.state.doc.lineAt(Math.min(Math.max(item.from, item.to - 1), update.state.doc.length)).number; });
                    this.decorations = this.build(update.view);
                    requestAnimationFrame(() => tab.positionComment?.());
                }
            }, {decorations: plugin => plugin.decorations});
            tab.agentLock = new Compartment();
            tab.view = new EditorView({
                parent: tab.editorHost, doc: content,
                extensions: [basicSetup, editorTheme, language(file.path), commentPlugin, tab.agentLock.of(EditorState.readOnly.of(false)), keymap.of([
                    { key: 'Mod-s', run: () => { save(tab); return true; } }, indentWithTab,
                ]), EditorView.updateListener.of(update => {
                    if (update.docChanged) { tab.dirty = update.state.doc.toString() !== tab.savedText; renderTabs(); }
                    if (update.selectionSet) {
                        requestAnimationFrame(() => tab.positionComment?.());
                        const head = update.state.selection.main.head;
                        const line = update.state.doc.lineAt(head);
                        hint(`${tab.path} · Ln ${line.number}, Col ${head - line.from + 1}`);
                    }
                })],
            });
            const rail = element('div', 'editor-comment-rail');
            const counts = element('div', 'editor-line-counts'); rail.append(counts);
            const commentButton = element('button', 'annotation-trigger editor-line-comment'); commentButton.type = 'button'; commentButton.setAttribute('aria-label', 'Comment on code'); commentButton.innerHTML = annotations.glyph; commentButton.hidden = true;
            rail.append(commentButton); tab.editorHost.append(rail);
            tab.positionComment = () => {
                if (tab.closed || !tab.view) return;
                const range = tab.view.state.selection.main;
                const coords = tab.view.coordsAtPos(range.head);
                const bounds = tab.editorHost.getBoundingClientRect();
                commentButton.hidden = !coords || coords.top < bounds.top || coords.bottom > bounds.bottom;
                if (coords) commentButton.style.top = `${Math.max(0, coords.top - bounds.top - 3)}px`;
                counts.replaceChildren();
                const grouped = new Map();
                annotations.comments.filter(item => item.source === path).forEach(item => { const line = tab.view.state.doc.lineAt(Math.min(item.from, tab.view.state.doc.length)); if (!grouped.has(line.from)) grouped.set(line.from, []); grouped.get(line.from).push(item); });
                grouped.forEach((items, pos) => {
                    const location = tab.view.coordsAtPos(pos); if (!location || location.top < bounds.top || location.bottom > bounds.bottom) return;
                    const ids = items.map(item => item.id).join(', ');
                    const badge = element('button', 'editor-line-count comment-badge'); badge.type = 'button'; badge.innerHTML = annotations.glyph; badge.append(element('span', '', ids)); badge.setAttribute('aria-label', `View code comments ${ids}`);
                    badge.style.top = `${location.top - bounds.top - 3}px`; badge.onclick = () => annotations.list(tab.commentListRect(), items); counts.append(badge);
                });
            };
            commentButton.onpointerdown = event => event.preventDefault();
            commentButton.onclick = () => {
                const selection = tab.view.state.selection.main;
                const line = tab.view.state.doc.lineAt(selection.head);
                const from = selection.empty ? line.from : selection.from;
                const to = selection.empty ? line.to : selection.to;
                const quote = tab.view.state.doc.sliceString(from, to) || `Line ${line.number} (empty)`;
                annotations.begin({quote, source: path, from, to, lineFrom: tab.view.state.doc.lineAt(from).number, lineTo: tab.view.state.doc.lineAt(Math.max(from, to - 1)).number}, commentButton.getBoundingClientRect());
            };
            const scroller = tab.view.scrollDOM;
            scroller.addEventListener('scroll', tab.positionComment);
            tab.view.dom.addEventListener('pointerup', tab.positionComment);
            const unsubscribe = annotations.subscribe(() => {
                if (tab.closed || !tab.view) return;
                tab.view.dispatch({effects: refreshComments.of(null)});
            });
            tab.commentCleanup = () => { unsubscribe(); scroller.removeEventListener('scroll', tab.positionComment); };
            if (tab.id === active) { selectedFile = file.path; send({ action: 'selected', path: file.path }); }
            renderTabs(); if (tab.browser) renderExplorer(tab);
            requestAnimationFrame(() => { tab.view.requestMeasure(); if (tab.id === active && visible) tab.view.focus(); });
            return tab;
        } catch (error) { report(error); }
        finally { tab.loading = false; }
    }
    function openFile(path) {
        const existing = tabs.find(tab => tab.path === path);
        if (existing) { activate(existing); existing.view?.focus(); return; }
        let tab = tabs.find(tab => tab.id === active && tab.type === 'editor' && !tab.path && !tab.loading);
        if (!tab) { tab = createTab(); createEditor(tab, null); }
        activate(tab); loadFile(tab, path);
    }
    async function renderExplorer(tab) {
        tab.explorer.replaceChildren();
        if (!root) { tab.explorer.append(element('p', 'explorer-empty', 'Open a project folder')); return; }
        const filter = element('input', 'file-filter'); filter.placeholder = 'Filter files…'; filter.setAttribute('aria-label', 'Filter files');
        filter.value = tab.filter || '';
        const tree = element('div', 'file-tree'); tree.setAttribute('role', 'tree'); tab.explorer.append(filter, tree);
        const applyFilter = () => {
            const query = filter.value.toLowerCase(); tab.filter = filter.value;
            for (const row of tree.querySelectorAll('.file-row')) row.hidden = row.dataset.directory !== 'true' && !row.dataset.label.toLowerCase().includes(query);
        };
        filter.oninput = applyFilter;
        tab.expanded ||= new Set();
        async function addFolder(parent, path, depth = 0) {
            let entries;
            try { entries = await rpc('list', { path }); }
            catch (error) { parent.append(element('p', 'explorer-empty', error.message)); return; }
            if (tab.closed || !parent.isConnected) return;
            for (const entry of entries) {
                const branch = element('div', 'file-branch'), row = element('button', 'file-row');
                row.type = 'button'; row.style.paddingLeft = `${10 + depth * 12}px`;
                row.dataset.label = entry.label; row.dataset.directory = String(entry.directory);
                row.setAttribute('role', 'treeitem'); row.setAttribute('aria-label', entry.label);
                row.append(icon(entry.directory ? 'chevron' : 'editor'), element('span', '', entry.label));
                if (entry.path === tab.path) row.classList.add('current-file');
                row.onmouseenter = () => hint(entry.path); row.onmouseleave = () => hint(null);
                const children = element('div', 'file-children'); children.hidden = true;
                row.onclick = async () => {
                    if (!entry.directory) { openFile(entry.path); return; }
                    if (!children.childElementCount) await addFolder(children, entry.path, depth + 1);
                    children.hidden = !children.hidden; row.classList.toggle('expanded', !children.hidden);
                    row.setAttribute('aria-expanded', String(!children.hidden));
                    if (children.hidden) tab.expanded.delete(entry.path); else tab.expanded.add(entry.path);
                    applyFilter();
                };
                branch.append(row, children); parent.append(branch);
                if (entry.directory && tab.expanded.has(entry.path)) {
                    await addFolder(children, entry.path, depth + 1); children.hidden = false; row.classList.add('expanded');
                }
            }
            applyFilter();
        }
        await addFolder(tree, root);
    }
    async function createTerminal(tab) {
        tab.type = 'terminal'; tab.title = 'Terminal'; tab.body.replaceChildren();
        const terminalHost = element('div', 'terminal-host'); tab.body.append(terminalHost);
        tab.terminal = new Terminal({
            fontFamily: 'Consolas, "Cascadia Code", monospace', fontSize: 12, lineHeight: 1.35,
            cursorBlink: true, scrollback: 5000,
            theme: { background: '#fafafa', foreground: '#333333', cursor: '#555555', selectionBackground: '#dddddd' },
        });
        tab.fit = new FitAddon(); tab.terminal.loadAddon(tab.fit); tab.terminal.open(terminalHost); renderTabs();
        // Subscribe before the first PTY bytes arrive: ConPTY asks xterm for its
        // cursor position during startup, and losing that response stalls the shell.
        tab.terminal.onData(text => rpc('terminal_input', { tab: tab.id, text }).catch(report));
        try {
            fitTerminal(tab);
            await rpc('terminal_open', { tab: tab.id, path: root, cols: tab.terminal.cols, rows: tab.terminal.rows });
            if (tab.closed) { await rpc('terminal_close', {tab: tab.id}); return; }
            tab.terminalResize = tab.terminal.onResize(({cols, rows}) => rpc('terminal_resize', { tab: tab.id, cols, rows }).catch(report));
            tab.terminal.focus();
        } catch (error) { tab.terminal.write(`\r\n${error.message}\r\n`); report(error); }
    }
    function fitTerminal(tab) {
        if (visible && tab?.type === 'terminal' && !tab.body.hidden && host.clientWidth > 0) {
            try { tab.fit?.fit(); } catch { /* Another layout frame will retry. */ }
        }
    }
    const resize = new ResizeObserver(() => {
        if (!visible) return;
        for (const tab of tabs) if (tab.id === active) { tab.view?.requestMeasure(); fitTerminal(tab); }
    });
    resize.observe(host);
    const refreshBrowser = setInterval(() => {
        const tab = tabs.find(tab => tab.id === active);
        if (visible && tab?.browser && !tab.explorer.contains(document.activeElement)) renderExplorer(tab);
    }, 4000);
    window.dadoWorkbench = {
        async refreshFromDisk(expectedRoot) {
            const startRoot=root,{pathKey,normalizeText}=window.dadoReview;
            if(!root || pathKey(root)!==pathKey(expectedRoot))return;
            const prefix=pathKey(root).replace(/\/$/,'')+'/';
            for(const tab of tabs.filter(tab=>tab.type==='editor' && tab.path && tab.view && pathKey(tab.path).startsWith(prefix))) {
                if(tab.closed || tab.loading || tab.saving || tab.agentLocked)continue;
                const view=tab.view,doc=view.state.doc,path=tab.path,original=tab.original;
                try {
                    const file=await rpc('read',{path});
                    if(disposed || root!==startRoot || tab.closed || tab.path!==path || tab.view!==view || tab.loading || tab.saving || tab.agentLocked || view.state.doc!==doc || tab.original!==original)continue;
                    if(file.text===tab.original)continue;
                    if(tab.dirty) {hint('File changed on disk · Your unsaved editor text was preserved');continue;}
                    const content=normalizeText(file.text),before=doc.toString();
                    let from=0,suffix=0;
                    while(from<before.length && from<content.length && before[from]===content[from])from++;
                    while(suffix<before.length-from && suffix<content.length-from && before.at(-1-suffix)===content.at(-1-suffix))suffix++;
                    tab.original=file.text;tab.savedText=content;tab.bom=file.text.startsWith('\ufeff');tab.newline=file.text.includes('\r\n')?'\r\n':'\n';
                    if(before!==content)view.dispatch({changes:{from,to:before.length-suffix,insert:content.slice(from,content.length-suffix)}});
                    tab.dirty=false;renderTabs();
                } catch(error) {if(root===startRoot && !tab.closed)report(error);}
            }
        },
        agentContext() {
            return {selectedFile,buffers:tabs.filter(tab=>tab.type==='editor' && tab.path && tab.view && !tab.loading).map(tab=>({path:tab.path,text:(tab.bom?'\ufeff':'')+tab.view.state.doc.toString().replace(/\n/g,tab.newline),original:tab.original}))};
        },
        async applyProposal(review, expectedRoot) {
            const {pathKey,normalizeText}=window.dadoReview;
            const startRoot=root, locked=new Set(), targets=[];
            const lock=tab=>{if(locked.has(tab))return;tab.agentLocked=true;tab.view.dispatch({effects:tab.agentLock.reconfigure(EditorState.readOnly.of(true))});locked.add(tab);};
            const bufferText=tab=>(tab.bom?'\ufeff':'')+tab.view.state.doc.toString().replace(/\n/g,tab.newline);
            try {
                if(!root || pathKey(root)!==pathKey(expectedRoot)) throw new Error('Project changed before applying.');
                if(review.state!=='pending' || !review.changes?.length) throw new Error('There are no pending file changes.');
                const issue=window.dadoReview.applicationIssue(review);if(issue)throw new Error(issue);
                for(const file of review.changes) {
                    if(file.conflict) throw new Error(`${file.path} changed on disk. Resolve that conflict before applying.`);
                    const open=tabs.filter(tab=>tab.path && pathKey(tab.path)===pathKey(file.absolutePath));
                    for(const tab of open) {if(!tab.view || tab.loading || tab.saving || tab.agentLocked) throw new Error(`${file.path} is busy.`);lock(tab);}
                    targets.push({file,open});
                }
                // Check every disk baseline before changing any editor. Never save here.
                for(const {file,open} of targets) {
                    const disk=await rpc('read',{path:file.absolutePath});
                    if(disk.text!==file.diskBefore) throw new Error(`${file.path} changed on disk after review.`);
                    if(!open.length) {
                        const tab=createTab();createEditor(tab,null);
                        if(!await loadFile(tab,file.absolutePath)) throw new Error(`Could not open ${file.path}.`);
                        lock(tab);open.push(tab);
                    }
                    for(const tab of open) if(bufferText(tab)!==file.before || tab.original!==file.diskBefore) throw new Error(`${file.path} has editor changes outside this proposal. Your text was preserved.`);
                }
                if(root!==startRoot) throw new Error('Project changed before applying.');
                for(const {file,open} of targets) for(const tab of open) {
                    if(tab.closed || bufferText(tab)!==file.before) throw new Error(`${file.path} changed before applying.`);
                }
                for(const {file,open} of targets) for(const tab of open) tab.view.dispatch({changes:{from:0,to:tab.view.state.doc.length,insert:normalizeText(file.after)},userEvent:'input.agent'});
                activate(targets[0].open[0]);
                hint('Applied to editors · Save to write the files');
            } finally {
                for(const tab of locked) {tab.agentLocked=false;if(!tab.closed && tab.view) tab.view.dispatch({effects:tab.agentLock.reconfigure(EditorState.readOnly.of(false))});}
            }
        },
        openFile,
        editCommand(command, target) {
            const view = target?.closest?.('.cm-editor') && EditorView.findFromDOM(target.closest('.cm-editor'));
            if (!view) return false;
            view.focus();
            const commands = { undo, redo, selectAll, delete: deleteCharForward };
            if (commands[command]) { commands[command](view); return true; }
            if (['cut', 'copy', 'paste'].includes(command)) {
                const selection = view.state.selection.main;
                const text = view.state.sliceDoc(selection.from, selection.to);
                (async () => {
                    try {
                        if (command === 'paste') {
                            const content = await navigator.clipboard.readText();
                            view.dispatch(view.state.replaceSelection(content));
                        } else if (!selection.empty) {
                            await navigator.clipboard.writeText(text);
                            if (command === 'cut') view.dispatch({changes: {from: selection.from, to: selection.to, insert: ''}, userEvent: 'delete.cut'});
                        }
                    } catch { hint('Clipboard unavailable. Use the keyboard shortcut.'); }
                })();
                return true;
            }
            return false;
        },
        selectFile(path) {
            selectedFile = path;
            const tab = tabs.find(tab => tab.id === active);
            if (path && tab?.type === 'editor' && !tab.path && !tab.loading) loadFile(tab, path);
        },
        setRoot(path) { root = path; const tab = tabs.find(tab => tab.id === active); if (tab?.browser) renderExplorer(tab); },
        setVisible(shown) {
            if (shown !== visible) document.dispatchEvent(new CustomEvent('dado-settings-change',{detail:{rightPanel:shown}}));
            visible = shown; host.setAttribute('aria-hidden', String(!shown));
            if (shown && tabs.length === 0) newChoice();
            requestAnimationFrame(() => { const tab = tabs.find(tab => tab.id === active); tab?.view?.requestMeasure(); fitTerminal(tab); });
        },
        reply(message) {
            const request = pending.get(message.id); if (!request) return;
            pending.delete(message.id);
            if (message.error) request.reject(new Error(message.error)); else request.resolve(message.result);
        },
        terminalOutput(chunks) {
            for (const [id, bytes] of chunks) tabs.find(tab => tab.id === id)?.terminal?.write(new Uint8Array(bytes));
        },
        dispose() {
            if (disposed) return; disposed = true;
            resize.disconnect(); clearInterval(refreshBrowser); if (hintTimer) clearTimeout(hintTimer);
            for (const tab of tabs) { tab.closed = true; tab.commentCleanup?.(); tab.view?.destroy(); tab.terminal?.dispose(); }
            host.replaceChildren();
        },
    };
    send({ action: 'ready' });
};
