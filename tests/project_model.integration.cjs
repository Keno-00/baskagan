const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {nativeApp}=require('./native_app.cjs');
(async()=>{
 const app=await nativeApp({setup(project){
  fs.mkdirSync(path.join(project,'.dado'),{recursive:true});
  const source=(file,start=null,end=null)=>({path:file,kind:file==='.'?'folder':'file',symbol:null,start,end});
  fs.writeFileSync(path.join(project,'.dado','systems.json'),JSON.stringify({version:1,concepts:[
   {id:'project',name:'Sample project',description:'A small application that greets the user.',parent:null,sources:[source('.')]},
   {id:'greeting',name:'Hand detection and camera processing',description:'Create a greeting and send it to the output.',parent:'project',sources:[source('main.rs')]},
   {id:'output',name:'Output',description:'Display the greeting.',parent:'project',sources:[source('README.md')]},
   {id:'print',name:'Print greeting',description:'Print the greeting from the main function.',parent:'greeting',sources:[source('main.rs',2,2)]}
  ],connections:[{from:'greeting',to:'output',kind:'data',description:'The greeting is sent to output.',uncertain:false}],revisions:{}}));
 }});
 const {page,project,errors}=app;
 try{
  await page.waitForTimeout(1800);
  await page.evaluate(()=>{window.originalSystemsScene=document.querySelector('#folder-scene');window.originalBackground=document.querySelector('#systems-atmosphere');});
  const folderFile=page.locator('#scene-nodes [aria-label="main.rs"]');await folderFile.click();await page.keyboard.press('Control++');await page.waitForTimeout(350);const folderCamera=await folderFile.getAttribute('transform');
  await page.getByRole('button',{name:'Mental model',exact:true}).click();
  await page.waitForTimeout(500);
  await page.evaluate(()=>{
    window.acceptedModel=JSON.parse(JSON.stringify(dadoProjectModel.model));
    const draft=JSON.parse(JSON.stringify(acceptedModel));
    draft.concepts.push({id:'forming',name:'Camera capture',description:'Still being understood.',parent:'project',sources:[]});
    draft.connections.push({from:'greeting',to:'forming',kind:'data',description:'Frame input',uncertain:false});
    dadoScene.setConceptGraph(draft,'','mental',false,acceptedModel.concepts.map(c=>c.id),['project','forming']);
  });
  const forming=page.locator('[data-concept-anchor="forming"]');
  await forming.waitFor();
  assert.equal(await forming.getAttribute('aria-disabled'),'true');
  assert.ok(Number(await page.locator('[data-concept-anchor="project"] .node-dot').getAttribute('r'))<7,'Previously known concepts respond as they are re-understood');
  const firstRadius=Number(await forming.locator('.node-dot').getAttribute('r'));
  const lineLength=()=>page.locator('#scene-edges .relationship').last().evaluate(e=>Math.hypot(+e.getAttribute('x2')-e.getAttribute('x1'),+e.getAttribute('y2')-e.getAttribute('y1')));
  const firstLength=await lineLength();
  await page.waitForTimeout(180);
  const nextRadius=Number(await forming.locator('.node-dot').getAttribute('r'));
  assert.ok(nextRadius>firstRadius,'Streamed dots grow in the existing scene');
  await forming.click({force:true});
  assert.equal(await page.evaluate(()=>dadoSidebar.context().concept?.id==='model-forming'),false,'Unvalidated nodes cannot become chat context');
  assert.equal(await page.evaluate(()=>dadoProjectModel.model.concepts.some(c=>c.id==='forming')),false,'Preview does not publish unvalidated concepts');
  await page.screenshot({path:'target/systems-forming.png'});
  await page.waitForTimeout(750);
  assert.equal(Number(await forming.locator('.node-dot').getAttribute('r')),3.5);
  assert.ok(await lineLength()>firstLength,'New connections draw outward from their source');
  await page.evaluate(()=>dadoScene.setConceptGraph(acceptedModel,'','mental',false));
  await page.waitForTimeout(450);
  assert.equal(await forming.count(),0,'Rejected previews roll back');
  await page.evaluate(()=>{
    document.documentElement.dataset.reducedMotion='true';
    dadoScene.setConceptGraph({version:1,concepts:[...acceptedModel.concepts,{id:'forming',name:'Camera capture',description:'Draft',parent:'project',sources:[]}],connections:[]},'','mental',false,acceptedModel.concepts.map(c=>c.id));
  });
  await forming.waitFor();
  assert.equal(Number(await forming.locator('.node-dot').getAttribute('r')),3.5,'Reduced motion shows new nodes immediately');
  await page.evaluate(()=>{delete document.documentElement.dataset.reducedMotion;dadoScene.setConceptGraph({concepts:[],connections:[]},'','mental',false);});
  await page.waitForTimeout(750);
  assert.equal(await page.locator('.concept-node').count(),0,'Failed first analysis clears all provisional nodes');
  await page.evaluate(()=>dadoScene.setConceptGraph(acceptedModel,'','mental',false));
  await page.waitForTimeout(750);
  const greeting=page.locator('.concept-node[aria-label="Hand detection and camera processing"]');await greeting.waitFor();await greeting.click();
  await page.locator('.concept-description').waitFor();
  const inspectorLayout=()=>page.evaluate(()=>{const graph=document.querySelector('#folder-scene').getBoundingClientRect(),panel=document.querySelector('.concept-detail').getBoundingClientRect();return {graph:{x:graph.x,y:graph.y,width:graph.width,height:graph.height},clear:graph.right<=panel.left};});
  assert.equal((await inspectorLayout()).clear,true,'Inspector occupies a separate side lane');
  const graphWithInspector=(await inspectorLayout()).graph;
  await page.getByRole('button',{name:'Close description',exact:true}).click();
  const closedGraph=await page.locator('#folder-scene').boundingBox();for(const key of ['x','y','width','height'])assert.ok(Math.abs(closedGraph[key]-graphWithInspector[key])<.1,'Closing information does not move the graph');
  await greeting.hover();await page.locator('.concept-description').waitFor();
  await page.locator('.concept-description').hover();await page.waitForTimeout(250);
  assert.equal(await page.locator('.concept-description').isVisible(),true,'Hover information stays available while moving into the inspector');
  await greeting.click();
  assert.equal(await page.evaluate(()=>dadoSidebar.context().concept.id),'model-greeting');
  assert.equal(await page.evaluate(()=>{const before=JSON.stringify(dadoSidebar.snapshot().concepts);dadoSidebar.syncModel({...dadoProjectModel,root:'different-project'});return JSON.stringify(dadoSidebar.snapshot().concepts)===before;}),true,'Background context cannot cross project boundaries');
  assert.equal(await page.evaluate(()=>dadoSidebar.context().concept.ancestors[0].name),'Sample project');
  await page.locator('.concept-sources').getByRole('button',{name:'main.rs',exact:true}).click();await page.locator('.cm-content').waitFor();
  await page.getByRole('button',{name:'Explore concept',exact:true}).click();
  await page.locator('.concept-node[aria-label="Print greeting"]').click();
  await page.getByRole('button',{name:'main.rs L2–2',exact:true}).click();
  assert.match(await page.evaluate(()=>dadoWorkbench.agentContext().buffers[0].text),/hello/);
  assert.equal(await page.locator('.cm-selectionBackground').count()>0,true,'Source range selects editor text');
  await page.getByRole('button',{name:'Annotate concept',exact:true}).click();
  await page.getByRole('textbox',{name:'Comment text',exact:true}).fill('Explain this print operation');await page.getByRole('button',{name:'Save',exact:true}).click();
  assert.equal(await page.evaluate(()=>dadoComments.comments[0].conceptId),'model-print');
  await page.getByRole('button',{name:'View comments (1)',exact:true}).first().waitFor();
  await page.getByRole('button',{name:'Add idea to concept',exact:true}).click();await page.getByRole('textbox',{name:'Idea text',exact:true}).fill('Maybe add speech output');await page.getByRole('button',{name:'Save',exact:true}).click();
  assert.equal(await page.evaluate(()=>dadoSidebar.context().concept.notes.length),2);
  await page.locator('#folder-scene').hover();await page.mouse.wheel(0,120);await page.waitForTimeout(800);
  assert.equal(await page.getByRole('button',{name:'System diagram',exact:true}).count(),0,'System diagram is deferred until its design is agreed');
  assert.equal(await page.locator('#folder-scene').isVisible(),true);
  assert.equal(await page.evaluate(()=>originalSystemsScene===document.querySelector('#folder-scene')&&originalBackground===document.querySelector('#systems-atmosphere')),true,'Switches retain the original canvas');
  assert.equal(await page.locator('.concept-graph').count(),0,'All modes use the original scene');
  assert.notEqual(await page.locator('#systems-atmosphere').evaluate(el=>getComputedStyle(el).opacity),'0');
  for(const theme of ['light','dark','system']){await page.evaluate(theme=>dadoChat.updatePreferences({appearance:{theme}}),theme);await page.screenshot({path:`target/systems-${theme}.png`});}
  await page.getByRole('button',{name:'Mental model',exact:true}).click();await greeting.click();
  await page.setViewportSize({width:900,height:650});assert.equal((await inspectorLayout()).clear,true,'Narrow inspector stays outside the graph');await page.screenshot({path:'target/systems-narrow.png'});
  await page.setViewportSize({width:1200,height:800});
  await page.getByRole('button',{name:'Folder hierarchy',exact:true}).click();await page.waitForTimeout(450);assert.equal(await folderFile.getAttribute('transform'),folderCamera,'Folder camera is restored');assert.equal(await folderFile.getAttribute('aria-pressed'),'true','Folder selection is restored');
  await page.getByRole('button',{name:'Show overview',exact:true}).click();
  await page.waitForTimeout(900);
  for(const name of ['Mental model','Folder hierarchy','Mental model','Folder hierarchy']){await page.getByRole('button',{name,exact:true}).click();await page.waitForTimeout(450);assert.equal(await page.locator('#folder-scene').isVisible(),true);assert.ok(await page.locator('#scene-nodes > g:visible').count()>0);}
  assert.ok(await page.locator('#scene-nodes [aria-label="main.rs"]').count()>0,'Folder graph is restored');
  await page.locator('#scene-nodes [aria-label="nested"]').dblclick();await page.waitForTimeout(750);assert.ok(await page.locator('#scene-nodes [aria-label="child.rs"]').count()>0,'Folder navigation still works after switching');
  const total=await page.locator('#scene-nodes > g').count(),labels=await page.locator('#scene-nodes text').count();assert.ok(labels<total,'Overview hides distant labels');
  assert.deepEqual(errors,[]);console.log('Native project model: progressive dots/lines, preview rollback, reduced motion, concept context, descent, source opening, notes, deferred diagram mode, themes and overview labels passed.');
 }catch(e){await page.screenshot({path:'target/systems-failure.png'});throw e;}finally{await app.close();console.log('Disposable fixture:',app.fixture);}
})().catch(e=>{console.error(e);process.exit(1)});

