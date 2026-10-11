(function (scope) {
    function slashQuery(text, caret) {
        const start = text.lastIndexOf('\n', caret - 1) + 1;
        if ((text.slice(0,start).match(/^\s*```/gm) || []).length % 2) return null;
        const match = /^\/([a-z0-9-]*)$/i.exec(text.slice(start,caret));
        return match ? {from:start,to:caret,query:match[1]} : null;
    }
    function search(catalog, query) {
        query=query.trim().toLowerCase().replace(/^\//,'');
        const score=c=>!query ? 0 : c.alias===query ? 0 : c.alias.startsWith(query) ? 1 : c.label.toLowerCase().includes(query) ? 2 : c.keywords.toLowerCase().includes(query) ? 3 : Infinity;
        return catalog.map((command,order)=>({command,order,score:score(command)})).filter(x=>Number.isFinite(x.score)).sort((a,b)=>a.score-b.score || a.order-b.order).map(x=>x.command);
    }
    function move(index,count,key) {
        if(!count) return -1;
        if(key==='Home') return 0;
        if(key==='End') return count-1;
        return (index+(key==='ArrowUp' ? -1 : 1)+count)%count;
    }
    function panel({form,input,trigger,rpc,root,ready,hint,dispatch,changed}) {
        const menu=document.createElement('div');menu.className='chat-action-menu';menu.hidden=true;menu.id='chat-action-menu';menu.setAttribute('role','dialog');menu.setAttribute('aria-label','Chat actions');
        const filter=document.createElement('input');filter.className='chat-action-search';filter.placeholder='Search actions';filter.setAttribute('aria-label','Search actions');
        const list=document.createElement('div');list.id='chat-action-results';list.setAttribute('role','listbox');menu.append(filter,list);document.body.append(menu);
        let catalog=[],results=[],active=0,mode=null,span=null,busy=false,disposed=false,serial=0;
        trigger.setAttribute('aria-controls',menu.id);
        for(const field of [input,filter]) {field.setAttribute('aria-controls',list.id);field.setAttribute('aria-autocomplete','list');}
        function close(focus=false) {
            const previous=mode;mode=null;span=null;menu.hidden=true;trigger.setAttribute('aria-expanded','false');
            for(const field of [input,filter]) {field.removeAttribute('aria-activedescendant');field.setAttribute('aria-expanded','false');}
            if(focus) (previous==='slash' ? input : trigger).focus();
        }
        function select(index) {
            active=index;
            [...list.querySelectorAll('button')].forEach((button,i)=>{button.setAttribute('aria-selected',String(i===index));if(i===index)button.scrollIntoView({block:'nearest'});});
            const field=mode==='slash' ? input : filter;
            if(results[index]) {field.setAttribute('aria-activedescendant',`chat-command-${index}`);hint(results[index].help);}else field.removeAttribute('aria-activedescendant');
        }
        function render(query) {
            results=search(catalog,query);list.replaceChildren();
            results.forEach((command,index)=>{
                const button=document.createElement('button');button.type='button';button.id=`chat-command-${index}`;button.tabIndex=-1;button.setAttribute('role','option');
                button.innerHTML=`<svg viewBox="0 0 24 24" aria-hidden="true"><path d="${command.icon}"/></svg>`;
                const label=document.createElement('span');label.textContent=command.label;button.append(label);
                button.onpointerdown=e=>e.preventDefault();button.onmouseenter=()=>select(index);button.onclick=()=>invoke(command);list.append(button);
            });
            if(!results.length) {const empty=document.createElement('div');empty.className='chat-action-empty';empty.textContent='No actions';list.append(empty);}
            select(results.length ? 0 : -1);
        }
        function open(next,querySpan=null) {
            if(!ready()) return;
            window.dadoModelSelector?.close();mode=next;span=querySpan;
            const rect=form.getBoundingClientRect();menu.style.left=`${rect.left}px`;menu.style.width=`${rect.width}px`;menu.style.bottom=`${window.innerHeight-rect.top+8}px`;menu.style.maxHeight=`${Math.min(320,Math.max(40,rect.top-16))}px`;
            menu.hidden=false;filter.hidden=next==='slash';filter.value='';trigger.setAttribute('aria-expanded','true');
            (next==='slash' ? input : filter).setAttribute('aria-expanded','true');render(querySpan?.query || '');if(next==='plus')filter.focus();
        }
        async function invoke(command) {
            if(busy) return;
            const project=root(),draft=input.value,querySpan=span && {...span},version=serial;busy=true;
            try {
                const validated=await rpc('command',{root:project,command:command.id});
                if(disposed || version!==serial || project!==root() || draft!==input.value) return;
                close();
                if(querySpan) {input.focus();input.setSelectionRange(querySpan.from,querySpan.to);if(!document.execCommand('insertText',false,''))input.setRangeText('',querySpan.from,querySpan.to,'end');changed();}
                dispatch(validated.id);
            }catch(error) {hint(error.message);}finally {busy=false;}
        }
        function key(event) {
            if(menu.hidden || event.isComposing) return false;
            if(event.key==='Tab' || event.key==='Escape' || (event.key==='Enter' && event.shiftKey)) {close();if(event.key==='Escape'){event.preventDefault();input.focus();}return event.key==='Escape';}
            if(['ArrowDown','ArrowUp','Home','End'].includes(event.key)){event.preventDefault();select(move(active,results.length,event.key));return true;}
            if(event.key==='Enter'){event.preventDefault();if(results[active])invoke(results[active]);return true;}
            return false;
        }
        filter.oninput=()=>render(filter.value);menu.onkeydown=key;
        return {menu,close,key,open,
            async load() {const version=++serial;catalog=[];close();if(!root())return;try{const value=await rpc('commands',{root:root()});if(version===serial && !disposed)catalog=value;}catch(error){hint(error.message);}},
            update(event) {if(event.isComposing)return;const next=slashQuery(input.value,input.selectionStart);if(mode==='slash'){if(next){span=next;render(next.query);}else close();}else if(event.inputType==='insertText' && event.data==='/' && next)open('slash',next);},
            dispose() {disposed=true;serial++;close();menu.remove();}
        };
    }
    const api={slashQuery,search,move,panel};
    if(typeof module!=='undefined') module.exports=api;
    scope.dadoCommands=api;
})(typeof window==='undefined' ? globalThis : window);
