window.createSkillPicker=function({form,input,load,choose,hint,closeOthers}) {
    const menu=document.createElement('div');menu.className='chat-action-menu chat-skill-menu';menu.hidden=true;menu.setAttribute('role','dialog');menu.setAttribute('aria-label','Skills');
    const filter=document.createElement('input');filter.className='chat-action-search';filter.placeholder='Find skills';filter.setAttribute('aria-label','Find skills');
    const list=document.createElement('div');list.setAttribute('role','listbox');list.setAttribute('aria-label','Available skills');menu.append(filter,list);document.body.append(menu);
    let catalog=[],results=[],active=-1,serial=0;
    function close(focus=false){serial++;menu.hidden=true;if(focus)input.focus();}
    function select(index){active=index;for(const [i,row]of [...list.children].entries()){row.setAttribute('aria-selected',String(i===index));if(i===index)row.scrollIntoView({block:'nearest'});}if(results[index])hint(results[index].reason || `${results[index].description} · ${results[index].scope} · ${results[index].path}`);}
    function invoke(skill){if(!skill.enabled)return;choose(skill);close(true);}
    function render(){const query=filter.value.toLowerCase();results=catalog.filter(s=>`${s.name} ${s.description} ${s.scope}`.toLowerCase().includes(query));list.replaceChildren();
        for(const [i,skill]of results.entries()){const row=document.createElement('button');row.type='button';row.tabIndex=-1;row.setAttribute('role','option');row.setAttribute('aria-disabled',String(!skill.enabled));
            row.innerHTML='<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 4h14v16H5zM8 8h8M8 12h8M8 16h5"/></svg>';
            const label=document.createElement('span');label.textContent=skill.name;row.append(label);row.onmouseenter=()=>select(i);row.onpointerdown=e=>e.preventDefault();row.onclick=()=>invoke(skill);list.append(row);}
        if(!results.length){const empty=document.createElement('div');empty.className='chat-action-empty';empty.textContent='No skills';list.append(empty);}
        select(results.findIndex(s=>s.enabled));
    }
    filter.oninput=render;
    menu.onkeydown=event=>{if(event.isComposing)return;if(event.key==='Escape'){event.preventDefault();event.stopPropagation();close(true);}else if(event.key==='Tab'){close();}else if(['ArrowDown','ArrowUp','Home','End'].includes(event.key)){event.preventDefault();const available=results.map((s,i)=>s.enabled?i:-1).filter(i=>i>=0);if(!available.length)return;const current=available.indexOf(active),next=event.key==='Home'?0:event.key==='End'?available.length-1:(current+(event.key==='ArrowUp'?-1:1)+available.length)%available.length;select(available[next]);}else if(event.key==='Enter'){event.preventDefault();event.stopPropagation();if(results[active])invoke(results[active]);}};
    const outside=event=>{if(!menu.contains(event.target))close();};document.addEventListener('pointerdown',outside);
    return {close,async open(){closeOthers();const version=++serial;const rect=form.getBoundingClientRect();menu.style.left=rect.left+'px';menu.style.width=rect.width+'px';menu.style.bottom=(innerHeight-rect.top+8)+'px';menu.style.maxHeight=Math.min(320,Math.max(40,rect.top-16))+'px';menu.hidden=false;filter.value='';list.textContent='Loading…';filter.focus();try{const value=await load();if(version!==serial)return;catalog=value.skills;render();if(value.errors?.length)hint(value.errors.join(' · '));}catch(error){if(version===serial){list.textContent='Skills unavailable';hint(error.message);}}},dispose(){close();document.removeEventListener('pointerdown',outside);menu.remove();}};
};

