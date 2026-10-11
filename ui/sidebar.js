window.startSidebar = function (send, saved, avatar, loadError) {
    const host = document.getElementById('left-sidebar');
    const settings = document.getElementById('settings-host');
    const node = (tag, cls, text) => { const el = document.createElement(tag); el.className = cls; if (text !== undefined) el.textContent = text; return el; };
    const icons = {
        new: 'M13 5H5v14h14v-8M12 12l8-8M17 4h3v3',
        arrow: 'm9 5 7 7-7 7', plus: 'M12 5v14M5 12h14',
        concept: 'M4 7h6l2-2h8v14H4z', more: 'M5 12h.01M12 12h.01M19 12h.01',
        settings: 'M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8M9 3h6l1 3 3 1 2 5-2 5-3 1-1 3H9l-1-3-3-1-2-5 2-5 3-1z',
        usage: 'M4 17a9 9 0 1 1 16 0M12 14l4-6',
        close: 'm6 6 12 12M18 6 6 18', external: 'M14 4h6v6M20 4l-9 9M10 5H4v15h15v-6',
        codex: 'm9 6-6 6 6 6M15 6l6 6-6 6M14 4l-4 16',
    };
    const glyph = key => `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="${icons[key]}"/></svg>`;
    const hint = text => send({action: 'hint', text});
    function help(el, text) {
        el.addEventListener('mouseenter', () => hint(typeof text === 'function' ? text() : text));
        el.addEventListener('mouseleave', () => hint(null));
        el.addEventListener('focus', () => hint(typeof text === 'function' ? text() : text));
        el.addEventListener('blur', () => hint(null));
    }
    function button(label, icon, action, cls = 'sidebar-row') {
        const b = node('button', cls); b.type = 'button';
        if (icon) b.innerHTML = glyph(icon);
        if (label) b.append(node('span', '', label));
        b.setAttribute('aria-label', label); b.onclick = action;
        return b;
    }
    const id = () => crypto.randomUUID?.() || Array.from(crypto.getRandomValues(new Uint8Array(16)), byte => byte.toString(16).padStart(2, '0')).join('');
    const valid = saved && saved.version === 1 && Array.isArray(saved.chats) && Array.isArray(saved.concepts);
    const state = valid ? saved : {version: 1, chats: [], concepts: [], active: null, preferences: {name: '', personality: ''}, expanded: {recents: true, concepts: true}};
    state.preferences ||= {name: '', personality: ''}; state.expanded ||= {recents: true, concepts: true};
    let activeConcept = state.chats.find(chat => chat.id === state.active)?.concept || null;
    let showAll = false, popup = null, popupTrigger = null, timer = null, settingsTab = 'personalization';
    const pathRefreshers = new Map();
    let storageProblem = loadError || (saved && !valid ? 'Saved chats use an unsupported format. The original file has been preserved.' : null);
    function persist() { if (!loadError && !(saved && !valid)) send({action: 'save', state}); }
    function later() { clearTimeout(timer); timer = setTimeout(persist, 200); }
    function capture() {
        const chat = state.chats.find(c => c.id === state.active);
        if (chat && window.dadoChat?.snapshot) chat.content = window.dadoChat.snapshot();
    }
    function closePopup() { popup?.remove(); popup = null; popupTrigger?.setAttribute('aria-expanded', 'false'); popupTrigger = null; }
    function openPopup(trigger, label) {
        closePopup(); popupTrigger = trigger; trigger.setAttribute('aria-expanded', 'true');
        popup = node('section', 'sidebar-popup'); popup.setAttribute('aria-label', label);
        popup.setAttribute('role', 'dialog'); host.append(popup);
        const rect = trigger.getBoundingClientRect(), bounds = host.getBoundingClientRect();
        popup.style.bottom = `${Math.max(54, bounds.bottom - rect.top + 6)}px`;
        return popup;
    }
    function select(chat) {
        capture(); closePopup(); closeSettings(false);
        state.active = chat.id; activeConcept = chat.concept || null; chat.visited = Date.now();
        window.dadoChat?.setConversation(chat.id, chat.content || {});
        render(); later();
    }
    function newChat(concept = activeConcept) {
        capture();
        const chat = {id: id(), title: 'New chat', concept: concept || null, visited: Date.now(), content: {messages: [], draft: '', attachments: []}};
        state.chats.unshift(chat); select(chat);
    }
    function syncChat() {
        if (!state.chats.some(chat => chat.id === state.active)) newChat(null);
        else window.dadoChat?.setConversation(state.active, state.chats.find(chat => chat.id === state.active).content || {});
    }
    function heading(label, key, target) {
        const b = button(label, 'arrow', () => {state.expanded[key] = !state.expanded[key]; render(); later();}, 'sidebar-heading');
        b.setAttribute('aria-expanded', String(state.expanded[key])); b.setAttribute('aria-controls', target);
        return b;
    }
    function chatRow(chat) {
        const b = button(chat.title, null, () => select(chat), 'sidebar-row sidebar-chat');
        b.classList.toggle('selected', chat.id === state.active); b.setAttribute('aria-current', chat.id === state.active ? 'true' : 'false');
        help(b, chat.title); return b;
    }
    function conceptEditor(concept, trigger) {
        const panel = openPopup(trigger, concept ? `Edit concept ${concept.name}` : 'Create concept');
        panel.classList.add('concept-editor');
        const form = node('form', 'concept-name-form');
        const label = node('label', '', 'Concept name'), input = node('input', 'settings-input'); input.value = concept?.name || ''; input.maxLength = 100; input.required = true;
        input.id = 'concept-name'; label.htmlFor = input.id;
        const submit = button(concept ? 'Save' : 'Create', null, null, 'sidebar-row concept-save'); submit.type = 'submit';
        form.append(label, input, submit); panel.append(form);
        form.onsubmit = event => {
            event.preventDefault(); const name = input.value.trim(); if (!name) {input.focus(); return;}
            if (concept) {concept.name = name; closePopup(); render(); later();}
            else {const created = {id: id(), name, paths: [], open: true}; state.concepts.push(created); newChat(created.id); conceptEditor(created, host.querySelector(`[data-concept="${created.id}"]`));}
        };
        if (concept) {
            const paths = node('div', 'concept-paths'); panel.append(paths);
            function showPaths() {
                paths.replaceChildren();
                concept.paths.forEach(item => {
                    const row = node('div', 'concept-path'), name = node('span', '', item.path.split(/[\\/]/).filter(Boolean).pop() || item.path);
                    help(name, item.path);
                    const remove = button(`Remove ${name.textContent}`, 'close', () => {concept.paths = concept.paths.filter(p => p !== item); showPaths(); renderConceptCount(); later();}, 'sidebar-icon');
                    remove.querySelector('span').remove(); row.append(name, remove); paths.append(row);
                });
            }
            pathRefreshers.set(concept.id, showPaths); showPaths();
            const actions = node('div', 'concept-path-actions');
            for (const [label, kind] of [['Add files', 'file'], ['Add folders', 'folder']]) {
                const b = button(label, 'plus', () => send({action: 'pick', concept: concept.id, kind}), 'sidebar-row');
                help(b, 'Choose local context for this concept · AI is not connected yet'); actions.append(b);
            }
            panel.append(actions);
        }
        const cancel = button('Close concept details', 'close', () => {closePopup(); trigger?.focus();}, 'sidebar-icon popup-close'); cancel.querySelector('span').remove(); panel.prepend(cancel); input.focus();
    }
    function renderConceptCount() {
        const label = host.querySelector(`[data-concept="${activeConcept}"]`);
        if (label) label.setAttribute('aria-label', `Edit concept ${state.concepts.find(c => c.id === activeConcept)?.name || ''}`);
    }
    function render() {
        host.querySelector('.sidebar-content')?.remove(); host.querySelector('.sidebar-account')?.remove();
        const content = node('div', 'sidebar-content'); host.prepend(content);
        content.append(node('div', 'sidebar-brand', 'ᜊᜐ᜔ᜃᜄᜈ᜔'));
        const create = button('New Chat', 'new', () => newChat(null)); help(create, 'Start a new local chat'); content.append(create);
        content.append(heading('Recents', 'recents', 'recent-chats'));
        const recents = node('div', 'sidebar-section'); recents.id = 'recent-chats'; recents.hidden = !state.expanded.recents;
        const recent = [...state.chats].filter(c => c.content?.messages?.length || c.content?.draft?.trim() || c.id === state.active).sort((a, b) => b.visited - a.visited);
        recent.slice(0, showAll ? undefined : 7).forEach(chat => recents.append(chatRow(chat)));
        const more = button(showAll ? 'Show less' : 'Show more', null, () => {showAll = !showAll; render();}, 'sidebar-row sidebar-more'); more.disabled = recent.length <= 7; recents.append(more); content.append(recents);
        const conceptsHead = node('div', 'sidebar-heading-row'); conceptsHead.append(heading('Concepts', 'concepts', 'concept-list'));
        const add = button('Create concept', 'plus', () => conceptEditor(null, add), 'sidebar-icon'); add.querySelector('span').remove(); help(add, 'Create a concept from related files, folders and chats'); conceptsHead.append(add); content.append(conceptsHead);
        const concepts = node('div', 'sidebar-section'); concepts.id = 'concept-list'; concepts.hidden = !state.expanded.concepts;
        for (const concept of state.concepts) {
            const group = node('div', 'concept-group'), row = node('div', 'concept-row');
            const toggle = button(concept.name, 'concept', () => {concept.open = !concept.open; render(); later();}); toggle.setAttribute('aria-expanded', String(concept.open));
            help(toggle, () => `${concept.name} · ${concept.paths.length} context ${concept.paths.length === 1 ? 'item' : 'items'} · ${state.chats.filter(c => c.concept === concept.id).length} chats`);
            const edit = button(`Edit concept ${concept.name}`, 'more', () => conceptEditor(concept, edit), 'sidebar-icon'); edit.dataset.concept = concept.id; edit.querySelector('span').remove();
            const addChat = button(`New chat in ${concept.name}`, 'new', () => newChat(concept.id), 'sidebar-icon'); addChat.querySelector('span').remove();
            row.append(toggle, edit, addChat); group.append(row);
            if (concept.open) {const chats = node('div', 'concept-chats'); state.chats.filter(c => c.concept === concept.id).sort((a, b) => b.visited - a.visited).forEach(chat => chats.append(chatRow(chat))); group.append(chats);}
            concepts.append(group);
        }
        content.append(concepts);
        if (storageProblem) {const problem = node('p', 'sidebar-storage-error', storageProblem); problem.setAttribute('role', 'alert'); content.append(problem);}
        const account = button('', null, () => accountMenu(account), 'sidebar-account'); account.setAttribute('aria-label', 'Account and settings'); account.setAttribute('aria-expanded', 'false');
        const image = node('img', 'dioxus-avatar'); image.src = avatar; image.alt = 'Dioxus'; account.append(image, node('span', 'account-name', state.preferences.name.trim() || 'You')); host.append(account); help(account, 'Account and settings');
    }
    function accountMenu(trigger) {
        if (popupTrigger === trigger) {closePopup(); return;}
        const panel = openPopup(trigger, 'Account and settings');
        panel.append(node('div', 'account-menu-name', state.preferences.name.trim() || 'You'));
        const usage = node('div', 'account-usage'); usage.innerHTML = glyph('usage'); usage.append(node('span', '', 'Usage remaining'), node('span', 'usage-value', 'Unavailable')); panel.append(usage);
        usage.querySelector('.usage-value').textContent = usageRemaining();
        help(usage, 'Remaining included Codex usage in each reported window');
        panel.append(button('Settings', 'settings', () => {closePopup(); openSettings();})); panel.querySelector('button').focus();
    }
    function usageRemaining() {
        const limits=window.dadoChat?.aiState()?.usage;
        const bucket=limits?.rateLimitsByLimitId?.codex || limits?.rateLimits || Object.values(limits?.rateLimitsByLimitId || {})[0];
        const values=[bucket?.primary,bucket?.secondary].filter(w=>w && Number.isFinite(w.usedPercent)).map(w=>{
            const minutes=w.windowDurationMins;
            const duration=minutes ? minutes%1440===0 ? `${minutes/1440}d ` : minutes%60===0 ? `${minutes/60}h ` : `${minutes}m ` : '';
            return `${duration}${Math.round(Math.max(0,Math.min(100,100-w.usedPercent)))}%`;
        });
        return values.length ? values.join(' · ') : 'Unavailable';
    }
    function closeSettings(focus = true) {
        if (settings.hidden) return;
        settings.hidden = true;
        document.querySelector('.app-body').classList.remove('settings-open');
        document.querySelector('.workspace').inert = false;
        host.inert = false;
        if (focus) host.querySelector('.sidebar-account')?.focus();
    }
    function openSettings() {
        settings.hidden = false;
        document.querySelector('.app-body').classList.add('settings-open');
        document.querySelector('.workspace').inert = true;
        host.inert = true;
        settings.replaceChildren();
        const nav = node('nav', 'settings-nav'); nav.setAttribute('aria-label', 'Settings sections');
        nav.append(node('h2', '', 'Settings'));
        for (const [key, label] of [['personalization', 'Personalization'], ['model', 'Model'], ['agent','Agent']]) {
            const b = button(label, null, () => {settingsTab = key; openSettings();}); b.classList.toggle('selected', settingsTab === key); b.setAttribute('aria-current', settingsTab === key ? 'page' : 'false'); nav.append(b);
        }
        const page = node('div', 'settings-page'), top = node('div', 'settings-page-heading'); top.append(node('h1', '', settingsTab === 'model' ? 'Model' : settingsTab==='agent' ? 'Agent' : 'Personalization'));
        const close = button('Close settings', 'close', () => closeSettings(), 'sidebar-icon'); close.querySelector('span').remove(); top.append(close); page.append(top);
        if (settingsTab === 'personalization') {
            for (const [key, title, tag] of [['name', 'Name', 'input'], ['personality', 'Customize Personality', 'textarea']]) {
                const block = node('div', 'settings-block'), label = node('label', '', title), field = node(tag, 'settings-input');
                field.id = `personalization-${key}`; label.htmlFor = field.id; field.value = state.preferences[key] || '';
                field.maxLength = key === 'name' ? 100 : 12000;
                if (key === 'name') field.autocomplete = 'given-name'; else field.rows = 4;
                field.oninput = () => {state.preferences[key] = field.value; const name = host.querySelector('.account-name'); if (name) name.textContent = state.preferences.name.trim() || 'You'; later();};
                help(field, key === 'name' ? 'What the AI should call you · Saved locally' : 'How you want the AI to speak · Saved locally for the future AI connection');
                block.append(label, field); page.append(block);
            }
        } else if(settingsTab==='agent') {
            const ai=window.dadoChat?.aiState() || {},prefs=window.dadoChat?.modelPreferences() || {};
            const block=node('div','settings-block'),label=node('label','','Permissions'),select=node('select','settings-input');select.id='settings-agent-mode';label.htmlFor=select.id;
            for(const [value,title] of [['chat','Chat'],['inspect','Inspect'],['approve','On approval'],['full','Full access']]) {const option=node('option','',title);option.value=value;option.disabled=value!=='chat' && !ai.inspectionAvailable;select.append(option);}
            select.value=prefs.agentMode || (ai.inspectionAvailable ? 'inspect' : 'chat');
            select.onchange=()=>document.dispatchEvent(new CustomEvent('dado-settings-change',{detail:{agentMode:select.value}}));
            help(select,'Applies next turn · On approval asks before each command · Full access enables live edits and commands without prompts');block.append(label,select);page.append(block);
        } else {
            const ai=window.dadoChat?.aiState() || {};
            const row=node('div','settings-block codex-auth'); row.innerHTML=glyph('codex'); row.append(node('span','','Codex auth'));
            const action=ai.signingIn ? 'ai-login-cancel' : ai.account ? 'ai-logout' : ai.ready ? 'ai-login' : 'ai-refresh';
            const label=ai.signingIn ? 'Cancel sign-in' : ai.account ? 'Sign out' : ai.ready ? 'Sign in' : 'Reconnect';
            const signin=button(label, ai.account || ai.signingIn ? null : 'external',()=>window.dadoChat?.aiAction(action),'auth-signin');row.append(signin);page.append(row);
            if (ai.account) {const account=node('div','settings-account',ai.account.email || 'Signed in');page.append(account);}
            if (ai.error) {const error=node('p','settings-error',ai.error);error.setAttribute('role','alert');page.append(error);}
            if (ai.account) {
                const refresh=button('Refresh models',null,()=>window.dadoChat?.aiAction('ai-refresh'),'auth-signin');page.append(refresh);
                if (ai.models?.length) {
                    const block=node('div','settings-block'),label=node('label','','Model'),select=node('select','settings-input');select.id='settings-model';label.htmlFor=select.id;
                    const prefs=window.dadoChat?.modelPreferences() || {};
                    ai.models.forEach(model=>{const option=node('option','',model.displayName || model.model);option.value=model.model;select.append(option);});select.value=prefs.model || ai.models[0].model;
                    select.onchange=()=>{window.dadoChat?.selectModel(select.value);openSettings();settings.querySelector('#settings-model')?.focus();};block.append(label,select);page.append(block);
                    const model=ai.models.find(m=>m.model===select.value),efforts=model?.supportedReasoningEfforts || [];
                    if (efforts.length) {
                        const group=node('div','settings-block'),label=node('label','','Reasoning'),select=node('select','settings-input');select.id='settings-reasoning';label.htmlFor=select.id;
                        efforts.forEach(e=>{const option=node('option','',e.reasoningEffort);option.value=e.reasoningEffort;select.append(option);});select.value=prefs.reasoningEffort || model.defaultReasoningEffort;
                        select.onchange=()=>document.dispatchEvent(new CustomEvent('dado-settings-change',{detail:{reasoningEffort:select.value}}));group.append(label,select);page.append(group);
                    }
                }
            }

        }
        settings.append(nav, page); settings.querySelector('.settings-nav .selected')?.focus();
    }
    const outside = event => {if (popup && !popup.contains(event.target) && !popupTrigger?.contains(event.target)) closePopup();};
    const escape = event => {if (event.key === 'Escape') {if (popup) {const trigger = popupTrigger; closePopup(); trigger?.focus();} else if (!settings.hidden) closeSettings();}};
    document.addEventListener('pointerdown', outside);
    document.addEventListener('keydown', escape);
    window.dadoSidebar = {
        refreshAI() {
            const usage=host.querySelector('.usage-value');if (usage) usage.textContent=usageRemaining();
            if (!settings.hidden && settingsTab==='model') openSettings();
        },
        openModelSettings() {closePopup(); settingsTab = "model"; openSettings();},
        openAgentSettings() {closePopup();settingsTab='agent';openSettings();},
        newChat() {newChat(null);},
        snapshot() {capture(); return state;},
        dispose() {clearTimeout(timer); closePopup(); closeSettings(false); document.removeEventListener('pointerdown',outside); document.removeEventListener('keydown',escape); window.dadoSidebar = null;},
        changed() {capture(); const chat = state.chats.find(c => c.id === state.active); if (chat) {const first = chat.content.messages?.[0]; chat.title = (first?.text || first?.attachments?.[0]?.name || first?.comments?.[0]?.text || 'New chat').replace(/\s+/g, ' ').slice(0, 70); chat.visited = Date.now();} render(); later();},
        draftChanged() {capture(); later();},
        attachmentsChanged(chatId, attachments) {const chat = state.chats.find(c => c.id === chatId); if (chat) {chat.content.attachments = attachments.map(file => ({...file})); later();}},
        context() {const concept = state.concepts.find(c => c.id === activeConcept); return {concept: concept ? {id: concept.id, name: concept.name, paths: concept.paths.map(p => ({...p}))} : null, personalization: {...state.preferences}};},
        addPaths(conceptId, paths) {const concept = state.concepts.find(c => c.id === conceptId); if (!concept) return; paths.forEach(path => {if (!concept.paths.some(p => p.path === path.path)) concept.paths.push(path);}); pathRefreshers.get(concept.id)?.(); later();},
        storageError(message) {storageProblem = message; render();},
    };
    render(); if (window.dadoChat?.setConversation) syncChat();
};
