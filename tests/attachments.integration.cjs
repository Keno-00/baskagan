const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {nativeApp}=require('./native_app.cjs');
(async()=>{const app=await nativeApp();const {page,project,errors}=app;try{
 const input=page.getByRole('textbox',{name:'Message',exact:true});
 const form=page.locator('.chat-composer');const before=await form.boundingBox();
 await input.fill('Keep this draft');await input.evaluate(el=>{el.setSelectionRange(5,9);window.originalDraft=el;});
 const image=path.join(project,'preview.png');fs.writeFileSync(image,Buffer.from(fs.readFileSync('assets/dioxus-avatar.txt','utf8').split(',')[1],'base64'));
 await page.evaluate(files=>files.forEach(file=>dadoChat.attachSystems(file)),[
  {path:path.join(project,'main.rs'),name:'main.rs',kind:'file'},
  {path:path.join(project,'nested'),name:'nested',kind:'folder'},
  {path:image,name:'preview.png',kind:'file'},
 ]);
 await page.locator('.attachment-card').nth(2).waitFor();await page.waitForFunction(()=>document.querySelector('.attachment-preview img')?.naturalWidth>0);
 assert.equal(await input.inputValue(),'Keep this draft');assert.ok(await input.evaluate(el=>el===window.originalDraft));
 const after=await form.boundingBox();assert.ok(after.height>before.height+75,'composer expands for attachment cards');
 assert.equal(await page.locator('.attachment-card[aria-label="Folder: nested"] .attachment-preview svg path').getAttribute('d'),'M3 7h7l2 2h9v11H3zM3 7V4h7l2 3');
 await input.hover();assert.equal(await page.getByRole('button',{name:'Remove nested',exact:true}).evaluate(el=>getComputedStyle(el).opacity),'0');
 const folder=page.locator('.attachment-card[aria-label="Folder: nested"]');await folder.hover();await page.waitForFunction(()=>getComputedStyle(document.querySelector('.attachment-card[aria-label="Folder: nested"] .attachment-remove')).opacity==='1');
 assert.ok((await page.locator('.status-bar').innerText()).includes('nested'));
 await page.screenshot({path:'target/ui-reference/attachments-after-light.png'});
 await page.evaluate(()=>dadoChat.updatePreferences({appearance:{theme:'dark'}}));await input.hover();await page.waitForTimeout(170);await page.screenshot({path:'target/ui-reference/attachments-after-dark.png'});await form.screenshot({path:'target/ui-reference/attachment-cards.png'});
 await folder.focus();assert.equal(await page.getByRole('button',{name:'Remove nested',exact:true}).evaluate(el=>getComputedStyle(el).pointerEvents),'auto');
 await page.getByRole('button',{name:'Remove nested',exact:true}).click();await page.waitForFunction(()=>document.querySelectorAll('.attachment-card').length===2);
 assert.equal(await input.inputValue(),'Keep this draft');
 await page.evaluate(root=>{for(let i=0;i<7;i++)dadoChat.attachSystems({path:root+'/extra-'+i+'.txt',name:'a-long-file-name-'+i+'.txt',kind:'file'});},project);
 await page.waitForFunction(()=>document.querySelectorAll('.attachment-card').length===9);
 assert.ok(await page.locator('.composer-attachments').evaluate(el=>el.scrollWidth>el.clientWidth));
 const boundary=await page.locator('.systems-chat > main').boundingBox();await input.fill('A long draft that should scroll within the composer.\n'.repeat(60));
 assert.ok(await input.evaluate(el=>el.scrollHeight>el.clientHeight));assert.deepEqual(await page.locator('.systems-chat > main').boundingBox(),boundary,'long text does not move the scene boundary');
 await page.evaluate(root=>dadoComments.replace([{id:1,contextId:'idea-fixture',source:'Systems',kind:'idea',folder:root,quote:'An uncertain possibility',text:'Maybe later',from:0,to:0},{id:2,contextId:'comment-fixture',source:'Systems',kind:'comment',folder:root,quote:'Systems note',text:'Review this',from:0,to:0}]),project);
 await page.getByRole('button',{name:'View ideas (1)',exact:true}).first().waitFor();
 for(const [width,height] of [[1200,800],[900,650],[900,500]]){
  await page.setViewportSize({width,height});await page.waitForTimeout(80);
  const box=await form.boundingBox(),send=await page.getByRole('button',{name:'Send message',exact:true}).boundingBox();assert.ok(send.y+send.height<=box.y+box.height+1,'actions stay inside composer');
  await page.screenshot({path:`target/ui-reference/attachments-long-${width}.png`});
 }
 await page.evaluate(()=>dadoChat.updatePreferences({appearance:{theme:'system'}}));await page.emulateMedia({colorScheme:'dark'});await page.waitForFunction(()=>document.documentElement.dataset.theme==='dark');
 await page.evaluate(()=>{for(const item of dadoChat.snapshot().attachments)dadoChat.composer('remove-file',item.path);});await page.waitForFunction(()=>document.querySelector('.composer-attachments').hidden);
 assert.equal(await input.inputValue(),'A long draft that should scroll within the composer.\n'.repeat(60));
 assert.deepEqual(errors,[]);console.log('Native attachment cards passed: previews, folder icons, bounded expansion, overflow, draft retention, remove/focus and themes.');
}finally{await app.close();}})().catch(e=>{console.error(e);process.exit(1)});
