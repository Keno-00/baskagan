const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {nativeApp}=require('./native_app.cjs');
(async()=>{
 const app=await nativeApp();const {page,project,errors}=app;
 try {
  const input=page.getByRole('textbox',{name:'Message',exact:true});
  const mainHeight=await page.locator('.systems-chat > main').evaluate(el=>el.clientHeight);
  await input.fill('Long draft\n'.repeat(80));
  assert.equal(await page.locator('.systems-chat > main').evaluate(el=>el.clientHeight),mainHeight);
  assert.ok(await input.evaluate(el=>el.scrollHeight>el.clientHeight));
  const mask=await page.locator('.chat-history').evaluate(el=>getComputedStyle(el).maskImage);
  assert.ok((mask.match(/rgba\(0, 0, 0, 0\)/g)||[]).length===2,'history fades at both edges');
  await input.fill('Undo ');await input.type('native');await input.press('Control+z');
  assert.equal(await input.inputValue(),'Undo ','native draft undo is retained');
  await input.fill('');
  await page.getByRole('button',{name:'Chat actions',exact:true}).click();
  const filter=page.getByRole('textbox',{name:'Search actions',exact:true});await filter.waitFor({state:'visible'});
  await page.waitForFunction(()=>document.activeElement?.id==='chat-action-search');
  await filter.fill('files');
  await page.getByRole('option',{name:'Add files',exact:true}).waitFor();
  await filter.press('Escape');await filter.waitFor({state:'hidden'});
  await page.waitForFunction(()=>document.activeElement===document.querySelector('.chat-composer textarea'),{},{timeout:5000}).catch(async error=>{console.error(await page.evaluate(()=>({active:document.activeElement?.outerHTML,actions:document.querySelector('#chat-action-menu')?.outerHTML.slice(0,300)})));throw error;});
  assert.ok(await input.evaluate(el=>document.activeElement===el),'Escape returns to draft');
  await input.type('/model');
  await page.getByRole('option',{name:'Models',exact:true}).waitFor();
  assert.equal(await input.inputValue(),'/model');
  await input.press('ArrowDown');await input.press('ArrowUp');await input.press('Escape');
  assert.equal(await input.inputValue(),'/model','dismissal preserves slash text');
  await page.evaluate(()=>{
   dadoModelSelector.catalog([{model:'fixture',displayName:'Fixture',supportedReasoningEfforts:[{reasoningEffort:'low'},{reasoningEffort:'high'}]}]);
   dadoModelSelector.restore({model:'fixture',reasoningEffort:'low'});
  });
  const modelButton=page.getByRole('button',{name:'Select model and reasoning',exact:true});
  await page.waitForFunction(()=>document.querySelector('.model-select-trigger').textContent==='Fixture');
  await page.evaluate(()=>{
   const selectors=['.chat-composer','#model-selector-mount','.chat-history'];
   const geometry=()=>selectors.map(s=>{const r=document.querySelector(s).getBoundingClientRect();return [r.x,r.y,r.width,r.height];}).flat();
   const baseline=geometry();window.pickerShifts=[];
   window.pickerObserver=new MutationObserver(()=>{const current=geometry();if(current.some((v,i)=>Math.abs(v-baseline[i])>.6))window.pickerShifts.push(current);});
   pickerObserver.observe(document.querySelector('#chat-host'),{childList:true,subtree:true,attributes:true});
  });
  for(let i=0;i<3;i++){
   await modelButton.click();await page.getByRole('listbox',{name:'Model selector'}).waitFor();
   await page.getByRole('slider',{name:'Reasoning level'}).waitFor();
   await input.click();
   await page.getByRole('listbox',{name:'Model selector'}).waitFor({state:'hidden'});
  }
  const shifts=await page.evaluate(()=>{pickerObserver.disconnect();return pickerShifts;});
  assert.deepEqual(shifts,[],'model picker must not move the composer or history, including intermediate renders');
  await input.fill('Existing draft');
  await page.evaluate(()=>{const chat=dadoChat.snapshot();dadoChat.setConversation(chat.id,{draft:'Existing draft',attachments:[{path:'fixture.txt',name:'fixture.txt'}],skills:[{id:'fixture-skill',name:'Fixture skill',scope:'project-private',description:'Fixture'}]});});
  await page.locator('.attachment-card[aria-label="File: fixture.txt"]').hover();await page.getByRole('button',{name:'Remove fixture.txt'}).click();
  await page.getByRole('button',{name:'Remove fixture.txt'}).waitFor({state:'detached'});
  assert.equal(await input.inputValue(),'Existing draft');
  await page.getByRole('button',{name:'Remove Fixture skill'}).click();
  await page.getByRole('button',{name:'Remove Fixture skill'}).waitFor({state:'detached'});
  const skillFolder=path.join(project,'.agents','skills','fixture');fs.mkdirSync(skillFolder,{recursive:true});
  fs.writeFileSync(path.join(skillFolder,'SKILL.md'),'---\nname: fixture\ndescription: Local UI fixture\n---\nUse this test skill.\n');
  await page.getByRole('button',{name:'Chat actions',exact:true}).click();
  await page.getByRole('option',{name:'Skills',exact:true}).click();
  const skillFilter=page.getByRole('textbox',{name:'Find skills',exact:true});await skillFilter.waitFor({state:'visible'});
  await page.waitForFunction(()=>document.activeElement?.id==='chat-skill-search');
  // Supply the catalog RPC locally; this native fixture has no agent process.
  await page.evaluate(()=>{for(let id=1;id<=100;id++)dadoChat.reply({id,result:{skills:[{id:'fixture',name:'fixture',description:'Local UI fixture',scope:'repo',path:'fixture/SKILL.md',enabled:true}],errors:[]}});});
  await skillFilter.fill('fixture');await page.getByRole('option',{name:'fixture',exact:true}).waitFor();
  await skillFilter.press('Enter');await page.getByRole('button',{name:'Remove fixture',exact:true}).waitFor();
  const geometry=await page.locator('.chat-attach').evaluate(el=>{const a=el.getBoundingClientRect(),b=el.querySelector('svg').getBoundingClientRect();return [Math.abs(a.x+a.width/2-b.x-b.width/2),Math.abs(a.y+a.height/2-b.y-b.height/2)];});
  assert.ok(geometry.every(delta=>delta<.6),'composer icon is centered');
  await page.getByRole('button',{name:'Chat actions',exact:true}).click();
  await filter.waitFor({state:'visible'});
  await page.screenshot({path:'target/ui-reference/composer-actions.png'});
  assert.deepEqual(errors,[]);console.log('Native composer passed: draft growth, both fades, action focus/search/keys, slash dismissal, attachment/skill removal, skill selection and icon alignment.');
 } finally {await app.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
