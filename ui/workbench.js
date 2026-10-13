import { EditorView, basicSetup } from 'codemirror';
import { keymap } from '@codemirror/view';
import { Decoration, ViewPlugin } from '@codemirror/view';
import { StateEffect, StateField, Compartment, EditorState } from '@codemirror/state';
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

function language(path) {
    const extension = path.split('.').pop().toLowerCase();
    if (extension === 'rs') return rust();
    if (['js', 'jsx', 'ts', 'tsx'].includes(extension)) return javascript({ typescript: extension.startsWith('t'), jsx: extension.endsWith('x') });
    return { json, py: python, md: markdown, html, css }[extension]?.() || [];
}
const editorTheme = EditorView.theme({
    '&': { height: '100%', fontSize: '12px', backgroundColor: 'var(--ui-workbench-content)', color: 'var(--ui-workbench-foreground)' },
    '.cm-scroller': { fontFamily: 'Consolas, "Cascadia Code", monospace', lineHeight: '1.7', overflow: 'auto' },
    '.cm-content': { padding: '12px 0', caretColor: 'var(--ui-accent)' },
    '.cm-line': { padding: '0 16px 0 8px' },
    '.cm-gutters': { backgroundColor: 'var(--ui-workbench-content)', color: 'var(--ui-muted)', border: 'none', paddingLeft: '8px' },
    '.cm-activeLine, .cm-activeLineGutter': { backgroundColor: 'var(--ui-workbench-hover)' },
    '&.cm-focused': { outline: 'none' },
    '.cm-selectionBackground, &.cm-focused .cm-selectionBackground': { backgroundColor: 'var(--ui-selection)' },
    '.cm-line.cm-source-flash': { backgroundColor: 'color-mix(in srgb, var(--ui-accent) 36%, transparent)' },
    '.cm-panels': { backgroundColor: 'var(--ui-workbench-control)', color: 'var(--ui-workbench-foreground)', border: 'none', fontFamily: '"Segoe UI", sans-serif' },
});
const sourceFlashEffect = StateEffect.define();
const sourceFlashField = StateField.define({
    create: () => Decoration.none,
    update(decorations, transaction) {
        let next = decorations.map(transaction.changes);
        for (const effect of transaction.effects)
            if (effect.is(sourceFlashEffect)) next = effect.value;
        return next;
    },
    provide: field => EditorView.decorations.from(field),
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
    function terminalTheme(terminalHost) {
        const surface = getComputedStyle(terminalHost);
        return {
            background: surface.backgroundColor,
            foreground: surface.color,
            cursor: surface.getPropertyValue('--ui-accent').trim(),
        };
    }
    function updateTerminalThemes() {
        for (const tab of tabs) if (tab.terminal && tab.terminalHost)
            tab.terminal.options.theme = terminalTheme(tab.terminalHost);
    }
    document.addEventListener('dado-appearance-change', updateTerminalThemes);
    const rpc = (action, data = {}) => new Promise((resolve, reject) => {
        const id = ++requestId;
        pending.set(id, { resolve, reject }); send({ action, id, ...data });
    });
    let notice = null;
    // Dioxus renders the shell. This adapter retains live widget sessions and
    // publishes their metadata; shell commands come from Dioxus event handlers.
    function renderTabs() {
        if (disposed) return;
        send({action: 'ui', state: {active, notice, tabs: tabs.map(tab => ({
            id: tab.id, kind: tab.type, title: tab.title, path: tab.path,
            dirty: tab.dirty, browser: tab.browser, loading: !!tab.loading,
            saving: !!tab.saving, locked: !!tab.agentLocked,
            operation: tab.pendingChange?.operation || null,
            commentTop: tab.commentTop ?? null, markers: tab.markers || [],
        }))}});
    }
    async function mounted(id) {
        const find = () => document.getElementById(id);
        if (find()) return find();
        return new Promise((resolve, reject) => {
            const observer = new MutationObserver(() => {
                const node = find();
                if (node) { observer.disconnect(); clearTimeout(timer); resolve(node); }
            });
            const timer = setTimeout(() => { observer.disconnect(); reject(new Error('Dioxus widget host did not mount.')); }, 10000);
            observer.observe(host, {childList: true, subtree: true});
        });
    }
    function report(error) {
        notice = {text: String(error.message || error), tab: null}; renderTabs();
        hint(notice.text, 4000);
    }
    function activate(tab) {
        active = tab.id; notice = null; renderTabs();
        if (tab.path) { selectedFile = tab.path; send({action: 'selected', path: tab.path}); }
        requestAnimationFrame(() => { tab.view?.requestMeasure(); fitTerminal(tab); });
    }
    function createTab() {
        const tab = { id: ++sequence, type: 'choice', title: 'New tab', dirty: false, browser: false, path: null };
        tabs.push(tab); activate(tab); return tab;
    }
    function newChoice() { return createTab(); }
    async function save(tab) {
        if (!tab.view || !tab.path || tab.saving || tab.agentLocked) return false;
        tab.saving = true; renderTabs();
        const snapshot = tab.view.state.doc.toString();
        const text = (tab.bom ? '\ufeff' : '') + snapshot.replace(/\n/g, tab.newline);
        try {
            if(tab.pendingChange) {
                const change=tab.pendingChange;
                await window.dadoChat.saveReviewedFile(change,text);
                if(change.operation==='delete') {
                    for(const entry of [...tabs]) if(entry.pendingChange?.requestId===change.requestId && entry.path===tab.path)removeTab(entry);
                    hint('Deleted reviewed file',1800);return true;
                }
                for(const entry of tabs) if(entry.pendingChange?.requestId===change.requestId && entry.path===tab.path){entry.pendingChange=null;entry.original=text;entry.savedText=snapshot;entry.dirty=entry.view.state.doc.toString()!==snapshot;}
            } else await rpc('save', { path: tab.path, text, expected: tab.original });
            window.dispatchEvent(new CustomEvent('dado-scene-activity',{detail:{root,action:'changed',paths:[tab.path],status:'completed'}}));
            tab.original = text; tab.savedText = snapshot;
            tab.dirty = tab.view.state.doc.toString() !== snapshot; renderTabs();
            hint(`Saved ${tab.path}`, 1800); return true;
        } catch (error) { report(error); return false; }
        finally { tab.saving = false; renderTabs(); }
    }
    function closeTab(tab) {
        if (tab.agentLocked) return;
        if (tab.dirty) {
            const deleting = tab.pendingChange?.operation === 'delete';
            notice = {text: deleting ? `Delete ${tab.title}?` : `Save changes to ${tab.title}?`, tab: tab.id, deleting};
            renderTabs(); return;
        }
        removeTab(tab);
    }
    function removeTab(tab) {
        if (tab.agentLocked || tab.closed) return;
        if (tab.sourceFlashTimer) clearTimeout(tab.sourceFlashTimer);
        tab.closed = true; tab.commentCleanup?.();
        tab.view?.destroy(); tab.terminal?.dispose(); tab.terminalResize?.dispose();
        if (tab.type === 'terminal') rpc('terminal_close', {tab: tab.id}).catch(report);
        const index = tabs.indexOf(tab); tabs.splice(index, 1); notice = null;
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
        tab.type = 'editor'; tab.title = 'Open file';
        tab.commentListRect = () => document.querySelector('.chat-composer')?.getBoundingClientRect()
            || document.getElementById(`tab-body-${tab.id}`).getBoundingClientRect();
        renderTabs(); if (path) loadFile(tab, path);
    }
    async function loadFile(tab, path, initialFile=null) {
        if(tab.agentLocked) return;
        tab.loading = true;
        try {
            const file = initialFile || await rpc('read', { path });
            if (tab.closed) return;
            tab.path = file.path; tab.title = file.path.split(/[\\/]/).pop();
            tab.pendingChange=null;tab.original = file.exists===false?null:file.text; tab.bom = file.text.startsWith('\ufeff');
            tab.newline = file.text.includes('\r\n') ? '\r\n' : '\n';
            const content = file.text.replace(/^\ufeff/, '').replace(/\r\n/g, '\n');
            tab.savedText = content; tab.dirty = false;
            renderTabs();
            tab.editorHost = await mounted(`editor-widget-${tab.id}`);
            if (tab.closed) return;
            tab.view?.destroy(); tab.editorHost.replaceChildren();
            tab.commentCleanup?.();
            const refreshComments = StateEffect.define();
            const annotations = window.dadoComments;
            const commentPlugin = ViewPlugin.fromClass(class {
                constructor(view) { this.decorations = this.build(view); }
                build(view) {
                    return Decoration.set(annotations.comments.filter(item => item.source === tab.path && item.to > item.from && item.to <= view.state.doc.length)
                        .map(item => Decoration.mark({class: 'comment-code-highlight', attributes: {'data-comment': String(item.id)}}).range(item.from, item.to)), true);
                }
                update(update) {
                    if (update.docChanged) annotations.comments.filter(item => item.source === tab.path).forEach(item => { item.from = update.changes.mapPos(item.from, 1); item.to = update.changes.mapPos(item.to, -1); if (item.to < item.from) item.to = item.from; });
                    if (update.docChanged) queueMicrotask(() => annotations.changed());
                    annotations.comments.filter(item => item.source === tab.path).forEach(item => { item.lineFrom = update.state.doc.lineAt(Math.min(item.from, update.state.doc.length)).number; item.lineTo = update.state.doc.lineAt(Math.min(Math.max(item.from, item.to - 1), update.state.doc.length)).number; });
                    this.decorations = this.build(update.view);
                    requestAnimationFrame(() => tab.positionComment?.());
                }
            }, {decorations: plugin => plugin.decorations});
            tab.agentLock = new Compartment();
            tab.view = new EditorView({
                parent: tab.editorHost, doc: content,
                extensions: [basicSetup, editorTheme, language(file.path), commentPlugin, sourceFlashField, tab.agentLock.of(EditorState.readOnly.of(false)), keymap.of([
                    { key: 'Mod-s', run: () => { save(tab); return true; } }, indentWithTab,
                ]), EditorView.updateListener.of(update => {
                    if (update.docChanged) { tab.dirty = !!tab.pendingChange || update.state.doc.toString() !== tab.savedText; renderTabs(); }
                    if (update.selectionSet) {
                        requestAnimationFrame(() => tab.positionComment?.());
                        const head = update.state.selection.main.head;
                        const line = update.state.doc.lineAt(head);
                        hint(`${tab.path} · Ln ${line.number}, Col ${head - line.from + 1}`);
                    }
                })],
            });
            tab.positionComment = () => {
                if (tab.closed || !tab.view) return;
                const range = tab.view.state.selection.main;
                const coords = tab.view.coordsAtPos(range.head), bounds = tab.editorHost.getBoundingClientRect();
                tab.commentTop = coords && coords.top >= bounds.top && coords.bottom <= bounds.bottom
                    ? Math.max(0, coords.top - bounds.top - 3) : null;
                const grouped = new Map();
                annotations.comments.filter(item => item.source === tab.path).forEach(item => {
                    const line = tab.view.state.doc.lineAt(Math.min(item.from, tab.view.state.doc.length));
                    if (!grouped.has(line.from)) grouped.set(line.from, []);
                    grouped.get(line.from).push(item);
                });
                tab.markers = [];
                grouped.forEach((items, pos) => {
                    const location = tab.view.coordsAtPos(pos);
                    if (location && location.top >= bounds.top && location.bottom <= bounds.bottom)
                        tab.markers.push({top: location.top - bounds.top - 3, ids: items.map(item => item.id).join(', ')});
                });
                renderTabs();
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
            renderTabs();
            requestAnimationFrame(() => { tab.view.requestMeasure(); if (tab.id === active && visible) tab.view.focus(); });
            return tab;
        } catch (error) { report(error); }
        finally { tab.loading = false; renderTabs(); }
    }
    async function openFile(path, range) {
        const existing = tabs.find(tab => tab.path === path);
        const reveal = tab => {
            const view = tab.view;
            if (!view) return;
            if (tab.sourceFlashTimer) { clearTimeout(tab.sourceFlashTimer); tab.sourceFlashTimer = null; }
            if (range?.start) {
                const doc = view.state.doc;
                const first = Math.min(doc.lines, Math.max(1, range.start));
                const last = Math.min(doc.lines, Math.max(first, range.end || first));
                const start = doc.line(first).from, end = doc.line(last).to;
                const flash = Decoration.set(Array.from({length: last - first + 1}, (_, index) =>
                    Decoration.line({class: 'cm-source-flash'}).range(doc.line(first + index).from)));
                view.dispatch({
                    selection: {anchor: start, head: end},
                    effects: [EditorView.scrollIntoView(start, {y: 'center'}), sourceFlashEffect.of(flash)],
                });
                tab.sourceFlashTimer = setTimeout(() => {
                    if (!tab.closed && tab.view === view)
                        view.dispatch({effects: sourceFlashEffect.of(Decoration.none)});
                    tab.sourceFlashTimer = null;
                }, 500);
            } else {
                view.dispatch({effects: sourceFlashEffect.of(Decoration.none)});
            }
            view.focus();
        };
        if (existing) { activate(existing); reveal(existing); return; }
        let tab = tabs.find(tab => tab.id === active && tab.type === 'editor' && !tab.path && !tab.loading);
        if (!tab) { tab = createTab(); createEditor(tab, null); }
        activate(tab); await loadFile(tab, path); reveal(tab);
    }
    async function createTerminal(tab) {
        tab.type = 'terminal'; tab.title = 'Terminal'; renderTabs();
        const terminalHost = await mounted(`terminal-widget-${tab.id}`);
        if (tab.closed) return;
        tab.terminalHost = terminalHost;
        tab.terminal = new Terminal({
            fontFamily: 'Consolas, "Cascadia Code", monospace', fontSize: 12, lineHeight: 1.35,
            cursorBlink: true, scrollback: 5000,
            theme: terminalTheme(terminalHost),
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
        if (visible && tab?.type === 'terminal' && tab.id === active && host.clientWidth > 0) {
            try { tab.fit?.fit(); } catch { /* Another layout frame will retry. */ }
        }
    }
    const resize = new ResizeObserver(() => {
        if (!visible) return;
        for (const tab of tabs) if (tab.id === active) { tab.view?.requestMeasure(); fitTerminal(tab); }
    });
    resize.observe(host);
    window.dadoWorkbench = {
        movedPath(from,to) {
            const move=path=>path && (path===from || path.startsWith(from+'\\') || path.startsWith(from+'/')) ? to+path.slice(from.length) : path;
            for(const tab of tabs) {const next=move(tab.path);if(next!==tab.path){tab.path=next;tab.title=next.split(/[\\/]/).pop();}}
            selectedFile=move(selectedFile);
            for(const item of window.dadoComments.comments) {
                item.source=move(item.source);item.folder=move(item.folder);item.sceneFolder=move(item.sceneFolder);
                if(item.source==='Systems')item.quote=item.quote.split(from).join(to);
            }
            window.dadoComments.changed();renderTabs();
        },
        command(action, id, value) {
            const tab = tabs.find(tab => tab.id === id);
            if (action === 'new') return newChoice();
            if (action === 'dismiss') { notice = null; renderTabs(); return; }
            if (!tab || tab.closed) return;
            if (action === 'activate') return activate(tab);
            if (action === 'close') return closeTab(tab);
            if (action === 'discard') return removeTab(tab);
            if (action === 'save-close') return save(tab).then(ok => {if (ok && !tab.dirty && !tab.closed) removeTab(tab);});
            if (action === 'editor' && tab.type === 'choice') return createEditor(tab, selectedFile);
            if (action === 'terminal' && tab.type === 'choice') return createTerminal(tab).catch(report);
            if (action === 'pick') return pick(tab);
            if (action === 'save') return save(tab);
            if (action === 'browser') {tab.browser = !tab.browser; renderTabs(); requestAnimationFrame(() => tab.view?.requestMeasure()); return;}
            if (action === 'comment-list') {
                const ids = String(value).split(', ').map(Number);
                return window.dadoComments.list(tab.commentListRect(), window.dadoComments.comments.filter(item => item.source === tab.path && ids.includes(item.id)));
            }
            if (action === 'comment' && tab.view) {
                const selection = tab.view.state.selection.main, line = tab.view.state.doc.lineAt(selection.head);
                const from = selection.empty ? line.from : selection.from, to = selection.empty ? line.to : selection.to;
                const quote = tab.view.state.doc.sliceString(from, to) || `Line ${line.number} (empty)`;
                window.dadoComments.begin({quote, source: tab.path, from, to, lineFrom: tab.view.state.doc.lineAt(from).number,
                    lineTo: tab.view.state.doc.lineAt(Math.max(from, to - 1)).number},
                    document.getElementById(`code-comment-${tab.id}`).getBoundingClientRect());
            }
        },
        async refreshFromDisk(expectedRoot) {
            const startRoot=root,{pathKey,normalizeText}=window.dadoReview;
            if(!root || pathKey(root)!==pathKey(expectedRoot))return;
            const prefix=pathKey(root).replace(/\/$/,'')+'/';
            for(const tab of tabs.filter(tab=>tab.type==='editor' && tab.path && tab.view && pathKey(tab.path).startsWith(prefix))) {
                if(tab.closed || tab.loading || tab.saving || tab.agentLocked || tab.pendingChange)continue;
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
            return {selectedFile,buffers:tabs.filter(tab=>tab.type==='editor' && tab.path && tab.view && !tab.loading).map(tab=>({path:tab.path,text:(tab.bom?'\ufeff':'')+tab.view.state.doc.toString().replace(/\n/g,tab.newline),original:tab.original,exists:tab.pendingChange?.operation!=='create'}))};
        },
        async applyProposal(review, expectedRoot, owner) {
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
                    for(const tab of open) {if(!tab.view || tab.loading || tab.saving || tab.agentLocked || tab.pendingChange) throw new Error(`${file.path} is busy.`);lock(tab);}
                    targets.push({file,open});
                }
                // Check every disk baseline before changing any editor. Never save here.
                for(const {file,open} of targets) {
                    const disk=await rpc('read_state',{path:file.absolutePath});
                    if((disk.exists===false?null:disk.text)!==file.diskBefore) throw new Error(`${file.path} changed on disk after review.`);
                    if(!open.length) {
                        const tab=createTab();createEditor(tab,null);
                        if(!await loadFile(tab,file.absolutePath,file.operation==='create'?{path:file.absolutePath,text:'',exists:false}:null)) throw new Error(`Could not open ${file.path}.`);
                        lock(tab);open.push(tab);
                    }
                    for(const tab of open) if(bufferText(tab)!==file.before || tab.original!==file.diskBefore) throw new Error(`${file.path} has editor changes outside this proposal. Your text was preserved.`);
                }
                if(root!==startRoot) throw new Error('Project changed before applying.');
                for(const {file,open} of targets) for(const tab of open) {
                    if(tab.closed || bufferText(tab)!==file.before) throw new Error(`${file.path} changed before applying.`);
                }
                for(const {file,open} of targets) for(const tab of open) {
                    if(file.operation==='create' || file.operation==='delete') {
                        tab.pendingChange={...owner,path:file.path,revision:file.revision,operation:file.operation};
                        tab.dirty=true;
                    }
                    if(file.operation!=='delete') {
                        if(file.operation==='create'){tab.bom=file.after.startsWith('\ufeff');tab.newline=file.after.includes('\r\n')?'\r\n':'\n';}
                        tab.view.dispatch({changes:{from:0,to:tab.view.state.doc.length,insert:normalizeText(file.after)},userEvent:'input.agent'});
                    }
                }
                renderTabs();
                activate(targets[0].open[0]);
                hint('Applied to editors · Save to write the files');
            } finally {
                for(const tab of locked) {tab.agentLocked=false;if(!tab.closed && tab.view) tab.view.dispatch({effects:tab.agentLock.reconfigure(EditorState.readOnly.of(tab.pendingChange?.operation==='delete'))});}
                renderTabs();
            }
        },
        openFile,
        editCommand(command, target) {
            const view = target?.closest?.('.cm-editor') && EditorView.findFromDOM(target.closest('.cm-editor'));
            if (!view) return false;
            if (view.state.readOnly && !['copy', 'selectAll'].includes(command)) return true;
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
        setRoot(path) { root = path; },
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
            resize.disconnect(); document.removeEventListener('dado-appearance-change', updateTerminalThemes); if (hintTimer) clearTimeout(hintTimer);
            for (const tab of tabs) { tab.closed = true; tab.commentCleanup?.(); tab.view?.destroy(); tab.terminal?.dispose(); }

        },
    };
    send({ action: 'ready' });
};
