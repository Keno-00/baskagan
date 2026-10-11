window.startChat = function (send, avatar) {
    window.dadoChat?.dispose();
    const host = document.getElementById('chat-host');
    const split = host.parentElement;
    host.innerHTML = `<div class="chat-divider"><button class="chat-dots" aria-label="Change chat layout"><svg viewBox="0 0 24 24"><circle cx="5" cy="12" r="1.5"/><circle cx="12" cy="12" r="1.5"/><circle cx="19" cy="12" r="1.5"/></svg></button><div class="chat-layouts" hidden></div></div><div class="chat-history" role="log" aria-label="Messages" aria-live="polite"><div class="chat-messages"></div></div><form class="chat-composer"><div class="chat-attachments" hidden></div><textarea aria-label="Message" placeholder="Message" rows="1"></textarea><div class="chat-actions"><button type="button" class="chat-attach" aria-label="Chat actions" aria-haspopup="menu" aria-expanded="false"><svg viewBox="0 0 24 24"><path d="M12 5v14M5 12h14"/></svg></button><button type="submit" class="chat-send" aria-label="Send message" disabled><svg viewBox="0 0 24 24"><path d="m6 12 6-6 6 6M12 6v13"/></svg></button></div></form>`;
    const history = host.querySelector('.chat-history');
    const messages = host.querySelector('.chat-messages');
    const form = host.querySelector('form');
    const input = host.querySelector('textarea');
    const chips = host.querySelector('.chat-attachments');
    const submit = host.querySelector('.chat-send');
    const dots = host.querySelector('.chat-dots');
    const layouts = host.querySelector('.chat-layouts');
    const divider = host.querySelector('.chat-divider');
    const annotations = window.dadoComments;
    let projectRoot = null, projectState = null, loaded = false, loading = false, generation = 0, saveTimer = null, sequence = 0;
    const pending = new Map();
    let aiState = {ready:false, account:null, models:[], error:null, signingIn:false}, usage = null;
    const jobs = new Map();
    let projectGeneration = 0, runtimeSession = null;
    const sessionWaiters = new Set();
    let reviewDialog = null;
    let selectedComments=[];
    const sessionReady = () => runtimeSession?.root === projectRoot && runtimeSession?.generation === projectGeneration && !!runtimeSession?.projectId;
    function waitForSession(root, generation) {
        if (runtimeSession?.root===root && runtimeSession?.generation===generation && runtimeSession?.projectId) return Promise.resolve();
        return new Promise((resolve,reject)=>sessionWaiters.add({root,generation,resolve,reject}));
    }
    const newId = () => crypto.randomUUID?.() || `${Date.now()}-${Array.from(crypto.getRandomValues(new Uint32Array(3))).join('-')}`;
    const rpc = (action, data) => new Promise((resolve, reject) => { const id = ++sequence; const timer=setTimeout(()=>{pending.delete(id);reject(new Error('Request timed out.'));},60000);pending.set(id, {resolve,reject,timer}); send({action,id,...data}); });
    const conversation = () => projectState?.conversations.find(item => item.id === projectState.active);
    function snapshot() {
        const chat = conversation(); if (!chat) return;
        chat.draft = input.value; chat.attachments = attachments; chat.skills=selectedSkills; chat.commentSelection=[...selectedComments];
        projectState.comments = annotations.comments.map(({onSave,onDelete,...item}) => item);
    }
    function flush() {
        clearTimeout(saveTimer); saveTimer = null;
        if (!loaded || !projectRoot) return;
        snapshot();
        const sidebar = window.dadoSidebar?.snapshot();
        if (sidebar) {
            projectState.sidebar = sidebar;
            projectState.active = sidebar.active;
            projectState.conversations = sidebar.chats.map(chat => ({...chat.content,id:chat.id,title:chat.title}));
        }
        return rpc('save', {root:projectRoot,state:projectState}).catch(error => hint(`Could not save project: ${error.message}`));
    }
    function persist() { if (!loaded || loading) return; clearTimeout(saveTimer); saveTimer = setTimeout(flush, 200); }
    function renderSidebar() { window.dadoSidebar?.changed(); }
    function startSidebar() {
        const saved = projectState.sidebar || {version:1, active:projectState.active, concepts:[], chats:projectState.conversations.map(chat => ({id:chat.id,title:chat.title,visited:Date.now(),concept:null,content:chat}))};
        window.startSidebar(message => {
            if (message.action === 'save') { projectState.sidebar = message.state; persist(); }
            else if (message.action === 'hint') hint(message.text);
            else send({...message,root:projectRoot});
        }, saved, avatar, null);
    }
    function rangeAt(text, from, to) {
        const walker = document.createTreeWalker(text, NodeFilter.SHOW_TEXT, {acceptNode: node => node.parentElement.closest('.comment-marker') ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT});
        const range = document.createRange(); let offset = 0, started = false, node;
        while ((node = walker.nextNode())) { const end = offset + node.length; if (!started && from <= end) { range.setStart(node, Math.max(0, from-offset)); started = true; } if (started && to <= end) { range.setEnd(node, Math.max(0,to-offset)); return range; } offset = end; }
        return null;
    }
    const markerSubscriptions = [];
    function bindComment(item) {
        const text = messages.querySelector(`[data-message="${CSS.escape(item.message || '')}"] .chat-message-text`);
        if (!text) return;
        const range = rangeAt(text,item.from,item.to); if (!range) return;
        const mark = document.createElement('span'); mark.className = 'comment-highlight'; mark.append(range.extractContents()); range.insertNode(mark);
        const marker = document.createElement('button'); marker.type = 'button'; marker.className = 'comment-marker'; marker.textContent = item.id; marker.setAttribute('aria-label', `Comment ${item.id}`); mark.after(marker);
        marker.onclick = () => annotations.edit(item,marker.getBoundingClientRect());
        const preview = document.createElement('span'); preview.className = 'comment-preview'; marker.append(preview);
        const stop = annotations.subscribe(() => { preview.textContent = item.text; }); markerSubscriptions.push(stop);
        item.onDelete = () => { stop(); marker.remove(); mark.replaceWith(...mark.childNodes); };
    }
    function renderMessage(record) {
        const article = document.createElement('article'); article.className = record.role === 'assistant' ? 'chat-assistant-message' : 'chat-user-message'; article.classList.toggle('streaming',record.status==='streaming'); article.dataset.message = record.id; article.setAttribute('aria-label', record.role === 'assistant' ? 'Assistant message' : 'Your message');
        const bubble = document.createElement('div'); bubble.className = record.role === 'assistant' ? 'chat-assistant-bubble' : 'chat-user-bubble';
        if (record.text || record.role === 'assistant') { const text = document.createElement('div'); text.className = 'chat-message-text'; text.textContent = record.text; bubble.append(text); }
        if(record.skills?.length){const refs=document.createElement('div');refs.className='chat-message-files';for(const skill of record.skills){const chip=fileChip({name:skill.name,path:''});help(chip,`Skill · ${skill.scope} · ${skill.revision}`);refs.append(chip);}bubble.append(refs);}
        if (record.attachments?.length) { const files = document.createElement('div'); files.className = 'chat-message-files'; record.attachments.forEach(file => files.append(fileChip(file))); bubble.append(files); }
        if(record.comments?.length) {const comments=document.createElement('div');comments.className='chat-message-files';record.comments.forEach(item=>comments.append(commentChip(item)));bubble.append(comments);}
        const time = document.createElement('time'); const date = new Date(record.time); time.dateTime = date.toISOString(); time.textContent = date.toLocaleTimeString([], {hour:'numeric',minute:'2-digit'}); article.append(bubble,time); if (record.role === 'assistant') { const state = document.createElement('span'); state.className = 'chat-response-state'; state.textContent = responseStatus(record); article.append(state); } messages.append(article);
        renderToolActivity(record);
        renderReviewButton(record);
    }
    function renderToolActivity(record) {
        const article=messages.querySelector(`[data-message="${CSS.escape(record.id)}"]`);if(!article || !record.tools?.length)return;
        let list=article.querySelector('.chat-tool-activity');if(!list){list=document.createElement('div');list.className='chat-tool-activity';article.append(list);}
        for(const tool of record.tools) {
            let row=[...list.children].find(item=>item.dataset.call===tool.id);
            if(!row){row=document.createElement('details');row.dataset.call=tool.id;row.append(document.createElement('summary'),document.createElement('pre'));list.append(row);}
            row.querySelector('summary').textContent=`${tool.tool.replace(/^sde_/,'').replaceAll('_',' ')} · ${tool.status}`;
            row.querySelector('pre').textContent=tool.output || '';
        }
    }
    function closeReview() {reviewDialog?.close();reviewDialog?.remove();reviewDialog=null;}
    function renderReviewButton(record) {
        const article=messages.querySelector(`[data-message="${CSS.escape(record.id)}"]`);if(!article)return;
        let button=article.querySelector('.chat-review-changes');
        if(!record.changes?.count) {button?.remove();return;}
        if(!button) {button=document.createElement('button');button.type='button';button.className='chat-review-changes';button.onclick=()=>openReview(record);button.onmouseenter=()=>hint(`Review ${record.changes.count} proposed file${record.changes.count===1?'':'s'}`);button.onmouseleave=()=>hint(null);article.append(button);}
        button.innerHTML='<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M7 3h7l4 4v6M7 3v18h5M14 3v5h4m-3 9 4-4 3 3-4 4-4 1z"/></svg>';
        const count=document.createElement('span');count.textContent=record.changes.count;button.append(count);
        button.disabled=record.status==='streaming';button.setAttribute('aria-label',`Review proposed changes (${record.changes.count})`);
    }
    async function openReview(record) {
        closeReview();const root=projectRoot,session=projectGeneration,projectId=runtimeSession?.projectId;
        const request={root,projectId,generation:session,requestId:record.changes.requestId};
        try {
            const review=await rpc('ai-review',request);
            if(disposed || root!==projectRoot || session!==projectGeneration)return;
            const dialog=document.createElement('dialog');dialog.className='chat-change-review';dialog.setAttribute('aria-label','Review proposed changes');reviewDialog=dialog;
            const header=document.createElement('header'),title=document.createElement('strong'),close=document.createElement('button');title.textContent='Changes';close.type='button';close.setAttribute('aria-label','Close change review');close.textContent='×';close.onclick=closeReview;header.append(title,close);
            const content=document.createElement('div');content.className='change-review-files';
            for(const file of review.changes) {
                const section=document.createElement('section'),name=document.createElement('div');name.className='change-review-name';name.textContent=file.path;
                if(file.operation==='create' || file.operation==='delete') {
                    const kind=document.createElement('span');kind.textContent=file.operation==='create'?'+':'−';kind.setAttribute('aria-label',file.operation==='create'?'New file':'Deleted file');name.prepend(kind);
                    name.onmouseenter=()=>hint(`${file.operation==='create'?'New file':'Proposed deletion'} · ${window.dadoReview.applicationIssue(review) || file.path}`);name.onmouseleave=()=>hint(null);
                }
                if(file.conflict) {const conflict=document.createElement('span');conflict.textContent='Changed on disk';conflict.className='change-review-conflict';name.append(conflict);}
                const code=document.createElement('pre');code.className='change-review-diff';
                for(const line of window.dadoReview.diff(file.before,file.after,file.operation)) {
                    const row=document.createElement('div');row.className=`diff-${line.kind}`;
                    const old=document.createElement('span'),next=document.createElement('span'),text=document.createElement('span');old.textContent=line.oldLine || '';next.textContent=line.newLine || '';text.textContent=`${line.kind==='add'?'+':line.kind==='remove'?'-':' '} ${line.text}`;row.append(old,next,text);code.append(row);
                }
                section.append(name,code);content.append(section);
            }
            const footer=document.createElement('footer'),apply=document.createElement('button');apply.type='button';apply.textContent=review.state==='applied_to_editor'?'Applied to editor':'Apply to editor';apply.disabled=review.state!=='pending' || !review.changes.length || review.changes.some(file=>file.conflict);
            const applicationIssue=window.dadoReview.applicationIssue(review);if(applicationIssue){apply.hidden=true;title.onmouseenter=()=>hint(applicationIssue);title.onmouseleave=()=>hint(null);hint(applicationIssue);}
            apply.onmouseenter=()=>hint('Apply to editor buffers · Save remains explicit');apply.onmouseleave=()=>hint(null);
            apply.onclick=async()=>{
                apply.disabled=true;
                try {
                    if(root!==projectRoot || session!==projectGeneration)throw new Error('Project session changed.');
                    await window.dadoWorkbench.applyProposal(review,root);
                    record.changes.status='applied_to_editor';persist();send({action:'panel',shown:true});closeReview();
                    await rpc('ai-review-applied',request);
                    hint('Applied to editors · Save to write the files');
                } catch(error) {hint(error.message);if(dialog.isConnected)apply.disabled=false;}
            };
            footer.append(apply);dialog.append(header,content,footer);host.append(dialog);dialog.addEventListener('cancel',event=>{event.preventDefault();closeReview();});dialog.showModal();
        } catch(error) {hint(`Could not review changes: ${error.message}`);}
    }
    function renderConversation() {
        closeReview();
        closeActions(); picking=false; pickTicket=null; attachButton.disabled=false;
        markerSubscriptions.splice(0).forEach(stop => stop()); messages.replaceChildren();
        const chat = conversation(); input.value = chat?.draft || ''; attachments = chat?.attachments || [];selectedSkills=chat?.skills || [];
        (chat?.messages || []).forEach(renderMessage);
        annotations.comments.filter(item => item.source === 'Chat' && item.conversation === chat?.id).sort((a,b)=>b.from-a.from).forEach(bindComment);
        pinned = true; renderAttachments();
        for(const job of jobs.values())if(job.root===projectRoot && job.chat.id===chat?.id)renderApprovals(job);
    }
    const badge = document.createElement('button'); badge.type = 'button'; badge.className = 'comment-badge';
    const pendingComments=()=>window.dadoCommentContext.pending(annotations.comments,(projectState?.conversations || []).flatMap(chat=>chat.messages || []));
    badge.onclick = () => annotations.list(form.getBoundingClientRect(),pendingComments());
    const contextRow = document.createElement('div'); contextRow.className = 'chat-context-row'; contextRow.append(badge,chips); form.prepend(contextRow);
    const unsubscribe = annotations.subscribe(() => {if(loaded)renderAttachments();persist(); });
    let selectionButton = null;
    const selectedText = () => {
        selectionButton?.remove(); selectionButton = null;
        const selection = window.getSelection();
        if (!selection.rangeCount || selection.isCollapsed) return;
        const range = selection.getRangeAt(0).cloneRange();
        const container = range.commonAncestorContainer.nodeType === 1 ? range.commonAncestorContainer : range.commonAncestorContainer.parentElement;
        const text = container.closest('.chat-message-text');
        if (!text || !host.contains(text) || text.closest('article')?.classList.contains('streaming') || !range.toString().trim()) return;
        const rect = range.getBoundingClientRect();
        selectionButton = document.createElement('button'); selectionButton.type = 'button'; selectionButton.className = 'annotation-trigger chat-selection-comment'; selectionButton.setAttribute('aria-label', 'Comment on selected text'); selectionButton.innerHTML = annotations.glyph;
        selectionButton.style.left = `${Math.max(8, Math.min(rect.right - 32, window.innerWidth - 40))}px`; selectionButton.style.top = `${Math.max(8, rect.top - 36)}px`;
        selectionButton.onpointerdown = event => event.preventDefault();
        selectionButton.onclick = () => {
            selectionButton.remove(); selectionButton = null;
            const prefix = document.createRange(); prefix.selectNodeContents(text); prefix.setEnd(range.startContainer,range.startOffset);
            const fragment = prefix.cloneContents(); fragment.querySelectorAll('.comment-marker').forEach(el=>el.remove());
            const quote = range.toString(); const from = fragment.textContent.length;
            annotations.begin({quote, source: 'Chat', conversation:conversation()?.id, message:text.closest('article').dataset.message, from, to:from+quote.length, onSave(item) {
                bindComment(item);
                window.getSelection().removeAllRanges();
            }}, rect);
        };
        document.body.append(selectionButton);
    };
    host.addEventListener('mouseup', selectedText);
    host.addEventListener('keyup', selectedText);
    const clearSelectionButton = () => { selectionButton?.remove(); selectionButton = null; };
    history.addEventListener('scroll', clearSelectionButton);
    let attachments = [];
    let selectedSkills=[];
    let picking = false;
    let disposed = false;
    let pinned = true;
    history.addEventListener('scroll', () => { pinned = history.scrollHeight - history.scrollTop - history.clientHeight < 40; });
    const hint = value => send({action: 'hint', hint: value});
    function help(element, text) {
        element.addEventListener('mouseenter', () => hint(text));
        element.addEventListener('mouseleave', () => hint(null));
        element.addEventListener('focus', () => hint(text));
        element.addEventListener('blur', () => hint(null));
    }
    function resize() {
        input.style.height = '0px';
        // The composer grows inside its row; the systems/chat boundary never moves.
        const available = Math.max(20, Math.min(host.clientHeight / 2, host.clientHeight - contextRow.offsetHeight - host.querySelector('.chat-actions').offsetHeight - 20));
        input.style.height = `${Math.min(input.scrollHeight, available)}px`;
        updateSend();
        if (pinned) history.scrollTop = history.scrollHeight;
    }
    function dismiss() { layouts.hidden = true; dots.hidden = false; divider.classList.remove('choosing'); }
    const choices = [ ['Systems larger', 3, 1], ['Balanced layout', 1, 1], ['Chat larger', 1, 3] ];
    choices.forEach(([label, top, bottom], index) => {
        const button = document.createElement('button');
        button.type = 'button'; button.setAttribute('aria-label', label); button.setAttribute('aria-pressed', String(index === 0));
        const height = 20 * top / (top + bottom);
        button.innerHTML = `<svg viewBox="0 0 28 28"><rect x="3" y="3" width="22" height="${height}" rx="2" fill="#aaa"/><rect x="3" y="${5+height}" width="22" height="${20-height}" rx="2" fill="#ddd"/></svg>`;
        button.onclick = () => {
            split.style.gridTemplateRows = `minmax(0, ${top}fr) minmax(0, ${bottom}fr)`;
            layouts.querySelectorAll('button').forEach(other => other.setAttribute('aria-pressed', String(other === button)));
            dismiss(); dots.focus(); resize();
            if (projectState) { projectState.settings.layout = index; persist(); }
        };
        help(button, label); layouts.append(button);
    });
    dots.onclick = () => { dots.hidden = true; layouts.hidden = false; divider.classList.add('choosing'); layouts.querySelector('button').focus(); };
    help(dots, 'Change the space shared by systems and chat');
    const outside = event => { if (!divider.contains(event.target)) dismiss(); if (!actionMenu.contains(event.target) && !attachButton.contains(event.target) && !event.target.closest('.chat-skill-menu')) closeActions(); };
    const escape = event => { if (event.key === 'Escape' && !layouts.hidden) { dismiss(); dots.focus(); } if (event.key === 'Escape' && !actionMenu.hidden) { event.preventDefault(); closeActions(true); } };
    document.addEventListener('pointerdown', outside);
    document.addEventListener('keydown', escape);
    function fileChip(file, remove) {
        const chip = document.createElement('span'); chip.className = 'chat-file';
        chip.title = file.path;
        const label = document.createElement('span'); label.textContent = file.name; chip.append(label);
        if (remove) {
            const button = document.createElement('button'); button.type = 'button'; button.textContent = '×'; button.setAttribute('aria-label', `Remove ${file.name}`);
            button.onclick = remove; chip.append(button);
        }
        return chip;
    }
    function renderAttachments() {
        const comments=pendingComments();selectedComments=comments.map(item=>item.contextId);
        badge.hidden=!comments.length;badge.innerHTML=annotations.glyph;
        const count=document.createElement('span');count.textContent=comments.length;badge.append(count);
        badge.setAttribute('aria-label',`View comments (${comments.length})`);
        chips.replaceChildren(); chips.hidden = !attachments.length && !selectedComments.length && !selectedSkills.length;
        attachments.forEach(file => chips.append(fileChip(file, () => { attachments = attachments.filter(item => item !== file); renderAttachments(); })));
        for(const skill of selectedSkills){const chip=fileChip({name:skill.name,path:''},()=>{selectedSkills=selectedSkills.filter(s=>s.id!==skill.id);renderAttachments();});chip.classList.add('chat-skill-chip');help(chip,`Skill · ${skill.description} · ${skill.scope}`);chips.append(chip);}
        for(const item of comments) chips.append(commentChip(item,()=>{item.contextDismissedSnapshot=window.dadoCommentContext.signature(item);renderAttachments();}));
        resize();
        window.dadoSidebar?.draftChanged(); persist();
    }
    function commentChip(item,remove) {
        const chat=item.source==='chat' || item.source==='Chat';
        const name=window.dadoCommentContext.label({...item,source:chat?'Chat':item.source});
        const chip=fileChip({name,path:''},remove);chip.removeAttribute('title');chip.classList.add('chat-comment-context');
        const icon=document.createElement('span');icon.innerHTML=annotations.glyph;chip.prepend(icon);
        help(chip,`Comment ${item.displayId || item.id} · ${item.text}`);return chip;
    }
    const attachButton = host.querySelector('.chat-attach');
    const commands = window.dadoCommands.panel({form,input,trigger:attachButton,rpc,root:()=>projectRoot,ready:()=>loaded,hint,
        changed() {resize();persist();},
        dispatch(id) {
            if(id==='skills') skillsPicker.open();
            else if(id==='create-skill') skillCreator.open();
            else if(id==='attach-file') pickAttachments('file');
            else if(id==='attach-folder') pickAttachments('folder');
            else if(id==='models') document.querySelector('#model-selector-mount button')?.click();
            else if(id==='model-settings') window.dadoSidebar?.openModelSettings();
            else if(id==='permissions') document.querySelector('#permission-selector-mount button')?.click();
            else if(id==='new-chat') window.dadoSidebar?.newChat();
        }
    });
    const skillsPicker=window.createSkillPicker({form,input,hint,closeOthers:()=>closeActions(),load:()=>rpc('ai-skills',{root:projectRoot,projectId:runtimeSession?.projectId,generation:projectGeneration}),choose(skill){if(selectedSkills.some(s=>s.id===skill.id))return;if(selectedSkills.length>=8){hint('Choose at most eight skills per message');return;}selectedSkills.push(skill);renderAttachments();}});
    const skillCreator=window.createSkillCreator({rpc,hint,session:()=>({root:projectRoot,projectId:runtimeSession?.projectId,generation:projectGeneration}),buffers:()=>window.dadoWorkbench?.agentContext().buffers || [],openEditor(path){send({action:'panel',shown:true});window.dadoWorkbench?.openFile(path);},startDraft(spec){
        if(!sessionReady() || currentJob()){hint('Wait for this conversation to finish before drafting a skill');return false;}
        if(!aiState.account || !aiState.models.length){window.dadoSidebar?.openModelSettings();hint('Sign in and choose a model before drafting');return false;}
        const record={id:newId(),role:'user',text:`Draft an instruction-only skill named ${spec.name}.\nWhat it should do: ${spec.task}\nWhen to use it: ${spec.when}\nReturn only the complete SKILL.md text, starting with YAML front matter containing name and description. The name must be exactly ${spec.name}. Write useful concrete instructions. Do not include scripts, assets, references to local supporting files, or permission grants. Do not execute tools or publish anything. I will review and publish the draft myself.`,skillCreation:spec,attachments:[],skills:[],comments:[],time:new Date().toISOString()};
        conversation().messages.push(record);renderMessage(record);flush();startReply(record);return true;
    }});
    const actionMenu=commands.menu;
    function closeActions(focus=false) {commands.close(focus);skillsPicker.close();}
    let pickTicket = null;
    function pickAttachments(kind) { if (picking || !loaded) return; closeActions(); picking = true; attachButton.disabled = true; pickTicket = newId(); send({action:'attach',kind,root:projectRoot,ticket:pickTicket}); }
    attachButton.onclick = () => {if (!actionMenu.hidden) closeActions(true); else commands.open('plus');};
    const menuResize = () => closeActions(); window.addEventListener('resize',menuResize);
    help(attachButton, 'Actions · type / at the start of a line to search');
    window.mountModelSelector = () => { const selector = document.getElementById('model-selector-mount'); if (selector) submit.before(selector); syncModel(); };
    window.mountModelSelector();
    window.mountPermissionSelector = () => {const selector=document.getElementById('permission-selector-mount');if(selector){attachButton.after(selector);syncPermissions();}};
    window.mountPermissionSelector();
    function syncPermissions() {window.dadoPermissionSelector?.restore({mode:projectState?.settings?.agentMode || (aiState.inspectionAvailable?'inspect':'chat'),available:!!aiState.inspectionAvailable});}
    help(submit, 'Send message · Enter to send, Shift+Enter for a new line');
    input.oninput = event => {resize();window.dadoSidebar?.draftChanged();persist();commands.update(event);};
    input.onkeydown = event => {
        if(commands.key(event)) return;
        if (event.key === 'Enter' && !event.shiftKey && !event.isComposing) { event.preventDefault(); if (!submit.disabled && !currentJob()) form.requestSubmit(); }
    };
    form.onsubmit = event => {
        event.preventDefault();
        if (currentJob()) {stopReply(); return;}
        if (!loaded || !sessionReady() || (!input.value.trim() && !attachments.length && !selectedComments.length && !selectedSkills.length)) return;
        if (!aiState.account || !aiState.models.length) {window.dadoSidebar?.openModelSettings(); hint(aiState.error || 'Sign in to Codex to send a message'); return;}
        let comments;
        try {comments=window.dadoCommentContext.capture(annotations.comments,selectedComments);}
        catch(error) {hint(error.message);return;}
        const record = {id:newId(),role:'user',text:input.value.trim(),attachments:[...attachments],skills:selectedSkills.map(s=>({...s})),comments,time:new Date().toISOString()};
        const chat = conversation(); if (chat) { chat.messages.push(record); if (chat.messages.length === 1) chat.title = record.text.slice(0,60) || record.attachments[0]?.name || record.skills[0]?.name || 'New chat'; }
        window.dadoCommentContext.consume(annotations.comments,comments);
        renderMessage(record);
        input.value = ''; attachments = []; selectedSkills=[];selectedComments=[];pinned = true; renderAttachments(); history.scrollTop = history.scrollHeight; input.focus();
        renderSidebar(); flush(); startReply(record);
    };
    function responseStatus(record) {
        if (record.error) return record.error;
        if (record.status==='streaming' && record.phase==='queued') return 'Queued';
        if (record.status==='streaming' && record.phase==='starting') return 'Starting…';
        if (record.status==='streaming' && record.phase==='stopping') return 'Stopping…';
        if (record.status==='streaming' && record.phase==='waiting_for_approval') return 'Waiting for approval';
        return record.status==='streaming' ? 'Responding…' : record.status==='interrupted' ? 'Stopped' : record.status==='failed' ? 'Reply failed' : '';
    }
    function currentJob() {return [...jobs.values()].find(j=>j.root===projectRoot && j.generation===projectGeneration && j.chat.id===projectState?.active);}
    function updateSend() {
        const running=currentJob();
        submit.disabled=running ? false : !loaded || !sessionReady() || (!input.value.trim() && !attachments.length && !selectedComments.length && !selectedSkills.length);
        submit.setAttribute('aria-label',running ? 'Stop response' : 'Send message');
        submit.innerHTML=running ? '<svg viewBox="0 0 24 24"><rect x="6" y="6" width="12" height="12" rx="2"/></svg>' : '<svg viewBox="0 0 24 24"><path d="m6 12 6-6 6 6M12 6v13"/></svg>';
    }
    function syncModel() {
        syncPermissions();
        const settings=projectState?.settings || {};
        let model=aiState.models.find(m=>m.model===settings.model);
        if (!model && aiState.models.length) {model=aiState.models.find(m=>m.isDefault) || aiState.models[0]; settings.model=model.model;}
        const efforts=model?.supportedReasoningEfforts?.map(e=>e.reasoningEffort) || [];
        if (model && !efforts.includes(settings.reasoningEffort)) {
            const legacy=['low','medium','high','xhigh','max'][settings.reasoning];
            settings.reasoningEffort=efforts.includes(legacy) ? legacy : (efforts.includes(model.defaultReasoningEffort) ? model.defaultReasoningEffort : efforts[0]) || null;
        }
        window.dadoModelSelector?.catalog(aiState.models);
        window.dadoModelSelector?.restore({model:model?.model || null, reasoningEffort:settings.reasoningEffort});
    }
    function saveJob(job) {
        const references=[job.state.conversations.find(c=>c.id===job.chat.id),job.state.sidebar?.chats.find(c=>c.id===job.chat.id)?.content];
        for (const ref of references) if (ref) {ref.messages=job.chat.messages;ref.codexThreadId=job.chat.codexThreadId;ref.codexAccount=job.chat.codexAccount;ref.codexMode=job.chat.codexMode;ref.codexProjectId=job.chat.codexProjectId;ref.codexToolsVersion=job.chat.codexToolsVersion;}
        if (job.root===projectRoot && job.generation===projectGeneration) persist();
        else rpc('save',{root:job.root,state:job.state}).catch(e=>hint(`Could not save response: ${e.message}`));
    }
    function startReply(record) {
        const chat=conversation(); if (!chat || !sessionReady()) return;
        const requestId=newId(), response={id:newId(),role:'assistant',text:'',time:new Date().toISOString(),status:'streaming'};
        const history=chat.messages.filter(m=>m!==record).map(({role,text,comments})=>({role:role || 'user',text,comments}));
        chat.messages.push(response);
        const job={requestId,root:projectRoot,projectId:runtimeSession.projectId,generation:projectGeneration,state:projectState,chat,records:new Map(),placeholder:response,skillCreation:record.skillCreation}; jobs.set(requestId,job);
        renderMessage(response); updateSend();
        const account=JSON.stringify(aiState.account),mode=record.skillCreation?'chat':projectState.settings.agentMode || (aiState.inspectionAvailable ? 'inspect' : 'chat');
        job.mode=mode;
        job.toolsVersion=record.skillCreation?'skill-draft-v1':mode==='inspect'?'inspect-skills-v3':mode==='approve'?'approve-skills-v2':mode==='full'?'full-native-v2':'chat-v1';
        const editorContext=window.dadoWorkbench?.agentContext() || {};
        send({action:'ai-send',requestId,root:projectRoot,projectId:job.projectId,generation:job.generation,chat:chat.id,threadId:!record.skillCreation && chat.codexAccount===account && chat.codexMode===mode && chat.codexProjectId===job.projectId && chat.codexToolsVersion===job.toolsVersion ? chat.codexThreadId : null,mode,selectedFile:editorContext.selectedFile,buffers:editorContext.buffers || [],model:projectState.settings.model,effort:projectState.settings.reasoningEffort || null,text:record.text,attachments:record.attachments,skills:record.skills || [],comments:record.comments,history,context:window.dadoSidebar?.context() || {}});
        flush();
    }
    function stopReply(all=false) {for (const job of jobs.values()) if (all || (job.root===projectRoot && job.chat.id===projectState?.active)) send({action:'ai-stop',requestId:job.requestId,root:job.root,projectId:job.projectId,generation:job.generation});}
    function renderApprovals(job) {
        const article=messages.querySelector(`[data-message="${CSS.escape(job.placeholder.id)}"]`);if(!article)return;
        const status=article.querySelector('.chat-response-state');if(status)status.textContent=responseStatus(job.placeholder);
        article.querySelector('.chat-approvals')?.remove();
        if(!job.approvals?.size)return;
        const list=document.createElement('div');list.className='chat-approvals';
        for(const approval of job.approvals.values()) {
            const card=document.createElement('section');card.className='chat-command-approval';card.setAttribute('aria-label','Command approval');
            const heading=document.createElement('div');heading.className='command-approval-heading';heading.textContent='Run command?';
            const command=document.createElement('pre');command.textContent=approval.command;
            const reason=document.createElement('div');reason.className='command-approval-reason';reason.textContent=approval.reason;
            const scope=document.createElement('div');scope.className='command-approval-scope';scope.textContent=`Working copy · ${approval.timeoutSeconds}s · Runs with your user permissions`;help(scope,approval.cwd);
            const actions=document.createElement('div');actions.className='command-approval-actions';
            for(const [decision,label] of [['decline','Decline'],['accept','Run once']]) {
                const button=document.createElement('button');button.type='button';button.textContent=label;button.disabled=!!approval.submitted;
                button.onclick=()=>{if(approval.submitted)return;approval.submitted=true;renderApprovals(job);send({action:'ai-approval',decision,approvalId:approval.approvalId,requestId:job.requestId,root:job.root,projectId:job.projectId,generation:job.generation,chat:job.chat.id});};
                actions.append(button);
            }
            card.append(heading,command,reason,scope,actions);list.append(card);
        }
        article.append(list);
    }
    function aiEvent(event) {
        if(event.kind==='skills_changed'){if(event.root===projectRoot && event.generation===projectGeneration)skillsPicker.close();return;}
        if (disposed) return;
        if (event.kind==='project') {
            if (!runtimeSession || event.generation>=runtimeSession.generation) runtimeSession=event;
            for(const waiter of sessionWaiters) {
                if (event.generation===waiter.generation && event.root===waiter.root && event.projectId) {sessionWaiters.delete(waiter);waiter.resolve();}
                else if(event.generation>waiter.generation) {sessionWaiters.delete(waiter);waiter.reject(new Error('Project session changed.'));}
            }
            updateSend(); return;
        }
        if (event.kind==='state') {
            if (event.generation != null && (event.generation!==projectGeneration || event.root!==projectRoot)) return;
            aiState={...aiState,...event}; if (!aiState.account) usage=null; syncModel();
            if(event.error && !sessionReady()) for(const waiter of sessionWaiters) {sessionWaiters.delete(waiter);waiter.reject(new Error(event.error));}
            if (event.error) hint(event.error);
            window.dadoSidebar?.refreshAI(); document.dispatchEvent(new CustomEvent('dado-ai-state')); return;
        }
        if (event.kind==='usage') {if(event.generation != null && event.generation!==projectGeneration) return; usage=aiState.account ? event.limits : null; window.dadoSidebar?.refreshAI(); return;}
        const job=jobs.get(event.requestId); if (!job || job.root!==event.root || job.chat.id!==event.chat || job.projectId!==event.projectId || job.generation!==event.generation) return;
        const visible=projectRoot===job.root && projectGeneration===job.generation && projectState?.active===job.chat.id;
        if (event.kind==='lifecycle') {
            job.placeholder.phase=event.phase;
            if(visible) {const label=messages.querySelector(`[data-message="${CSS.escape(job.placeholder.id)}"] .chat-response-state`); if(label) label.textContent=responseStatus(job.placeholder);}
        }
        if (event.kind==='thread') {job.chat.codexThreadId=event.threadId;job.chat.codexAccount=JSON.stringify(aiState.account);job.chat.codexMode=job.mode;job.chat.codexProjectId=job.projectId;job.chat.codexToolsVersion=job.toolsVersion;}
        if(event.kind==='approval') {job.placeholder.phase='waiting_for_approval';job.approvals ||= new Map();job.approvals.set(event.approvalId,event);if(visible)renderApprovals(job);}
        if(event.kind==='approval_resolved') {if(job.placeholder.phase!=='stopping')job.placeholder.phase='running';job.approvals?.delete(event.approvalId);if(visible)renderApprovals(job);}
        if (event.kind==='tool') {
            job.placeholder.tools ||= [];
            let activity=job.placeholder.tools.find(item=>item.id===event.itemId);
            if(!activity) {activity={id:event.itemId};job.placeholder.tools.push(activity);}
            Object.assign(activity,{tool:event.tool,status:event.status,output:event.output});
            if(visible) renderToolActivity(job.placeholder);
        }
        if(event.kind==='changes') {
            job.placeholder.changes={requestId:event.requestId,workspaceId:event.workspaceId,count:event.count,status:event.status};
            if(visible)renderReviewButton(job.placeholder);
        }
        if (event.kind==='delta' || event.kind==='message') {
            let record=job.records.get(event.itemId);
            if (!record) {record=job.records.size ? {id:newId(),role:'assistant',text:'',status:'streaming',time:new Date().toISOString()} : job.placeholder;record.itemId=event.itemId;if (job.records.size) {job.chat.messages.push(record);if (visible) renderMessage(record);}job.records.set(event.itemId,record);}
            record.text=event.kind==='message' ? event.text || record.text : record.text + (event.delta || '');
            if (visible) {const text=messages.querySelector(`[data-message="${CSS.escape(record.id)}"] .chat-message-text`);if (text) text.textContent=record.text;}
        }
        if (event.kind==='notice') hint(event.error);
        if (event.kind==='completed') {
            if(job.skillCreation && event.status==='completed' && job.root===projectRoot && job.generation===projectGeneration){
                const owner={root:job.root,projectId:job.projectId,generation:job.generation};
                const text=[...job.records.values()].map(r=>r.text).join('\n') || job.placeholder.text;
                rpc('ai-skill-draft',{...owner,...job.skillCreation,text}).then(draft=>{
                    job.placeholder.skillDraft=draft;saveJob(job);
                    if(job.root===projectRoot && job.generation===projectGeneration){send({action:'panel',shown:true});window.dadoWorkbench?.openFile(draft.path);hint('Skill draft opened in editor · Use Create skill to review and publish');}
                }).catch(error=>hint(`Skill draft was not published: ${error.message}`));
            }
            for(const record of job.chat.messages) {
                for(const tool of record.tools || [])if(['running','waiting_for_approval'].includes(tool.status))tool.status=event.status==='completed'?'outcome unknown':'interrupted';
                if(visible && record.tools?.length)renderToolActivity(record);
            }
            if(job.mode==='full' && job.root===projectRoot && job.generation===projectGeneration)window.dadoWorkbench?.refreshFromDisk(job.root);
            job.approvals?.clear();if(visible)renderApprovals(job);
            for (const record of job.records.size ? job.records.values() : [job.placeholder]) {record.status=event.status; if (event.error) record.error=event.error;}
            if (event.error) hint(event.error);
            jobs.delete(event.requestId); updateSend();
            if(visible)renderReviewButton(job.placeholder);
            if (visible) for (const record of job.chat.messages.filter(m=>m.role==='assistant')) {const label=messages.querySelector(`[data-message="${CSS.escape(record.id)}"] .chat-response-state`);if (label) {label.textContent=responseStatus(record);label.closest('article').classList.remove('streaming');}}
        }
        saveJob(job);
        if (visible && pinned) history.scrollTop=history.scrollHeight;
    }
    const observer = new ResizeObserver(resize); observer.observe(host);
    const settingsChanged = event => { if (loaded && projectState) { Object.assign(projectState.settings,event.detail); syncModel(); persist(); } };
    document.addEventListener('dado-settings-change',settingsChanged);
    const leaving = () => flush(); window.addEventListener('beforeunload',leaving);
    window.dadoChat = {
        saveNow: flush,
        closeActions,
        snapshot() { const chat = conversation(); return {...chat,messages:chat?.messages || [],draft:input.value,skills:[...selectedSkills],attachments:[...attachments],commentSelection:[...selectedComments]}; },
        aiEvent,
        aiState() {return {...aiState,usage};},
        modelPreferences() {return {...projectState?.settings};},
        aiAction(action) { send({action,root:projectRoot,projectId:runtimeSession?.projectId,generation:projectGeneration}); },
        selectModel(model) { if (!projectState || !aiState.models.some(m=>m.model===model)) return; projectState.settings.model=model; delete projectState.settings.reasoningEffort; syncModel(); persist(); },
        setConversation(id, content) {
            if (!projectState) return;
            commands.load();
            let chat = projectState.conversations.find(item => item.id === id);
            if (!chat) { chat = {id,title:'New chat'}; projectState.conversations.push(chat); }
            Object.assign(chat,content); chat.messages ||= []; projectState.active = id; renderConversation();
        },
        reply(message) { const request=pending.get(message.id); if (!request) return; clearTimeout(request.timer);pending.delete(message.id); if (message.error) request.reject(new Error(message.error)); else request.resolve(message.result); },
        async setProject(root, sessionGeneration = 0) {
            if (root === projectRoot && projectGeneration===sessionGeneration && (loaded || loading)) return;
            projectGeneration=sessionGeneration;
            stopReply(true); skillCreator.close(); closeReview(); window.dadoModelSelector?.close(); window.dadoPermissionSelector?.close(); flush(); closeActions(); pickTicket=null; picking=false; attachButton.disabled=false; window.dadoSidebar?.dispose(); loaded=false; loading=true; const version=++generation; projectRoot=root; projectState=null;
            clearSelectionButton(); markerSubscriptions.splice(0).forEach(stop=>stop()); annotations.replace([]); messages.replaceChildren(); input.value=''; attachments=[]; selectedSkills=[];renderAttachments(); input.disabled=!!root;
            const sidebar=document.getElementById('left-sidebar'); sidebar?.replaceChildren();
            document.querySelector('.app-body')?.classList.remove('sidebar-open'); if (sidebar) sidebar.hidden=true;
            document.querySelector('[aria-label="Toggle left sidebar"]')?.setAttribute('aria-pressed','false');
            send({action:'panel',shown:false}); window.dadoModelSelector?.restore({});
            if (!root) { loading=false; split.style.gridTemplateRows=''; return; }
            try {
                // The owner emits old-run terminal events before this acknowledgement.
                // Their saves are dispatched before loading a reopened project's state.
                await waitForSession(root,sessionGeneration);
                if (version!==generation || disposed) return;
                const state=await rpc('load',{root}); if (version!==generation || disposed) return;
                projectState=state; projectState.settings ||= {}; projectState.conversations ||= []; syncModel();
                if (!projectState.conversations.length) projectState.conversations.push({id:newId(),title:'New chat',messages:[],draft:'',attachments:[]});
                if (!conversation()) projectState.active=projectState.conversations[0].id;
                for (const job of jobs.values()) if (job.root===root && job.generation===projectGeneration) {
                    const chat=projectState.conversations.find(c=>c.id===job.chat.id);
                    if (chat) {chat.messages=job.chat.messages;chat.codexThreadId=job.chat.codexThreadId;chat.codexAccount=job.chat.codexAccount;chat.codexMode=job.chat.codexMode;chat.codexProjectId=job.chat.codexProjectId;chat.codexToolsVersion=job.chat.codexToolsVersion;}
                    const ref=projectState.sidebar?.chats.find(c=>c.id===job.chat.id)?.content;
                    if (ref && chat) Object.assign(ref,chat);
                    job.state=projectState;
                }
                for (const chat of projectState.conversations) for (const msg of chat.messages || []) if (msg.status==='streaming' && ![...jobs.values()].some(j=>j.root===root && j.generation===projectGeneration && j.chat.id===chat.id)) msg.status='interrupted';
                for (const chat of projectState.sidebar?.chats || []) for (const msg of chat.content?.messages || []) if (msg.status==='streaming' && ![...jobs.values()].some(j=>j.root===root && j.generation===projectGeneration && j.chat.id===chat.id)) msg.status='interrupted';
                annotations.replace(projectState.comments || []); renderConversation(); startSidebar();
                const index=Number.isInteger(state.settings.layout) && state.settings.layout>=0 && state.settings.layout<3 ? state.settings.layout : 0;
                const [,top,bottom]=choices[index]; split.style.gridTemplateRows=`minmax(0, ${top}fr) minmax(0, ${bottom}fr)`;
                layouts.querySelectorAll('button').forEach((button,i)=>button.setAttribute('aria-pressed',String(i===index)));
                const shown=!!state.settings.leftSidebar; if (sidebar) sidebar.hidden=!shown; document.querySelector('.app-body')?.classList.toggle('sidebar-open',shown); document.querySelector('[aria-label="Toggle left sidebar"]')?.setAttribute('aria-pressed',String(shown));
                send({action:'panel',shown:!!state.settings.rightPanel}); loading=false; loaded=true; resize(); flush();
                commands.load();
            } catch(error) { if (version===generation && !disposed) { loading=false; hint(`Could not open project storage: ${error.message}`); input.disabled=true; submit.disabled=true; } }
            if (version===generation && loaded) input.disabled=false;
        },
        attach(files, ticket) { if (disposed || ticket !== pickTicket) return; pickTicket=null; picking = false; host.querySelector('.chat-attach').disabled = false; files.forEach(file => { if (!attachments.some(item => item.path === file.path)) attachments.push(file); }); renderAttachments(); input.focus(); },
        dispose() { stopReply(true); skillCreator.dispose(); closeReview(); skillsPicker.dispose();commands.dispose(); window.removeEventListener('resize',menuResize); flush(); window.dadoSidebar?.dispose(); disposed = true; for(const waiter of sessionWaiters) waiter.reject(new Error('Chat closed.')); sessionWaiters.clear(); clearTimeout(saveTimer); unsubscribe(); markerSubscriptions.splice(0).forEach(stop=>stop()); clearSelectionButton(); document.removeEventListener('dado-settings-change',settingsChanged); window.removeEventListener('beforeunload',leaving); host.removeEventListener('mouseup', selectedText); host.removeEventListener('keyup', selectedText); observer.disconnect(); document.removeEventListener('pointerdown', outside); document.removeEventListener('keydown', escape); }
    };
    resize();
    send({action:'ready'}); send({action:'ai-refresh'});
};

