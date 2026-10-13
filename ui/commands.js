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
        const menu={hidden:true,contains:target=>!!target.closest('#chat-action-menu')};
        let catalog=[],results=[],active=0,mode=null,span=null,busy=false,disposed=false,serial=0,query='';
        const filter=()=>document.getElementById('chat-action-search');
        input.setAttribute('aria-controls','chat-action-menu-results');input.setAttribute('aria-autocomplete','list');
        function publish() {
            const rect=form.getBoundingClientRect();
            window.dadoComposer?.restore('actions',{shown:!menu.hidden,mode:mode || '',query,active,rect:{left:rect.left,top:rect.top,width:rect.width},rows:results.map(command=>({...command,enabled:true})),empty:'No actions'});
        }
        function close(focus=false) {
            const previous=mode;mode=null;span=null;menu.hidden=true;publish();
            input.removeAttribute('aria-activedescendant');input.setAttribute('aria-expanded','false');
            if(focus)(previous==='slash'?input:trigger).focus();
        }
        function select(index) {
            active=index;publish();
            const field=mode==='slash'?input:filter();
            if(results[index]){field?.setAttribute('aria-activedescendant',`chat-command-${index}`);hint(results[index].help);}else field?.removeAttribute('aria-activedescendant');
            requestAnimationFrame(()=>document.getElementById(`chat-command-${index}`)?.scrollIntoView({block:'nearest'}));
        }
        function render(value) {query=value;results=search(catalog,query);select(results.length?0:-1);}
        function open(next,querySpan=null) {
            if(!ready())return;
            window.dadoModelSelector?.close();mode=next;span=querySpan;menu.hidden=false;
            input.setAttribute('aria-expanded',String(next==='slash'));render(querySpan?.query || '');
            if(next==='plus')requestAnimationFrame(()=>filter()?.focus());
        }
        async function invoke(command) {
            if(busy || !command)return;
            const project=root(),draft=input.value,querySpan=span && {...span},version=serial;busy=true;
            try {
                const validated=await rpc('command',{root:project,command:command.id});
                if(disposed || version!==serial || project!==root() || draft!==input.value)return;
                close();
                if(querySpan){input.focus();input.setSelectionRange(querySpan.from,querySpan.to);if(!document.execCommand('insertText',false,''))input.setRangeText('',querySpan.from,querySpan.to,'end');changed();}
                dispatch(validated.id);
            }catch(error){hint(error.message);}finally{busy=false;}
        }
        function key(event) {
            if(menu.hidden || event.isComposing)return false;
            if(event.key==='Tab' || event.key==='Escape' || (event.key==='Enter' && event.shiftKey)){close();if(event.key==='Escape'){event.preventDefault();input.focus();}return event.key==='Escape';}
            if(['ArrowDown','ArrowUp','Home','End'].includes(event.key)){event.preventDefault();select(move(active,results.length,event.key));return true;}
            if(event.key==='Enter'){event.preventDefault();invoke(results[active]);return true;}return false;
        }
        const filterKey=event=>{if(event.target.id==='chat-action-search')key(event);};document.addEventListener('keydown',filterKey);
        return {menu,close,key,open,publish,
            ui(action,value){if(action==='filter')render(value);if(action==='select')select(Number(value));if(action==='choose')invoke(results[Number(value)]);},
            async load(){const version=++serial;catalog=[];close();if(!root())return;try{const value=await rpc('commands',{root:root()});if(version===serial && !disposed)catalog=value;}catch(error){hint(error.message);}},
            update(event){if(event.isComposing)return;const next=slashQuery(input.value,input.selectionStart);if(mode==='slash'){if(next){span=next;render(next.query);}else close();}else if(event.inputType==='insertText' && event.data==='/' && next)open('slash',next);},
            dispose(){disposed=true;serial++;close();document.removeEventListener('keydown',filterKey);}
        };
    }
    const api={slashQuery,search,move,panel};
    if(typeof module!=='undefined') module.exports=api;
    scope.dadoCommands=api;
})(typeof window==='undefined' ? globalThis : window);
