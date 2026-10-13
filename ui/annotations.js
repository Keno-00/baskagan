if (!window.dadoComments) {
    const comments = [], listeners = new Set();
    let next = 0, popup = null, draft = null, selection = null, frame = 0, revision = 0;
    const anchors = new Map();
    const glyph = '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 4h16v12H9l-5 4z"/></svg>';
    const identity = () => crypto.randomUUID?.() || `${Date.now()}-${Array.from(crypto.getRandomValues(new Uint32Array(3))).join('-')}`;
    const rectValue = r => ({left:r.left,top:r.top,right:r.right,bottom:r.bottom,width:r.width,height:r.height});
    const itemValue = item => ({id:item.contextId || String(item.id),label:['Systems','systems','idea'].includes(item.source) ? item.quote : item.source === 'Chat' || item.source === 'chat' ? item.quote : `L${item.lineFrom}-L${item.lineTo}`,quote:item.quote || '',text:item.text || ''});
    function publish() {
        cancelAnimationFrame(frame); frame = requestAnimationFrame(() => {
            const controls=[];
            for(const [key,entry] of anchors) {
                if(!entry.el.isConnected) {anchors.delete(key);continue;}
                const items=entry.items(); const rect=entry.el.getBoundingClientRect();
                const viewport=entry.el.closest('.chat-history')?.getBoundingClientRect();
                if(!items.length || !rect.width || !rect.height || (viewport && (rect.bottom<viewport.top || rect.top>viewport.bottom)))continue;
                controls.push({key,kind:entry.kind,rect:rectValue(rect),count:items.length,preview:entry.kind==='marker'?items[0].text:'',text:entry.kind.startsWith('ideas')?`${items.length} Ideas/Things`:entry.kind==='marker'?String(items[0].draftNumber || items[0].id):`${items.length} annotation${items.length===1?'':'s'}`});
            }
            const state={revision,popup,selection:selection?rectValue(selection.rect):null,controls};
            window.dadoAnnotationState=state;
            document.dispatchEvent(new CustomEvent('dado-annotations',{detail:state}));
        });
    }
    function close() {popup=null;draft=null;publish();}
    function notify() {if(!comments.length)next=0;listeners.forEach(listener=>listener(comments));publish();}
    function edit(item,rect,fresh=false) {
        selection=null; draft={item,fresh};
        popup={idea:item.kind==='idea',kind:'edit',rect:rectValue(rect),items:[itemValue(item)],fresh,readonly:false};revision++;publish();
    }
    function list(rect,items=comments,options={}) {
        selection=null;draft=null;
        popup={idea:items.length>0 && items.every(item=>item.kind==='idea' || item.source==='idea'),kind:'list',rect:rectValue(rect),items:items.map(itemValue),readonly:!!options.readonly};revision++;publish();
    }
    function anchor(el,key,items,kind='summary',remove=null) {anchors.set(key,{el,items,kind,remove});publish();}
    function command(action,key,value) {
        if(action==='close'){close();return;}
        if(action==='select'){const chosen=selection;selection=null;chosen?.open();publish();return;}
        if(action==='open') {
            const entry=anchors.get(key);if(!entry)return;
            const source=entry.el.getBoundingClientRect(),composer=entry.el.closest('#chat-host')?.querySelector('.chat-composer')?.getBoundingClientRect();
            const rect=composer?{left:composer.left,right:composer.right,width:composer.width,top:entry.kind==='summary'?composer.top:source.top,bottom:source.bottom,height:source.height}:source;
            list(rect,entry.items(),{readonly:entry.kind==='sent' || entry.kind==='ideas-sent'});return;
        }
        if(action==='clear') {const entry=anchors.get(key);entry?.remove?.();close();return;}
        const item=comments.find(item=>(item.contextId || String(item.id))===key);
        if(action==='edit' && item){edit(item,popup.rect);return;}
        if(action==='delete' && item){comments.splice(comments.indexOf(item),1);item.onDelete?.();close();notify();return;}
        if(action==='save' && draft && value.trim()) {
            const {item,fresh}=draft;item.text=value.trim();
            if(fresh){item.id=++next;item.contextId=identity();comments.push(item);item.onSave?.(item);}
            close();notify();
            document.querySelector('.chat-composer textarea')?.focus();
        }
    }
    document.addEventListener('pointerdown',event=>{if(popup && !event.target.closest('#annotation-ui, .annotation-trigger, .comment-marker, .comment-badge'))close();});
    document.addEventListener('keydown',event=>{if(event.key==='Escape'){selection=null;close();}});
    document.addEventListener('scroll',publish,true);
    window.addEventListener('resize',()=>{selection=null;close();});
    new ResizeObserver(publish).observe(document.documentElement);
    new MutationObserver(publish).observe(document.getElementById('chat-host'),{childList:true,subtree:true,characterData:true,attributes:true,attributeFilter:['hidden','class','style']});
    window.dadoComments={comments,glyph,begin:(item,rect)=>edit(item,rect,true),edit,list,close,command,anchor,publish,
        select(rect,open){selection=rect?{rect,open}:null;publish();},
        replace(items){close();items.forEach(item=>{item.contextId ||= identity();});comments.splice(0,comments.length,...items);next=Math.max(0,...items.map(item=>item.id || 0));notify();},
        subscribe(listener){listeners.add(listener);listener(comments);return()=>listeners.delete(listener);},changed:notify};
}
