(() => {

const bar = document.getElementById('titlebar');

const send = async action => { if (action === 'quit') await window.dadoChat?.saveNow(); dioxus.send({action}); };

const icons = {back:'m14 6-6 6 6 6M8 12h12',forward:'m10 6 6 6-6 6M4 12h12',sidebar:'M4 5h16v14H4zM9 5v14',minimize:'M5 12h14',maximize:'M5 5h14v14H5z',quit:'m6 6 12 12M18 6 6 18'};

function button(label, action, icon) {

 const b = document.createElement('button'); b.type = 'button'; b.setAttribute('aria-label', label); b.dataset.hint = label;

 if(icon) b.innerHTML = `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="${icons[icon]}"/></svg>`; else b.textContent = label;

 b.onclick = action; bar.append(b); return b;

}

const back = button('Back', () => window.dadoScene?.goHistory(-1), 'back');

const forward = button('Forward', () => window.dadoScene?.goHistory(1), 'forward');

back.disabled = forward.disabled = true;

window.dadoTitlebar = {navigation(b, f) {back.disabled = !b; forward.disabled = !f;}};

const sidebar = button('Toggle left sidebar', () => {
 if (document.querySelector('.app-body')?.dataset.project === 'closed') return;

 const panel = document.getElementById('left-sidebar'); panel.hidden = !panel.hidden;

 document.querySelector('.app-body').classList.toggle('sidebar-open', !panel.hidden);

 sidebar.setAttribute('aria-pressed', String(!panel.hidden));
 document.dispatchEvent(new CustomEvent('dado-settings-change', {detail:{leftSidebar:!panel.hidden}}));

}, 'sidebar'); sidebar.setAttribute('aria-pressed','false');

let target = null, popup = null, trigger = null;

document.addEventListener('focusin', e => { if(!bar.contains(e.target)) target = e.target; });

function close() { popup?.remove(); popup = null; trigger?.setAttribute('aria-expanded','false'); dioxus.send({action:'hint',text:null}); }

function edit(command) {

 target?.focus();

 if (!window.dadoWorkbench?.editCommand(command, target)) document.execCommand(command);

}

const menus = {

 File: [['New Project','',() => send('new')],['Open Folder…','',() => send('open')],['Close Project','',() => send('close-project')],['Exit','Alt+F4',() => send('quit')]],

 Edit: [['Undo','Ctrl+Z',() => edit('undo')],['Redo','Ctrl+Y',() => edit('redo')],['Cut','Ctrl+X',() => edit('cut')],['Copy','Ctrl+C',() => edit('copy')],['Paste','Ctrl+V',() => edit('paste')],['Delete','Del',() => edit('delete')],['Select All','Ctrl+A',() => edit('selectAll')]],

 View: [['Zoom In','Ctrl+Shift+=',() => send('zoom-in')],['Zoom Out','Ctrl+-',() => send('zoom-out')],['Actual Size','Ctrl+0',() => send('zoom-reset')],['Toggle Sidebar','',() => sidebar.click()]],

 Help: [['About ᜊᜐ᜔ᜃᜄᜈ᜔','',() => { const dialog = document.createElement('dialog'); dialog.className='about-dialog'; dialog.innerHTML='<h2>ᜊᜐ᜔ᜃᜄᜈ᜔</h2><p>SDE</p><p>Version 0.1.0</p><button>Close</button>'; document.body.append(dialog); dialog.querySelector('button').onclick=()=>dialog.close(); dialog.onclose=()=>dialog.remove(); dialog.showModal(); }]]

};

for(const [label, items] of Object.entries(menus)) {

 const b = button(label, () => {

 const wasOpen = trigger === b && popup; close(); if(wasOpen) return;

 trigger = b; b.setAttribute('aria-expanded','true'); popup = document.createElement('div'); popup.className='title-menu'; popup.setAttribute('role','menu');

 popup.style.left = `${b.offsetLeft}px`;

 for(const [text, shortcut, action] of items) { const item = document.createElement('button'); item.setAttribute('role','menuitem'); item.dataset.hint=text; item.innerHTML=`<span>${text}</span><span>${shortcut}</span>`; item.onclick=()=>{close(); action();}; popup.append(item); }

 bar.append(popup); popup.firstChild.focus();

 }); b.className='title-menu-trigger'; b.setAttribute('aria-haspopup','menu'); b.setAttribute('aria-expanded','false');

 b.onkeydown=e=>{if(e.key==='ArrowDown'){e.preventDefault();b.click();}};

}

const drag = document.createElement('div'); drag.className='title-drag'; drag.onmousedown=e=>{if(e.button===0)send('drag');}; drag.ondblclick=()=>send('maximize'); bar.append(drag);

for(const action of ['minimize','maximize','quit']) button(action==='quit'?'Close window':action==='maximize'?'Maximize or restore':'Minimize',()=>send(action),action).className=`window-control ${action}`;

bar.addEventListener('pointerdown',e=>{if(e.target.closest('button'))e.preventDefault();});

for(const event of ['mouseover','focusin']) bar.addEventListener(event,e=>{const text=e.target.closest('[data-hint]')?.dataset.hint; if(text)dioxus.send({action:'hint',text});});

for(const event of ['mouseleave','focusout']) bar.addEventListener(event,()=>dioxus.send({action:'hint',text:null}));

document.addEventListener('pointerdown',e=>{if(!bar.contains(e.target))close();});

document.addEventListener('keydown',e=>{

 if(popup && ['Escape','ArrowDown','ArrowUp','Home','End','Tab'].includes(e.key)) {

  if(e.key==='Escape'){close();trigger.focus();} else if(e.key==='Tab'){close();trigger.focus();return;} else {const rows=[...popup.children]; let i=rows.indexOf(document.activeElement); i=e.key==='Home'?0:e.key==='End'?rows.length-1:(i+(e.key==='ArrowDown'?1:-1)+rows.length)%rows.length;rows[i].focus();} e.preventDefault(); return;

 }

 if(e.ctrlKey && !e.altKey && ['+','=','-','_','0'].includes(e.key)){e.preventDefault(); e.stopImmediatePropagation(); send(e.key==='0'?'zoom-reset':['-','_'].includes(e.key)?'zoom-out':'zoom-in');}

},true);

})();

