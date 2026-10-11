if (!window.dadoComments) {
    const comments = [];
    const listeners = new Set();
    let next = 0, popup = null, draft = null;
    const glyph = '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 4h16v12H9l-5 4z"/></svg>';
    const identity = () => crypto.randomUUID?.() || `${Date.now()}-${Array.from(crypto.getRandomValues(new Uint32Array(3))).join('-')}`;
    const node = (tag, cls, text) => { const el = document.createElement(tag); el.className = cls; if (text !== undefined) el.textContent = text; return el; };
    function close() { popup?.remove(); popup = null; draft = null; }
    function position(el, rect) {
        const width = Math.min(380, window.innerWidth - 32);
        el.style.width = `${width}px`;
        el.style.left = `${Math.max(16, Math.min(rect.right - width, window.innerWidth - width - 16))}px`;
        el.style.top = `${Math.max(16, Math.min(rect.bottom + 8, window.innerHeight - el.offsetHeight - 16))}px`;
    }
    function notify() { if (!comments.length) next = 0; listeners.forEach(listener => listener(comments)); }
    function edit(item, rect, fresh = false) {
        close(); draft = item;
        popup = node('form', 'annotation-popup annotation-edit'); popup.setAttribute('aria-label', 'Comment');
        const quote = node('div', 'annotation-quote', item.quote);
        const input = node('textarea', 'annotation-input'); input.placeholder = 'Leave a comment'; input.setAttribute('aria-label', 'Comment text'); input.value = item.text || '';
        const actions = node('div', 'annotation-actions');
        if (!fresh) { const remove = node('button', '', 'Delete'); remove.type = 'button'; remove.onclick = () => { comments.splice(comments.indexOf(item), 1); item.onDelete?.(); close(); notify(); }; actions.append(remove); }
        const cancel = node('button', '', 'Cancel'); cancel.type = 'button'; cancel.onclick = close;
        const save = node('button', '', 'Save'); save.type = 'submit'; save.disabled = !input.value.trim(); input.oninput = () => { save.disabled = !input.value.trim(); };
        actions.append(cancel, save); popup.append(quote, input, actions); document.body.append(popup); position(popup, rect);
        popup.onsubmit = event => { event.preventDefault(); if (!input.value.trim()) return; item.text = input.value.trim(); if (fresh) { item.id = ++next; item.contextId=identity(); comments.push(item); item.onSave?.(item); } close(); notify(); };
        input.focus();
    }
    function list(rect, items = comments, selection = null) {
        close(); popup = node('section', 'annotation-popup annotation-list'); popup.setAttribute('aria-label', 'Comments');
        const closeButton = node('button', 'annotation-close', '×'); closeButton.setAttribute('aria-label', 'Close comments'); closeButton.onclick = close; popup.append(closeButton);
        items.forEach((item, index) => {
            const row = node('div', 'annotation-entry');
            const label = item.source === 'Chat' ? item.quote : `L${item.lineFrom}-L${item.lineTo}`;
            const quote = node('div', 'annotation-quote annotation-summary', `${index + 1}. ${label}`);
            row.append(quote, node('div', 'annotation-comment', `“${item.text}”`));
            const button = node('button', 'annotation-edit-icon'); button.setAttribute('aria-label', 'Edit comment'); button.innerHTML = '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="m5 15 10-10 4 4-10 10-5 1zM13 7l4 4"/></svg>'; button.onclick = () => edit(item, rect); row.append(button); popup.append(row);
            if(selection) {
                const include=node('button','annotation-include-icon');include.type='button';include.setAttribute('aria-label',`Include comment ${item.id} in next message`);include.setAttribute('aria-pressed',String(selection.selected(item)));
                const render=()=>{const selected=selection.selected(item);include.setAttribute('aria-pressed',String(selected));include.setAttribute('aria-label',`${selected?'Remove':'Include'} comment ${item.id} ${selected?'from':'in'} next message`);include.innerHTML=selected?'<svg viewBox="0 0 24 24" aria-hidden="true"><path d="m5 12 4 4 10-10"/></svg>':'<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 5v14M5 12h14"/></svg>';};render();
                include.onclick=()=>{selection.toggle(item);render();};
                const help=()=>selection.hint(selection.selected(item)?'Remove comment from the next message':'Include this comment in the next message');include.onmouseenter=help;include.onfocus=help;include.onmouseleave=()=>selection.hint(null);include.onblur=()=>selection.hint(null);
                row.append(include);
            }
        });
        document.body.append(popup);
        const width = Math.min(rect.width, window.innerWidth - 32); popup.style.width = `${width}px`;
        popup.style.left = `${Math.max(16, Math.min(rect.left, window.innerWidth - width - 16))}px`;
        popup.style.top = `${Math.max(16, rect.top - popup.offsetHeight - 10)}px`;
    }
    document.addEventListener('pointerdown', event => { if (popup && !popup.contains(event.target) && !event.target.closest('.annotation-trigger, .comment-marker, .comment-badge')) close(); });
    document.addEventListener('keydown', event => { if (event.key === 'Escape') close(); });
    window.addEventListener('resize', close);
    window.dadoComments = { comments, glyph, begin: (item, rect) => edit(item, rect, true), edit, list, close,
        replace(items) { close(); items.forEach(item=>{item.contextId ||= identity();}); comments.splice(0, comments.length, ...items); next = Math.max(0, ...items.map(item => item.id || 0)); notify(); },
        subscribe(listener) { listeners.add(listener); listener(comments); return () => listeners.delete(listener); },
        changed: notify };
}
