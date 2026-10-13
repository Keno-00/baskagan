const assert=require('node:assert/strict');
const {nativeApp}=require('./native_app.cjs');
(async()=>{
 const app=await nativeApp();const {page,errors}=app;
 try{
  const seed=()=>page.evaluate(()=>{
   const chat=dadoChat.snapshot();window.historyChat=chat.id;
   dadoChat.setConversation(chat.id,{messages:[
    {id:'history-user',role:'user',text:'Keep the layout compact.',time:new Date().toISOString(),attachments:[{name:'main.rs',path:'main.rs'}]},
    {id:'history-assistant',role:'assistant',text:'Updated the layout while preserving the existing behavior.',time:new Date().toISOString(),tools:[{id:'read',tool:'sde_read_file',status:'completed',output:'Read src/main.rs'},{id:'search',tool:'sde_search',status:'completed',output:'Found two references'}],changes:{count:1,requestId:'history-review'}}
   ]});
  });
  await seed();
  await page.getByRole('button',{name:'Change chat layout'}).click();
  await page.getByRole('button',{name:'Chat larger',exact:true}).click();
  await page.waitForFunction(()=>document.querySelector('.systems-chat').style.gridTemplateRows.includes('3fr'));
  const activity=page.locator('.chat-tool-activity');await activity.waitFor();
  assert.equal(await activity.getAttribute('open'),null,'activity starts collapsed');
  await activity.locator(':scope > summary').click();
  await activity.locator('[data-call="read"] > summary').click();
  assert.equal(await activity.locator('[data-call="read"] pre').innerText(),'Read src/main.rs');
  // An ordinary state update must retain expanded rows and native text selection.
  await page.locator('[data-message="history-assistant"] .chat-message-text').evaluate(el=>{const range=document.createRange();range.selectNodeContents(el);getSelection().removeAllRanges();getSelection().addRange(range);});
  await page.evaluate(()=>{const chat=dadoChat.snapshot();chat.messages[1].tools[1].output='Three references';dadoChat.setConversation(chat.id,{messages:chat.messages});});
  await page.waitForFunction(()=>document.querySelector('[data-call="search"] pre').textContent==='Three references');
  assert.equal(await activity.locator('[data-call="read"]').getAttribute('open'),'');
  assert.equal(await page.evaluate(()=>getSelection().toString()),'Updated the layout while preserving the existing behavior.');
  await page.evaluate(()=>getSelection().removeAllRanges());
  await page.screenshot({path:'target/ui-reference/history-after.png'});
  // This fixture has no agent. Replace its RPC error with a fixed review response.
  await page.evaluate(()=>{
   window.fixtureReview={state:'pending',structuralSaveAvailable:true,changes:[{path:'main.rs',operation:'modify',before:'old\n',after:'new\n',conflict:false}]};
   const reply=dadoChat.reply;dadoChat.reply=message=>reply({...message,error:undefined,result:fixtureReview});
  });
  await page.getByRole('button',{name:'Review proposed changes (1)'}).click();
  const dialog=page.getByRole('region',{name:'Review proposed changes'});await dialog.waitFor();
  assert.equal(await dialog.locator('.diff-add').innerText(),'1\n+ new');
  assert.equal(await page.getByRole('button',{name:'Apply to editor',exact:true}).isEnabled(),true);
  assert.equal(await page.getByRole('dialog').count(),0,'changes expand inline without a popup');
  await page.screenshot({path:'target/ui-reference/history-review.png'});
  await dialog.getByRole('button',{name:'Close change review'}).click();await dialog.waitFor({state:'detached'});
  await page.evaluate(()=>{window.fixtureReview={state:'pending',changes:[{path:'main.rs',operation:'modify',before:'old',after:'new',conflict:true}]};});
  await page.getByRole('button',{name:'Review proposed changes (1)'}).click();
  await dialog.waitFor();assert.equal(await dialog.locator('.review-issue').innerText(),'Review required');
  await dialog.getByRole('button',{name:'Close change review'}).click();
  // Direct apply fetches and checks the proposal without opening a review surface.
  await page.evaluate(()=>{
   fixtureReview={state:'pending',changes:[{path:'main.rs',operation:'modify',before:'old',after:'new',conflict:false}]};
   window.appliedReviews=[];dadoWorkbench.applyProposal=async review=>appliedReviews.push(review);
  });
  await page.getByRole('button',{name:'Apply to editor',exact:true}).click();
  await page.waitForFunction(()=>appliedReviews.length===1);
  await page.waitForFunction(()=>document.querySelector('.chat-agent-actions .chat-compact-action')?.textContent==='Applied');
  assert.equal(await page.getByRole('region',{name:'Review proposed changes'}).count(),0);
  await page.evaluate(()=>{
   const chat=dadoChat.snapshot();chat.messages[1].mode='full';chat.messages[1].changes.status='pending';dadoChat.setConversation(chat.id,{messages:chat.messages});
  });
  await page.waitForFunction(()=>!document.querySelector('.chat-agent-actions .chat-compact-action'));
  assert.equal(await page.getByRole('button',{name:'Apply to editor',exact:true}).count(),0,'full access has no manual apply step');
  // Render live metadata through the same Dioxus bridge; capture UI decisions without executing commands.
  await page.evaluate(()=>{
   window.historyDecisions=[];const original=dadoChat.historyAction;dadoChat.historyAction=(action,id,value)=>{if(action==='approval')historyDecisions.push({action,id,value});else original(action,id,value);};
   dadoHistory.restore({chat:historyChat,follow:true,messages:[{id:'live',role:'assistant',text:'Streaming response',time:new Date().toISOString(),clock:'1:00 PM',status:'streaming',response:'Waiting for approval',approvals:[{approvalId:'approval1',requestId:'run1',command:'cargo check',reason:'Check the change',cwd:'fixture',timeoutSeconds:30,submitted:false}]}]});
  });
  await page.getByRole('button',{name:'Run once',exact:true}).click();
  await page.waitForFunction(()=>historyDecisions.length===1);
  assert.deepEqual(await page.evaluate(()=>historyDecisions[0]),{action:'approval',id:'run1',value:{id:'approval1',decision:'accept'}});
  await page.evaluate(()=>dadoHistory.restore({chat:historyChat,follow:true,messages:[{id:'live',role:'assistant',text:'Streaming response continued',time:new Date().toISOString(),clock:'1:00 PM',status:'completed',response:'',approvals:[]}]}));
  await page.waitForFunction(()=>document.querySelector('[data-message="live"] .chat-message-text').textContent==='Streaming response continued');
  assert.equal(await page.locator('.chat-command-approval').count(),0);
  await seed();
  // User scroll position must survive updates while reading older messages.
  await page.evaluate(()=>{const chat=dadoChat.snapshot();dadoChat.setConversation(chat.id,{messages:Array.from({length:30},(_,i)=>({id:'long-'+i,role:'assistant',text:'Message '+i,time:new Date().toISOString()}))});});
  await page.locator('[data-message="long-29"]').waitFor();
  await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  await page.locator('.chat-history').evaluate(el=>{el.dispatchEvent(new WheelEvent('wheel',{deltaY:-300}));el.scrollTop=0;el.dispatchEvent(new Event('scroll'));});
  const position=await page.locator('.chat-history').evaluate(el=>el.scrollTop);
  await page.evaluate(()=>{const chat=dadoChat.snapshot();chat.messages[29].text+=' updated';dadoChat.historyReady();});
  await page.waitForFunction(()=>document.querySelector('[data-message="long-29"] .chat-message-text').textContent.endsWith('updated'));
  assert.equal(await page.locator('.chat-history').evaluate(el=>el.scrollTop),position);
  assert.deepEqual(errors,[]);console.log('Native history passed: layout, selection, stable activity expansion, reviews/conflicts, approval decisions, streaming and scroll position.');
 }finally{await app.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
