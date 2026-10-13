window.startChat = function (send, avatar) {
    window.dadoChat?.dispose();
    const host = document.getElementById('chat-host');
    const split = host.parentElement;
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
    let userPreferences={version:1,name:'',personality:'',model:null,reasoningEffort:null,agentMode:null,smallModel:null,smallReasoningEffort:null,general:{showSystemsView:true,automaticProjectUnderstanding:true,reopenLastProject:true},appearance:{theme:'system',accentColor:'blue',font:'system',textSize:12,reducedMotion:false,backgroundMotion:true,motionIntensity:'balanced',navigationMotion:true,agentMotion:true,nodeAnimations:true,respectReducedMotion:true}};
    let preferencesExist=false,preferencesProblem=null,preferencesLoaded,preferenceWrites=Promise.resolve();
    const systemTheme=matchMedia('(prefers-color-scheme: dark)');
    function applyAppearance(){
        const a=userPreferences.appearance;
        document.documentElement.dataset.theme=a.theme==='system'?(systemTheme.matches?'dark':'light'):a.theme;
        document.documentElement.dataset.reducedMotion=String(a.reducedMotion);
        document.documentElement.dataset.respectReducedMotion=String(a.respectReducedMotion!==false);
        const style=document.documentElement.style;
        const accentHues={blue:218,teal:174,green:145,violet:268,rose:344,amber:38};
        style.setProperty('--ui-theme-hue',`${accentHues[a.accentColor]??accentHues.blue}deg`);
        style.setProperty('--ui-font',a.font==='arial'?'Arial, sans-serif':a.font==='segoe'?'"Segoe UI", sans-serif':'system-ui, "Segoe UI", sans-serif');
        style.setProperty('--ui-text',`${a.textSize}px`);style.setProperty('--ui-text-small',`${Math.max(10,a.textSize-1)}px`);
        document.dispatchEvent(new CustomEvent('dado-appearance-change',{detail:a}));
    }
    systemTheme.addEventListener('change',applyAppearance);
    function updatePreferences(patch){
        if(preferencesProblem){hint(preferencesProblem);return;}
        userPreferences={...userPreferences,...patch,general:{...userPreferences.general,...patch.general},appearance:{...userPreferences.appearance,...patch.appearance}};
        if(projectState)for(const key of ['agentMode'])if(userPreferences[key]!=null)projectState.settings[key]=userPreferences[key];
        applyAppearance();window.dadoSidebar?.publish();
        const state=structuredClone(userPreferences);
        preferenceWrites=preferenceWrites.then(()=>rpc('preferences-save',{state})).then(()=>{preferencesExist=true;}).catch(error=>hint(`Could not save user settings: ${error.message}`));
    }

    let projectGeneration = 0, runtimeSession = null;
    const sessionWaiters = new Set();
    let reviewDialog = null, reviewEpoch = 0;
    const applyingReviews=new Set();
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
        }, saved, avatar, null, projectRoot);
    }
    function rangeAt(text, from, to) {
        const walker = document.createTreeWalker(text, NodeFilter.SHOW_TEXT, {acceptNode: node => node.parentElement.closest('.comment-marker') ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT});
        const range = document.createRange(); let offset = 0, started = false, node;
        while ((node = walker.nextNode())) { const end = offset + node.length; if (!started && from <= end) { range.setStart(node, Math.max(0, from-offset)); started = true; } if (started && to <= end) { range.setEnd(node, Math.max(0,to-offset)); return range; } offset = end; }
        return null;
    }
    let boundAnnotations = null;
    function bindComment(item) {
        const text = messages.querySelector(`[data-message="${CSS.escape(item.message || '')}"] .chat-message-text`);
        if (!text) return;
        const range = rangeAt(text,item.from,item.to); if (!range) return;
        const mark = document.createElement('span'); mark.className = 'comment-highlight'; mark.append(range.extractContents()); range.insertNode(mark);
        const marker = document.createElement('span'); marker.className = 'annotation-marker-anchor'; mark.after(marker);
        annotations.anchor(marker,`marker-${item.contextId}`,()=>[item],'marker');
        item.onDelete = () => { marker.remove(); mark.replaceWith(...mark.childNodes); annotations.publish(); };
    }

    let historyRevision=0, historyUpdating=false;
    function publishHistory() {
        historyUpdating=true;historyRevision++;
        const chat=conversation();
        window.dadoHistory?.restore({chat:chat?.id || '',revision:historyRevision,follow:pinned,messages:(chat?.messages || []).map(record=>({
            ...record,text:record.text || '',changes:record.changes?{...record.changes,applying:applyingReviews.has(record.id)}:null,tools:(record.tools || []).map(tool=>({...tool,output:tool.output || ''})),clock:new Date(record.time).toLocaleTimeString([],{hour:'numeric',minute:'2-digit'}),response:responseStatus(record),
            approvals:[...jobs.values()].filter(job=>job.root===projectRoot && job.generation===projectGeneration && job.chat.id===chat.id && job.placeholder.id===record.id).flatMap(job=>[...(job.approvals?.values() || [])].map(approval=>({...approval,requestId:job.requestId})))
        }))});
    }
    function renderMessage() {publishHistory();}
    function renderToolActivity() {publishHistory();}
    function renderReviewButton() {publishHistory();}
    function closeReview() {
        reviewEpoch++;reviewDialog=null;window.dadoHistory?.review(null);
    }
    function publishReview() {
        if(!reviewDialog)return;
        const {review,busy,record}=reviewDialog,issue=window.dadoReview.applicationIssue(review);
        window.dadoHistory?.review({...review,message:record.id,busy,issue,changes:review.changes.map(file=>({...file,lines:window.dadoReview.diff(file.before,file.after,file.operation)}))});
    }
    async function openReview(record,apply=false) {
        if(apply && (record.mode==='full' || applyingReviews.has(record.id)))return;
        if(apply){applyingReviews.add(record.id);publishHistory();}
        closeReview();const epoch=reviewEpoch,root=projectRoot,session=projectGeneration,projectId=runtimeSession?.projectId;
        const request={root,projectId,generation:session,requestId:record.changes.requestId};
        try {
            const review=await rpc('ai-review',request);
            if(disposed || epoch!==reviewEpoch || root!==projectRoot || session!==projectGeneration)return;
            reviewDialog={review,record,root,session,request,busy:false,shown:!apply};
            if(apply)await applyReview();else publishReview();
        } catch(error) {hint(`Could not review changes: ${error.message}`);}
        finally {if(apply){applyingReviews.delete(record.id);publishHistory();}}
    }
    async function applyReview() {
        const current=reviewDialog;if(!current || current.busy)return;
        const {review,record,root,session,request}=current;
        const issue=window.dadoReview.applicationIssue(review);
        if(issue){hint(issue);current.shown=true;publishReview();return;}
        if(review.state!=='pending' || !review.changes.length)return;
        current.busy=true;publishHistory();if(current.shown)publishReview();
        try {
            if(root!==projectRoot || session!==projectGeneration)throw new Error('Project session changed.');
            await window.dadoWorkbench.applyProposal(review,root,request);
            record.changes.status='applied_to_editor';publishHistory();persist();send({action:'panel',shown:true});
            if(reviewDialog===current)closeReview();
            await rpc('ai-review-applied',request);hint('Applied to editors · Save to write the files');
        } catch(error) {hint(error.message);publishHistory();if(reviewDialog===current){current.busy=false;if(current.shown)publishReview();}}
    }
    function renderConversation() {
        closeReview();
        closeActions(); picking=false; pickTicket=null; attachButton.disabled=false;
        pinned=true;
        const chat = conversation(); input.value = chat?.draft || ''; attachments = chat?.attachments || [];selectedSkills=chat?.skills || [];
        publishHistory();
        boundAnnotations = null;
        pinned = true; renderAttachments();
        for(const job of jobs.values())if(job.root===projectRoot && job.chat.id===chat?.id)renderApprovals(job);
    }
    const badge = host.querySelector('#pending-annotations');
    const contextRow=host.querySelector('.chat-context-row');
    const pendingComments=()=>window.dadoCommentContext.pending(annotations.comments,(projectState?.conversations || []).flatMap(chat=>chat.messages || []));
    annotations.anchor(badge,'pending',()=>pendingComments().filter(item=>item.kind!=='idea'),'summary',()=>{for(const item of pendingComments().filter(item=>item.kind!=='idea'))item.contextDismissedSnapshot=window.dadoCommentContext.signature(item);renderAttachments();});
    annotations.anchor(host.querySelector('#pending-ideas'),'pending-ideas',()=>pendingComments().filter(item=>item.kind==='idea'),'ideas-summary',()=>{for(const item of pendingComments().filter(item=>item.kind==='idea'))item.contextDismissedSnapshot=window.dadoCommentContext.signature(item);renderAttachments();});
    const unsubscribe = annotations.subscribe(() => {if(loaded)renderAttachments();persist(); });

    const selectedText = () => {
        annotations.select(null);
        const selection = window.getSelection();
        if (!selection.rangeCount || selection.isCollapsed) return;
        const range = selection.getRangeAt(0).cloneRange();
        const container = range.commonAncestorContainer.nodeType === 1 ? range.commonAncestorContainer : range.commonAncestorContainer.parentElement;
        const text = container.closest('.chat-message-text');
        if (!text || !host.contains(text) || text.closest('article')?.classList.contains('streaming') || !range.toString().trim()) return;
        const rect = range.getBoundingClientRect();
        annotations.select(rect, () => {
            const prefix = document.createRange(); prefix.selectNodeContents(text); prefix.setEnd(range.startContainer,range.startOffset);
            const fragment = prefix.cloneContents(); fragment.querySelectorAll('.comment-marker').forEach(el=>el.remove());
            const quote = range.toString(); const from = fragment.textContent.length;
            annotations.begin({quote, source: 'Chat', conversation:conversation()?.id, message:text.closest('article').dataset.message, from, to:from+quote.length, onSave(item) {
                window.getSelection().removeAllRanges();
            }}, rect);
        });
    };
    host.addEventListener('mouseup', selectedText);
    host.addEventListener('keyup', selectedText);
    const clearSelectionButton = () => annotations.select(null);
    history.addEventListener('scroll', selectedText);
    let attachments = [];
    let selectedSkills=[];
    let picking = false;
    let disposed = false;
    let pinned = true;
    history.addEventListener('scroll', () => { if(historyUpdating)return; pinned = history.scrollHeight - history.scrollTop - history.clientHeight < 40; });
    const readingHistory=event=>{
        if((event.type==='wheel' && event.deltaY<0) || (event.type==='keydown' && ['ArrowUp','PageUp','Home'].includes(event.key)) || (event.type==='pointerdown' && event.clientX>history.getBoundingClientRect().right-14))pinned=false;
    };
    history.addEventListener('wheel',readingHistory,{passive:true});
    history.addEventListener('keydown',readingHistory);
    history.addEventListener('pointerdown',readingHistory);

    const hint = value => send({action: 'hint', hint: value});
    function help(element, text) {
        element.addEventListener('mouseenter', () => hint(text));
        element.addEventListener('mouseleave', () => hint(null));
        element.addEventListener('focus', () => hint(text));
        element.addEventListener('blur', () => hint(null));
    }
    function resize() {
        input.style.height = '0px';
        // Attachment cards expand the chat row once; long drafts scroll within it.
        const style=getComputedStyle(form),hostStyle=getComputedStyle(host);
        const visible=[...form.children].filter(el=>getComputedStyle(el).display!=='none');
        const padding=Number.parseFloat(style.paddingTop)+Number.parseFloat(style.paddingBottom);
        const fixed=contextRow.offsetHeight+host.querySelector('.composer-attachments').offsetHeight+host.querySelector('.chat-actions').offsetHeight+padding+Number.parseFloat(style.rowGap)*Math.max(0,visible.length-1)+(contextRow.offsetHeight?Number.parseFloat(getComputedStyle(contextRow).marginBottom):0);
        const room=host.clientHeight-Number.parseFloat(hostStyle.paddingTop)-Number.parseFloat(hostStyle.paddingBottom)-Number.parseFloat(hostStyle.rowGap)-fixed;
        const available = Math.max(20, Math.min(host.clientHeight / 2,room));
        input.style.height = `${Math.min(input.scrollHeight, available)}px`;
        updateSend();
        if (pinned) history.scrollTop = history.scrollHeight;
    }
    function dismiss() {window.dadoHistory?.layout(false,projectState?.settings.layout || 0);}
    const choices = [ ['Systems larger', 3, 1], ['Balanced layout', 1, 1], ['Chat larger', 1, 3] ];
    function chooseLayout(index) {
        const [,top,bottom]=choices[index] || choices[0];
        split.dataset.layout=String(index);split.style.gridTemplateRows=`var(--attachment-rows, minmax(0, ${top}fr) minmax(0, ${bottom}fr))`;
        if(projectState){projectState.settings.layout=index;persist();}
        dismiss();resize();
    }
    const outside = event => { if (!divider.contains(event.target)) dismiss(); if (!actionMenu.contains(event.target) && !attachButton.contains(event.target) && !event.target.closest('.chat-skill-menu')) closeActions(); };
    const escape = event => { if (event.key === 'Escape' && !layouts.hidden) { dismiss(); dots.focus(); } if (event.key === 'Escape' && !actionMenu.hidden) { event.preventDefault(); closeActions(); input.focus(); } };
    document.addEventListener('pointerdown', outside);
    document.addEventListener('keydown', escape);
    function renderAttachments() {
        const comments=pendingComments();selectedComments=comments.map(item=>item.contextId);
        comments.forEach((item,index)=>{item.draftNumber=index+1;});
        const chatComments=comments.filter(item=>item.source==='Chat' && item.conversation===conversation()?.id);
        const binding=JSON.stringify(chatComments.map(item=>[item.contextId,item.from,item.to]));
        if(binding!==boundAnnotations) {
            messages.querySelectorAll('.annotation-marker-anchor').forEach(marker=>marker.remove());
            messages.querySelectorAll('.comment-highlight').forEach(mark=>mark.replaceWith(...mark.childNodes));
            messages.querySelectorAll('.chat-message-text').forEach(text=>text.normalize());
            chatComments.sort((a,b)=>b.from-a.from).forEach(bindComment);
            boundAnnotations=binding;
        }
        annotations.publish();
        resize();
        window.dadoSidebar?.draftChanged(); persist();
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
    function pickAttachments(kind) { if (picking || !loaded) return; closeActions(); picking = true; attachButton.disabled = true; pickTicket = newId(); updateSend(); send({action:'attach',kind,root:projectRoot,ticket:pickTicket}); }

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
        annotations.changed();
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
        window.dadoComposer?.restore('composer',{files:attachments,skills:selectedSkills,pendingCount:pendingComments().filter(item=>item.kind!=='idea').length,ideaCount:pendingComments().filter(item=>item.kind==='idea').length,picking,running:!!running,disabled:submit.disabled});
    }

    function syncModel() {
        syncPermissions();
        const chat=conversation();
        const settings=chat ? (chat.modelSettings ||= {model:userPreferences.model,reasoningEffort:userPreferences.reasoningEffort}) : {};
        let model=aiState.models.find(m=>m.model===settings.model);
        if (!model && aiState.models.length) {model=aiState.models.find(m=>m.isDefault) || aiState.models[0]; settings.model=model.model;}
        const efforts=model?.supportedReasoningEfforts?.map(e=>e.reasoningEffort) || [];
        if (model && !efforts.includes(settings.reasoningEffort)) {
            const legacy=['low','medium','high','xhigh','max'][settings.reasoning];
            settings.reasoningEffort=efforts.includes(legacy) ? legacy : (efforts.includes(model.defaultReasoningEffort) ? model.defaultReasoningEffort : efforts[0]) || null;
        }
        if(projectState)Object.assign(projectState.settings,settings);
        if(projectState)Object.assign(projectState.settings,settings);
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
        job.mode=mode;response.mode=mode;
        job.toolsVersion=record.skillCreation?'skill-draft-v1':mode==='inspect'?'inspect-skills-v4':mode==='approve'?'approve-skills-v3':mode==='full'?'full-native-v2':'chat-v1';
        const editorContext=window.dadoWorkbench?.agentContext() || {};
        send({action:'ai-send',requestId,root:projectRoot,projectId:job.projectId,generation:job.generation,chat:chat.id,threadId:!record.skillCreation && chat.codexAccount===account && chat.codexMode===mode && chat.codexProjectId===job.projectId && chat.codexToolsVersion===job.toolsVersion ? chat.codexThreadId : null,mode,selectedFile:editorContext.selectedFile,buffers:editorContext.buffers || [],model:projectState.settings.model,effort:projectState.settings.reasoningEffort || null,text:record.text,attachments:record.attachments,skills:record.skills || [],comments:record.comments,history,context:window.dadoSidebar?.context() || {}});
        flush();
    }
    function stopReply(all=false) {for (const job of jobs.values()) if (all || (job.root===projectRoot && job.chat.id===projectState?.active)) send({action:'ai-stop',requestId:job.requestId,root:job.root,projectId:job.projectId,generation:job.generation});}
    function renderApprovals() {publishHistory();}
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
            if(visible)publishHistory();
        }
        if (event.kind==='thread') {job.chat.codexThreadId=event.threadId;job.chat.codexAccount=JSON.stringify(aiState.account);job.chat.codexMode=job.mode;job.chat.codexProjectId=job.projectId;job.chat.codexToolsVersion=job.toolsVersion;}
        if(event.kind==='approval') {job.placeholder.phase='waiting_for_approval';job.approvals ||= new Map();job.approvals.set(event.approvalId,event);if(visible)renderApprovals(job);}
        if(event.kind==='approval_resolved') {if(job.placeholder.phase!=='stopping')job.placeholder.phase='running';job.approvals?.delete(event.approvalId);if(visible)renderApprovals(job);}
        if (event.kind==='scene_activity' && job.root===projectRoot && job.generation===projectGeneration) window.dispatchEvent(new CustomEvent('dado-scene-activity',{detail:event}));
        if(event.kind==='tool' && event.readPaths?.length && job.root===projectRoot && job.generation===projectGeneration) window.dispatchEvent(new CustomEvent('dado-scene-activity',{detail:{root:event.root,action:'read',paths:event.readPaths,status:'completed'}}));
        if(event.kind==='tool' && event.tool==='file_edit' && event.paths?.length && job.root===projectRoot && job.generation===projectGeneration) window.dispatchEvent(new CustomEvent('dado-scene-activity',{detail:{root:event.root,action:event.status==='running'?'editing':event.status==='completed'?'changed':'stopped',paths:event.paths,status:event.status}}));
        if (event.kind==='tool') {
            job.placeholder.tools ||= [];
            let activity=job.placeholder.tools.find(item=>item.id===event.itemId);
            if(!activity) {activity={id:event.itemId};job.placeholder.tools.push(activity);}
            Object.assign(activity,{tool:event.tool,status:event.status,output:event.output});
            if(visible) renderToolActivity(job.placeholder);
        }
        if(event.kind==='changes' && job.mode!=='full') {
            job.placeholder.changes={requestId:event.requestId,workspaceId:event.workspaceId,count:event.count,status:event.status};
            if(visible)renderReviewButton(job.placeholder);
        }
        if (event.kind==='delta' || event.kind==='message') {
            let record=job.records.get(event.itemId);
            if (!record) {record=job.records.size ? {id:newId(),role:'assistant',text:'',status:'streaming',time:new Date().toISOString()} : job.placeholder;record.itemId=event.itemId;if (job.records.size) {job.chat.messages.push(record);if (visible) renderMessage(record);}job.records.set(event.itemId,record);}
            record.text=event.kind==='message' ? event.text || record.text : record.text + (event.delta || '');
            if (visible) publishHistory();
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
            if(visible)publishHistory();
        }
        saveJob(job);
        if (visible && pinned) history.scrollTop=history.scrollHeight;
    }
    const observer = new ResizeObserver(resize); observer.observe(host);observer.observe(form);
    const settingsChanged = event => {
        const {model,reasoningEffort,...preferences}=event.detail;
        if(Object.keys(preferences).length)updatePreferences(preferences);
        const chat=conversation();
        if(chat && (model!==undefined || reasoningEffort!==undefined)){chat.modelSettings ||= {};if(model!==undefined)chat.modelSettings.model=model;if(reasoningEffort!==undefined)chat.modelSettings.reasoningEffort=reasoningEffort;}
        if(loaded && projectState){syncModel();persist();}
    };
    document.addEventListener('dado-settings-change',settingsChanged);
    const leaving = () => flush(); window.addEventListener('beforeunload',leaving);
    window.dadoChat = {
        attachSystems(file) { if (!loaded) return; if (!attachments.some(item=>item.path===file.path)) attachments.push(file);renderAttachments();persist();input.focus(); },
        saveNow: async()=>{await flush();await preferenceWrites;},
        preferences:()=>structuredClone(userPreferences),
        preferencesProblem:()=>preferencesProblem,
        updatePreferences,
        historyReady:publishHistory,
        historyRendered(chatId,follow,revision) {
            if(revision!==historyRevision)return;
            if(chatId!==(conversation()?.id || ''))return;
            const comments=pendingComments().filter(item=>item.source==='Chat' && item.conversation===chatId);
            if(comments.length && !messages.querySelector('.annotation-marker-anchor'))boundAnnotations=null;
            if(follow && pinned)history.scrollTop=history.scrollHeight;renderAttachments();
            requestAnimationFrame(()=>{if(revision===historyRevision)historyUpdating=false;});
        },
        historyAction(action,id,value) {
            if(action==='sent-context'){const record=conversation()?.messages.find(item=>item.id===id);const kind=value?'idea':null;const items=(record?.comments || []).filter(item=>kind?item.source==='idea':item.source!=='idea');const badge=messages.querySelector(`[data-message="${CSS.escape(id)}"] ${kind?'.sent-ideas':'.sent-annotations'}`);if(badge && items.length)annotations.list(badge.getBoundingClientRect(),items,{readonly:true});}
            else if(action==='review' || action==='apply'){const record=conversation()?.messages.find(item=>item.id===id);if(record?.changes){if(action==='review' && reviewDialog?.record.id===id && reviewDialog.shown)closeReview();else openReview(record,action==='apply');}}
            else if(action==='review-close')closeReview();
            else if(action==='review-apply')applyReview();
            else if(action==='layout-open')window.dadoHistory?.layout(true,projectState?.settings.layout || 0);
            else if(action==='layout')chooseLayout(Number(id));
            else if(action==='approval'){
                const job=jobs.get(id),approval=job?.approvals?.get(value.id);
                if(!job || job.root!==projectRoot || job.generation!==projectGeneration || job.chat.id!==conversation()?.id || !approval || approval.submitted || !['accept','decline'].includes(value.decision))return;
                approval.submitted=true;renderApprovals();
                send({action:'ai-approval',decision:value.decision,approvalId:approval.approvalId,requestId:job.requestId,root:job.root,projectId:job.projectId,generation:job.generation,chat:job.chat.id});
            }
        },
        skillCreatorReady(){skillCreator.publish();},
        skillCreator(action,key,value){skillCreator.ui(action,key,value);},
        composerReady(){updateSend();commands.publish();skillsPicker.publish();},
        composer(action,value) {
            if(action==='actions'){if(!actionMenu.hidden)closeActions(true);else commands.open('plus');}
            else if(action==='send')form.requestSubmit();
            else if(action==='remove-file'){attachments=attachments.filter(file=>file.path!==value);renderAttachments();input.focus({preventScroll:true});}
            else if(action==='remove-skill'){selectedSkills=selectedSkills.filter(skill=>skill.id!==value);renderAttachments();}
            else if(action.startsWith('actions-'))commands.ui(action.slice(8),value);
            else if(action.startsWith('skills-'))skillsPicker.ui(action.slice(7),value);
        },
        saveReviewedFile(change,text) {
            if(change.root!==projectRoot || change.projectId!==runtimeSession?.projectId || !sessionReady()) return Promise.reject(new Error('Project session changed. Reopen the review in this project before saving.'));
            return rpc('ai-save-reviewed-file',{...change,generation:projectGeneration,text});
        },
        closeActions,
        snapshot() { const chat = conversation(); return {...chat,messages:chat?.messages || [],draft:input.value,skills:[...selectedSkills],attachments:[...attachments],commentSelection:[...selectedComments]}; },
        aiEvent,
        aiState() {return {...aiState,usage};},
        modelPreferences() {return {...projectState?.settings};},
        aiAction(action) { send({action,root:projectRoot,projectId:runtimeSession?.projectId,generation:projectGeneration}); },
        selectModel(model) { const chat=conversation();if (!chat || !aiState.models.some(m=>m.model===model)) return;chat.modelSettings={model,reasoningEffort:null};syncModel();persist(); },
        selectDefaultModel(key,model) {const entry=aiState.models.find(m=>m.model===model);if(model && !entry)return;const effortKey=key==='smallModel'?'smallReasoningEffort':'reasoningEffort';updatePreferences({[key]:model || null,[effortKey]:entry?.defaultReasoningEffort || null});},
        setConversation(id, content) {
            if (!projectState) return;
            commands.load();
            let chat = projectState.conversations.find(item => item.id === id);
            if (!chat) { chat = {id,title:'New chat'}; projectState.conversations.push(chat); }
            Object.assign(chat,content); chat.messages ||= []; projectState.active = id; syncModel();renderConversation();
        },
        reply(message) { const request=pending.get(message.id); if (!request) return; clearTimeout(request.timer);pending.delete(message.id); if (message.error) request.reject(new Error(message.error)); else request.resolve(message.result); },
        async setProject(root, sessionGeneration = 0) {
            await preferencesLoaded;
            if(window.dadoProjectModel?.root!==root)window.dadoProjectModel=null;
            window.dadoConceptNotes?.bind(root);
            if (root === projectRoot && projectGeneration===sessionGeneration && (loaded || loading)) return;
            projectGeneration=sessionGeneration;
            stopReply(true); skillCreator.close(); closeReview(); window.dadoModelSelector?.close(); window.dadoPermissionSelector?.close(); flush(); closeActions(); pickTicket=null; picking=false; attachButton.disabled=false; window.dadoSidebar?.dispose(); loaded=false; loading=true; const version=++generation; projectRoot=root; projectState=null;
            clearSelectionButton(); annotations.replace([]); publishHistory(); input.value=''; attachments=[]; selectedSkills=[];renderAttachments(); input.disabled=!!root;
            window.dadoTitlebar?.sidebar(false);
            send({action:'panel',shown:false}); window.dadoModelSelector?.restore({});
            if (!root) { loading=false; split.style.gridTemplateRows=''; return; }
            try {
                // The owner emits old-run terminal events before this acknowledgement.
                // Their saves are dispatched before loading a reopened project's state.
                await waitForSession(root,sessionGeneration);
                if (version!==generation || disposed) return;
                const state=await rpc('load',{root}); if (version!==generation || disposed) return;
                projectState=state; projectState.settings ||= {}; projectState.conversations ||= [];
                if(!preferencesExist && !preferencesProblem){
                    const old=state.sidebar?.preferences || {};
                    const patch={};for(const key of ['name','personality'])if(old[key])patch[key]=old[key];
                    for(const key of ['model','reasoningEffort','agentMode'])if(state.settings[key]!=null)patch[key]=state.settings[key];
                    if(Object.keys(patch).length)updatePreferences(patch);
                }
                for(const key of ['model','reasoningEffort','agentMode'])if(userPreferences[key]!=null)state.settings[key]=userPreferences[key];
                syncModel();
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
                const [,top,bottom]=choices[index]; split.dataset.layout=String(index);split.style.gridTemplateRows=`var(--attachment-rows, minmax(0, ${top}fr) minmax(0, ${bottom}fr))`;
                window.dadoHistory?.layout(false,index);
                window.dadoTitlebar?.sidebar(!!state.settings.leftSidebar);
                send({action:'panel',shown:!!state.settings.rightPanel}); loading=false; loaded=true; resize(); flush();
                commands.load();
            } catch(error) { if (version===generation && !disposed) { loading=false; hint(`Could not open project storage: ${error.message}`); input.disabled=true; submit.disabled=true; } }
            if (version===generation && loaded) input.disabled=false;
        },
        attach(files, ticket) { if (disposed || ticket !== pickTicket) return; pickTicket=null; picking = false; host.querySelector('.chat-attach').disabled = false; files.forEach(file => { if (!attachments.some(item => item.path === file.path)) attachments.push(file); }); renderAttachments(); input.focus(); },
        dispose() { stopReply(true); skillCreator.dispose(); closeReview(); skillsPicker.dispose();commands.dispose(); window.removeEventListener('resize',menuResize); flush(); window.dadoSidebar?.dispose(); disposed = true; for(const waiter of sessionWaiters) waiter.reject(new Error('Chat closed.')); sessionWaiters.clear(); clearTimeout(saveTimer); unsubscribe(); clearSelectionButton(); document.removeEventListener('dado-settings-change',settingsChanged); window.removeEventListener('beforeunload',leaving); host.removeEventListener('mouseup', selectedText); host.removeEventListener('keyup', selectedText); observer.disconnect();systemTheme.removeEventListener('change',applyAppearance); history.removeEventListener('wheel',readingHistory);history.removeEventListener('keydown',readingHistory);history.removeEventListener('pointerdown',readingHistory); document.removeEventListener('pointerdown', outside); document.removeEventListener('keydown', escape); }
    };
    preferencesLoaded=rpc('preferences-load',{}).then(value=>{userPreferences=value.settings;preferencesExist=value.exists;applyAppearance();window.dadoSidebar?.publish();}).catch(error=>{preferencesProblem=error.message;hint(error.message);applyAppearance();});
    resize();
    send({action:'ready'}); send({action:'ai-refresh'});
};

