window.startSidebar=function(send,saved,avatar,loadError,projectRoot=null){
 const valid=saved && saved.version===1 && Array.isArray(saved.chats) && Array.isArray(saved.concepts);
 const state=valid?saved:{version:1,chats:[],concepts:[],active:null,expanded:{recents:true,concepts:true}};
 state.expanded ||= {recents:true,concepts:true};
 let activeConcept=state.chats.find(c=>c.id===state.active)?.concept || null,showAll=false,popup=null,timer=null,settingsOpen=false,settingsTab='general',revision=0;
 let storageProblem=loadError || (saved && !valid?'Saved chats use an unsupported format. The original file has been preserved.':null);
 const id=()=>crypto.randomUUID?.() || Array.from(crypto.getRandomValues(new Uint8Array(16)),byte=>byte.toString(16).padStart(2,'0')).join('');
 const hint=text=>send({action:'hint',text});
 function persist(){if(!loadError && !(saved && !valid))send({action:'save',state});}
 function later(){clearTimeout(timer);timer=setTimeout(persist,200);}
 function capture(){const chat=state.chats.find(c=>c.id===state.active);if(chat && window.dadoChat?.snapshot)chat.content=window.dadoChat.snapshot();}
 function recent(){return [...state.chats].filter(c=>c.content?.messages?.length || c.content?.draft?.trim() || c.id===state.active).sort((a,b)=>b.visited-a.visited);}
 function usageRemaining(){
  const limits=window.dadoChat?.aiState()?.usage,bucket=limits?.rateLimitsByLimitId?.codex || limits?.rateLimits || Object.values(limits?.rateLimitsByLimitId || {})[0];
  const values=[bucket?.primary,bucket?.secondary].filter(w=>w && Number.isFinite(w.usedPercent)).map(w=>{const m=w.windowDurationMins;return `${m?m%1440===0?`${m/1440}d `:m%60===0?`${m/60}h `:`${m}m `:''}${Math.round(Math.max(0,Math.min(100,100-w.usedPercent)))}%`;});return values.length?values.join(' · '):'Unavailable';
 }
 function publish(){
  const chats=recent(),preferences=window.dadoChat?.preferences() || {};
  window.dadoSidebarUI?.restore({revision,avatar,active:state.active || '',showAll,expanded:state.expanded,recents:chats.slice(0,showAll?undefined:7).map(({id,title})=>({id,title})),more:chats.length>7,
   concepts:state.concepts.map(c=>({...c,chats:state.chats.filter(chat=>chat.concept===c.id).sort((a,b)=>b.visited-a.visited).map(({id,title})=>({id,title}))})),
   popup,settingsOpen,settingsTab,preferences,ai:window.dadoChat?.aiState() || {},modelPreferences:window.dadoChat?.modelPreferences() || {},usage:usageRemaining(),problem:storageProblem || window.dadoChat?.preferencesProblem() || ''});
 }
 function closePopup(){popup=null;publish();}
 function closeSettings(focus=true){settingsOpen=false;publish();if(focus)requestAnimationFrame(()=>document.querySelector('.sidebar-account')?.focus());}
 function openSettings(tab=settingsTab){settingsTab=tab;settingsOpen=true;popup=null;publish();requestAnimationFrame(()=>document.querySelector('.settings-nav [aria-current="page"]')?.focus());}
 function select(chat){capture();popup=null;settingsOpen=false;state.active=chat.id;activeConcept=chat.concept || null;chat.visited=Date.now();window.dadoChat?.setConversation(chat.id,chat.content || {});publish();later();}
 function newChat(concept=activeConcept){capture();const chat={id:id(),title:'New chat',concept:concept || null,visited:Date.now(),content:{messages:[],draft:'',attachments:[]}};state.chats.unshift(chat);select(chat);}
 function editConcept(concept,triggerId){
  const trigger=document.getElementById(triggerId),host=document.getElementById('left-sidebar');const rect=trigger?.getBoundingClientRect(),bounds=host.getBoundingClientRect();
  popup={kind:'concept',id:concept?.id || '',name:concept?.name || '',paths:concept?.paths || [],bottom:Math.max(54,bounds.bottom-(rect?.top || bounds.bottom)+6)};revision++;publish();
  requestAnimationFrame(()=>document.getElementById('concept-name')?.focus());
 }
 const outside=event=>{if(popup && !event.target.closest('.sidebar-popup,.sidebar-account,[data-concept],#create-concept'))closePopup();};
 const escape=event=>{if(event.key==='Escape'){if(popup)closePopup();else if(settingsOpen)closeSettings();}};
 document.addEventListener('pointerdown',outside);document.addEventListener('keydown',escape);
 window.dadoSidebar={
  publish,refreshAI:publish,openModelSettings(){openSettings('model');},openAgentSettings(){openSettings('agent');},newChat(){newChat(null);},
  ui(action,key,value){
   if(action==='new')newChat(key || null);
   else if(action==='select'){const chat=state.chats.find(c=>c.id===key);if(chat)select(chat);}
   else if(action==='expand'){state.expanded[key]=!state.expanded[key];publish();later();}
   else if(action==='more'){showAll=!showAll;publish();}
   else if(action==='concept-toggle'){const c=state.concepts.find(c=>c.id===key);if(c){c.open=!c.open;window.dadoSidebar.selectConcept(key);publish();later();}}
   else if(action==='concept-edit')editConcept(state.concepts.find(c=>c.id===key),key?`concept-${key}`:'create-concept');
   else if(action==='concept-save'){
    const name=value.trim().slice(0,100);if(!name)return;let concept=state.concepts.find(c=>c.id===key);
    if(concept){concept.name=name;closePopup();publish();later();}
    else{concept={id:id(),name,paths:[],open:true};state.concepts.push(concept);newChat(concept.id);editConcept(concept,`concept-${concept.id}`);}
   }
   else if(action==='path-remove'){const c=state.concepts.find(c=>c.id===key);if(c){c.paths=c.paths.filter(p=>p.path!==value);if(popup?.id===key)popup.paths=c.paths;publish();later();}}
   else if(action==='pick')send({action:'pick',concept:key,kind:value});
   else if(action==='popup-close')closePopup();
   else if(action==='account'){popup=popup?.kind==='account'?null:{kind:'account',id:'',name:'',paths:[],bottom:54};revision++;publish();}
   else if(action==='settings')openSettings(key || settingsTab);
   else if(action==='settings-close')closeSettings();
   else if(action==='preference')window.dadoChat?.updatePreferences({[key]:value});
   else if(action==='general')window.dadoChat?.updatePreferences({general:{[key]:value}});
   else if(action==='appearance')window.dadoChat?.updatePreferences({appearance:{[key]:value}});
   else if(action==='appearance-reset')window.dadoChat?.updatePreferences({appearance:{theme:'system',accentColor:'blue',font:'system',textSize:12,reducedMotion:false,backgroundMotion:true,motionIntensity:'balanced',navigationMotion:true,agentMotion:true,nodeAnimations:true,respectReducedMotion:true}});
   else if(action==='default-model'){window.dadoChat?.selectDefaultModel(key,value);publish();}
   else if(action==='setting')document.dispatchEvent(new CustomEvent('dado-settings-change',{detail:{[key]:value}}));
   else if(action==='ai')window.dadoChat?.aiAction(key);
  },
  snapshot(){capture();return state;},
  dispose(){clearTimeout(timer);popup=null;settingsOpen=false;publish();document.removeEventListener('pointerdown',outside);document.removeEventListener('keydown',escape);window.dadoSidebar=null;},
  changed(){capture();const chat=state.chats.find(c=>c.id===state.active);if(chat){const first=chat.content.messages?.[0];chat.title=(first?.text || first?.attachments?.[0]?.name || first?.comments?.[0]?.text || 'New chat').replace(/\s+/g,' ').slice(0,70);chat.visited=Date.now();}publish();later();},
  draftChanged(){capture();later();},
  attachmentsChanged(chatId,attachments){const chat=state.chats.find(c=>c.id===chatId);if(chat){chat.content.attachments=attachments.map(file=>({...file}));later();}},
  selectConcept(key){const c=state.concepts.find(c=>c.id===key);if(!c)return;const chat=state.chats.filter(c=>c.concept===key).sort((a,b)=>b.visited-a.visited)[0];if(chat)select(chat);else newChat(key);},
  syncModel(data){if((projectRoot && data?.root!==projectRoot) || !data?.model?.concepts?.length)return;const m=data.model,root=data.root,ids=new Set(m.concepts.map(c=>'model-'+c.id));state.concepts=state.concepts.filter(c=>!c.generated || ids.has(c.id) || state.chats.some(chat=>chat.concept===c.id));for(const c of m.concepts){const old=state.concepts.find(n=>n.id==='model-'+c.id),ancestors=[];let parent=c.parent;const visited=new Set();while(parent && !visited.has(parent)){visited.add(parent);const p=m.concepts.find(n=>n.id===parent);if(!p)break;ancestors.unshift({id:p.id,name:p.name});parent=p.parent;}const entry={id:'model-'+c.id,name:c.name,description:c.description,generated:true,ancestors,connections:m.connections.filter(e=>e.from===c.id || e.to===c.id).map(e=>({...e,fromName:m.concepts.find(c=>c.id===e.from)?.name,toName:m.concepts.find(c=>c.id===e.to)?.name})),sources:c.sources,paths:c.sources.map(s=>({...s,revision:m.revisions?.[s.path] || null,path:root.replace(/[\\/]$/,'')+'/'+s.path})),open:old?.open || false};if(old)Object.assign(old,entry);else state.concepts.push(entry);}publish();later();},
  context(){const c=state.concepts.find(c=>c.id===activeConcept),p=window.dadoChat?.preferences() || {};return {concept:c?{id:c.id,name:c.name,description:c.description,inspection:window.dadoNodeInsight?.root===projectRoot && window.dadoNodeInsight?.conceptId===c.id?window.dadoNodeInsight.explanation:null,ancestors:c.ancestors,connections:c.connections,sources:c.sources,notes:(window.dadoComments?.comments || []).filter(n=>n.conceptId===c.id && !n.resolved),paths:c.paths.map(p=>({...p}))}:null,personalization:{name:p.name || '',personality:p.personality || ''}};},
  addPaths(conceptId,paths){const c=state.concepts.find(c=>c.id===conceptId);if(!c)return;paths.forEach(path=>{if(!c.paths.some(p=>p.path===path.path))c.paths.push(path);});if(popup?.id===conceptId)popup.paths=c.paths;publish();later();},
  storageError(message){storageProblem=message;publish();}
 };
 publish();if(window.dadoChat?.setConversation){const chat=state.chats.find(c=>c.id===state.active);if(chat)window.dadoChat.setConversation(chat.id,chat.content || {});else newChat(null);}if(window.dadoProjectModel)window.dadoSidebar.syncModel(window.dadoProjectModel);
};
