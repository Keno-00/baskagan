const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {nativeApp}=require('./native_app.cjs');
const {execFileSync}=require('node:child_process');
(async()=>{
 const app=await nativeApp();const p=app.page;
 try {
 await p.evaluate(()=>dadoSidebar.openModelSettings());
 await p.getByRole('heading',{name:'Providers',exact:true}).waitFor();
 await p.getByRole('heading',{name:'Defaults',exact:true}).waitFor();
 await p.evaluate(()=>dadoChat.aiEvent({kind:'state',ready:true,account:{email:'test@example.test'},models:[
  {model:'large',displayName:'Large',isDefault:true,defaultReasoningEffort:'high',supportedReasoningEfforts:[{reasoningEffort:'low'},{reasoningEffort:'high'}]},
  {model:'small',displayName:'Small',defaultReasoningEffort:'low',supportedReasoningEfforts:[{reasoningEffort:'low'},{reasoningEffort:'high'}]}
 ]}));
 await p.getByLabel('Default Model',{exact:true}).selectOption('large');
 await p.getByLabel('Default Reasoning',{exact:true}).selectOption('high');
 await p.getByLabel('Default Small Model',{exact:true}).selectOption('small');
 await p.getByLabel('Default Small Reasoning',{exact:true}).selectOption('low');
 await p.evaluate(()=>dadoChat.saveNow());
 const preferences=JSON.parse(fs.readFileSync(path.join(app.fixture,'user/.dado/sde/settings.json')));
 assert.equal(preferences.model,'large');assert.equal(preferences.reasoningEffort,'high');assert.equal(preferences.smallModel,'small');assert.equal(preferences.smallReasoningEffort,'low');
 if(process.env.DADO_UI_CAPTURE)await p.screenshot({path:'target/model-settings-after-light.png'});
 await p.evaluate(()=>dadoChat.updatePreferences({appearance:{theme:'dark'}}));
 if(process.env.DADO_UI_CAPTURE)await p.screenshot({path:'target/model-settings-after-dark.png'});
 await p.getByRole('button',{name:'Close settings',exact:true}).click();
 await p.evaluate(()=>dadoSidebar.newChat());
 const first=await p.evaluate(()=>dadoChat.snapshot().id);
 assert.deepEqual(await p.evaluate(()=>dadoChat.snapshot().modelSettings),{model:'large',reasoningEffort:'high'});
 await p.evaluate(()=>{dadoChat.selectModel('small');document.dispatchEvent(new CustomEvent('dado-settings-change',{detail:{reasoningEffort:'high'}}));});
 await p.evaluate(()=>dadoSidebar.newChat());
 const second=await p.evaluate(()=>dadoChat.snapshot().id);
 assert.deepEqual(await p.evaluate(()=>dadoChat.snapshot().modelSettings),{model:'large',reasoningEffort:'high'});
 await p.evaluate(()=>dadoChat.saveNow());
 const saved=JSON.parse(fs.readFileSync(path.join(app.project,'.dado/state.json')));
 assert.deepEqual(saved.conversations.find(c=>c.id===first).modelSettings,{model:'small',reasoningEffort:'high'});
 assert.equal(saved.conversations.find(c=>c.id===second).modelSettings.model,'large');
 assert.equal(await p.evaluate(()=>dadoChat.preferences().model),'large');
 await p.evaluate(async root=>{const generation=dadoChat.aiState().generation;await dadoChat.setProject(null);await dadoChat.setProject(root,generation);},app.project);
 await p.evaluate(id=>dadoSidebar.ui('select',id),first);
 assert.deepEqual(await p.evaluate(()=>dadoChat.snapshot().modelSettings),{model:'small',reasoningEffort:'high'});
 await p.evaluate(()=>dadoSidebar.openModelSettings());
 for(const theme of ['light','dark','system']){
  await p.evaluate(theme=>dadoChat.updatePreferences({appearance:{theme}}),theme);
  for(const width of [1100,580]){
   await p.setViewportSize({width,height:800});
   assert.equal(await p.locator('.settings-page').evaluate(e=>e.scrollWidth>e.clientWidth),false);
  }
 }
 assert.deepEqual(app.errors,[]);
 console.log('Providers/defaults, independent per-chat model persistence, themes and narrow layout passed. Fixture: '+app.fixture);
 }finally{
  await app.close();
  execFileSync('powershell.exe',['-NoProfile','-Command',`Add-Type -AssemblyName Microsoft.VisualBasic; $target=[IO.Path]::GetFullPath($env:DADO_TEST_CLEANUP_ROOT); $workspace=[IO.Path]::GetFullPath($env:DADO_TEST_TARGET_ROOT); if (!$target.StartsWith($workspace + [IO.Path]::DirectorySeparatorChar)) {throw 'Fixture outside target'}; [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteDirectory($target,'OnlyErrorDialogs','SendToRecycleBin')`],{env:{...process.env,DADO_TEST_CLEANUP_ROOT:app.fixture,DADO_TEST_TARGET_ROOT:path.resolve('target')},windowsHide:true});
 }

})().catch(e=>{console.error(e);process.exitCode=1});
