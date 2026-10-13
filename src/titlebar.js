(() => {
 const bar=document.getElementById('titlebar');
 const events=new AbortController();let target=null;
 const listen=(el,event,fn,options={})=>el.addEventListener(event,fn,{...options,signal:events.signal});
 async function action(action){
  if(action==='quit')await window.dadoChat?.saveNow();
  if(action==='back'||action==='forward'){window.dadoScene?.goHistory(action==='back'?-1:1);return;}
  if(action.startsWith('edit:')){const command=action.slice(5);target?.focus();if(!window.dadoWorkbench?.editCommand(command,target))document.execCommand(command);return;}
  dioxus.send({action});
 }
 window.dadoTitlebar={
  navigation(back,forward){dioxus.send({action:'navigation',back,forward});},
  sidebar(shown){dioxus.send({action:'sidebar-restore',shown});},
  action,
  menu(name){const trigger=document.getElementById(`title-menu-${name}`);dioxus.send({action:'menu',name,left:trigger?.offsetLeft || 0});},
  dispose(){events.abort();}
 };
 listen(document,'focusin',e=>{if(!bar.contains(e.target)&&!e.target.closest('.about-dialog'))target=e.target;});
 listen(bar,'pointerdown',e=>{if(e.target.closest('button'))e.preventDefault();});
 listen(document,'pointerdown',e=>{if(!bar.contains(e.target))dioxus.send({action:'menu-close'});});
 listen(document,'keydown',e=>{
  const popup=bar.querySelector('[role=menu]');
  if(popup&&['Escape','ArrowDown','ArrowUp','Home','End','Tab'].includes(e.key)){
   if(e.key==='Escape'||e.key==='Tab'){const trigger=bar.querySelector('[aria-expanded=true]');dioxus.send({action:'menu-close'});trigger?.focus();if(e.key==='Tab')return;}
   else{const rows=[...popup.querySelectorAll('[role=menuitem]')];let i=rows.indexOf(document.activeElement);i=e.key==='Home'?0:e.key==='End'?rows.length-1:(i+(e.key==='ArrowDown'?1:-1)+rows.length)%rows.length;rows[i]?.focus();}
   e.preventDefault();return;
  }
  if(e.ctrlKey&&!e.altKey&&['+','=','-','_','0'].includes(e.key)){e.preventDefault();e.stopImmediatePropagation();action(e.key==='0'?'zoom-reset':['-','_'].includes(e.key)?'zoom-out':'zoom-in');}
 },{capture:true});
})();