window.createSkillCreator=function({rpc,session,startDraft,openEditor,buffers,hint}) {
    let dialog=null,version=0;
    function close(){version++;dialog?.remove();dialog=null;}
    function button(label,action){const b=document.createElement('button');b.type='button';b.textContent=label;b.onclick=action;return b;}
    async function open(){
        close();const ticket=version,owner=session();
        const sheet=document.createElement('dialog');sheet.className='skill-creator';sheet.setAttribute('aria-label','Create skill');dialog=sheet;
        const form=document.createElement('form');form.innerHTML='<h3>Create skill</h3><label>Name<input name="name" required pattern="[a-z0-9]+(?:-[a-z0-9]+)*" maxlength="64" placeholder="review-changes"></label><label>What should it do?<textarea name="task" required maxlength="4000" rows="3"></textarea></label><label>When should it run?<textarea name="when" required maxlength="2000" rows="2"></textarea></label><label>Scope<select name="scope"><option value="project-private">This project · private</option><option value="repo">This repository · shared</option></select></label>';
        const actions=document.createElement('div');actions.className='skill-creator-actions';
        const draft=document.createElement('button');draft.type='submit';draft.textContent='Draft with agent';actions.append(button('Cancel',close),draft);form.append(actions);
        const existing=document.createElement('div');existing.className='skill-draft-list';sheet.append(form,existing);document.body.append(sheet);sheet.showModal();
        sheet.addEventListener('cancel',e=>{e.preventDefault();close();});
        sheet.addEventListener('keydown',e=>e.stopPropagation());
        form.onsubmit=e=>{e.preventDefault();const data=new FormData(form);if(startDraft({name:data.get('name'),task:data.get('task'),when:data.get('when'),scope:data.get('scope')}))close();};
        try {
            const drafts=await rpc('ai-skill-drafts',owner);if(ticket!==version)return;
            for(const item of drafts){
                const row=document.createElement('div');row.className='skill-draft-row';
                const name=document.createElement('span');name.textContent=item.name;row.append(name);
                const publish=button('Publish',async()=>{
                    publish.disabled=true;
                    try{
                        const buffer=buffers().find(b=>window.dadoReview.pathKey(b.path)===window.dadoReview.pathKey(item.path));
                        if(!buffer)throw new Error('Open the draft in the editor before publishing');
                        if(ticket!==version || JSON.stringify(owner)!==JSON.stringify(session()))return;
                        await rpc('ai-skill-publish',{...owner,draftId:item.id,original:buffer.original,text:buffer.text});
                        if(ticket===version){row.remove();hint(`Published ${item.name} · ${item.scope==='repo'?'Repository':'Private project'} skill`);}
                    }catch(error){if(ticket===version){hint(error.message);publish.disabled=false;}}
                });publish.disabled=!buffers().some(b=>window.dadoReview.pathKey(b.path)===window.dadoReview.pathKey(item.path));
                const review=button('',()=>{openEditor(item.path);close();});review.setAttribute('aria-label',`Review ${item.name}`);
                review.innerHTML='<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M14 3H6v18h12V7zM14 3v5h4M9 12h6M9 16h6"/></svg>';
                review.onmouseenter=()=>hint(`Review draft · ${item.scope==='repo'?'Publishes to the repository':'Private to this project'}`);
                row.append(review,publish);existing.append(row);
            }
            const skills=await rpc('ai-private-skills',owner);if(ticket!==version)return;
            for(const item of skills){
                const row=document.createElement('div');row.className='skill-draft-row';
                const name=document.createElement('span');name.textContent=item.name;row.append(name);
                const review=button('',()=>{openEditor(item.path);close();});review.setAttribute('aria-label',`Review private skill ${item.name}`);
                review.innerHTML='<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M14 3H6v18h12V7zM14 3v5h4M9 12h6M9 16h6"/></svg>';
                const share=button('',async()=>{
                    share.disabled=true;
                    try{
                        const buffer=buffers().find(b=>window.dadoReview.pathKey(b.path)===window.dadoReview.pathKey(item.path));
                        if(buffer && buffer.text!==buffer.original)throw new Error('Save your skill edits before sharing');
                        if(ticket!==version || JSON.stringify(owner)!==JSON.stringify(session()))return;
                        await rpc('ai-skill-export',{...owner,skillId:item.id,revision:item.revision});
                        if(ticket===version)hint(`Shared ${item.name} with the repository · Private copy preserved`);
                    }catch(error){if(ticket===version){share.disabled=false;hint(error.message);}}
                });share.setAttribute('aria-label',`Share ${item.name} with repository`);
                share.innerHTML='<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M13 4h7v7M20 4 10 14M10 5H4v15h15v-6"/></svg>';
                share.onmouseenter=()=>hint('Copy this private skill and its supporting files to .agents/skills for repository sharing');
                row.append(review,share);existing.append(row);
            }
        }catch(error){if(ticket===version)hint(error.message);}
    }
    return {open,close,dispose:close};
};
