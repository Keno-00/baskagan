window.createSkillPicker=function({form,input,load,choose,hint,closeOthers}) {
    let catalog=[],results=[],active=-1,serial=0,shown=false,query='',empty='No skills';
    function publish(){const rect=form.getBoundingClientRect();window.dadoComposer?.restore('skills',{shown,mode:'skills',query,active,rect:{left:rect.left,top:rect.top,width:rect.width},rows:results.map(skill=>({id:skill.id,label:skill.name,icon:'M5 4h14v16H5zM8 8h8M8 12h8M8 16h5',enabled:skill.enabled,help:skill.reason || `${skill.description} · ${skill.scope} · ${skill.path}`})),empty});}
    function close(focus=false){serial++;shown=false;publish();if(focus)input.focus();}
    function select(index){active=index;publish();if(results[index])hint(results[index].reason || `${results[index].description} · ${results[index].scope} · ${results[index].path}`);requestAnimationFrame(()=>document.getElementById(`chat-skill-${index}`)?.scrollIntoView({block:'nearest'}));}
    function invoke(skill){if(!skill?.enabled)return;choose(skill);close(true);}
    function render(){const value=query.toLowerCase();results=catalog.filter(s=>`${s.name} ${s.description} ${s.scope}`.toLowerCase().includes(value));select(results.findIndex(s=>s.enabled));}
    const key=event=>{if(!shown || event.target.id!=='chat-skill-search' || event.isComposing)return;if(event.key==='Escape'){event.preventDefault();event.stopPropagation();close(true);}else if(event.key==='Tab'){close();}else if(['ArrowDown','ArrowUp','Home','End'].includes(event.key)){event.preventDefault();const available=results.map((s,i)=>s.enabled?i:-1).filter(i=>i>=0);if(!available.length)return;const current=available.indexOf(active),next=event.key==='Home'?0:event.key==='End'?available.length-1:(current+(event.key==='ArrowUp'?-1:1)+available.length)%available.length;select(available[next]);}else if(event.key==='Enter'){event.preventDefault();event.stopPropagation();invoke(results[active]);}};
    const outside=event=>{if(!event.target.closest('#chat-skill-menu'))close();};document.addEventListener('pointerdown',outside);document.addEventListener('keydown',key);
    return {close,publish,ui(action,value){if(action==='filter'){query=value;render();}if(action==='select')select(Number(value));if(action==='choose')invoke(results[Number(value)]);},async open(){closeOthers();const version=++serial;shown=true;query='';results=[];empty='Loading…';publish();requestAnimationFrame(()=>document.getElementById('chat-skill-search')?.focus());try{const value=await load();if(version!==serial)return;catalog=value.skills;empty='No skills';render();if(value.errors?.length)hint(value.errors.join(' · '));}catch(error){if(version===serial){empty='Skills unavailable';publish();hint(error.message);}}},dispose(){close();document.removeEventListener('pointerdown',outside);document.removeEventListener('keydown',key);}};
};

window.createSkillCreator=function({rpc,session,startDraft,openEditor,buffers,hint}) {
 let shown=false,version=0,reset=0,owner=null,drafts=[],skills=[],loading=false,error='',busy=new Set();
 const sameSession=()=>JSON.stringify(owner)===JSON.stringify(session());
 const bufferFor=item=>buffers().find(b=>window.dadoReview.pathKey(b.path)===window.dadoReview.pathKey(item.path));
 function publish(){window.dadoSkillCreator?.restore({shown,version,reset,loading,error,drafts:drafts.map(item=>({...item,canPublish:!!bufferFor(item)})),skills,busy:[...busy]});}
 function close(){document.querySelector(".skill-creator")?.close();version++;shown=false;publish();}
 async function open(){version++;const ticket=version;owner=session();shown=true;drafts=[];skills=[];error='';loading=true;busy.clear();publish();
  try{const result=await rpc('ai-skill-drafts',owner);if(ticket!==version)return;drafts=result;publish();const privateSkills=await rpc('ai-private-skills',owner);if(ticket!==version)return;skills=privateSkills;}
  catch(e){if(ticket===version){error=e.message;hint(e.message);}}finally{if(ticket===version){loading=false;publish();}}
 }
 async function ui(action,key,value){
  if(action==='close'){close();return;}
  if(action==='draft'){if(startDraft(value)){reset++;close();}return;}
  const item=(action==='share'||action==='review-private'?skills:drafts).find(item=>item.id===key);if(!item)return;
  if(action==='review'||action==='review-private'){openEditor(item.path);close();return;}
  if(busy.has(item.id) || !sameSession())return;const ticket=version;busy.add(item.id);publish();
  try{
   const buffer=bufferFor(item);
   if(action==='publish'){
    if(!buffer)throw new Error('Open the draft in the editor before publishing');
    await rpc('ai-skill-publish',{...owner,draftId:item.id,original:buffer.original,text:buffer.text});
    if(ticket===version){drafts=drafts.filter(d=>d.id!==item.id);hint(`Published ${item.name} · ${item.scope==='repo'?'Repository':'Private project'} skill`);}
   }else if(action==='share'){
    if(buffer && buffer.text!==buffer.original)throw new Error('Save your skill edits before sharing');
    await rpc('ai-skill-export',{...owner,skillId:item.id,revision:item.revision});
    if(ticket===version)hint(`Shared ${item.name} with the repository · Private copy preserved`);
   }
  }catch(e){if(ticket===version){error=e.message;hint(e.message);}}finally{if(ticket===version){busy.delete(item.id);publish();}}
 }
 return {open,close,publish,ui,dispose:close};
};
