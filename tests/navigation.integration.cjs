const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path');
const {nativeApp}=require('./native_app.cjs');
(async()=>{
 const app=await nativeApp({openPanel:false,setup(project){
  fs.mkdirSync(path.join(project,'nested','deep'));fs.writeFileSync(path.join(project,'nested','deep','leaf.rs'),'// detail');
  fs.mkdirSync(path.join(project,'.dado'),{recursive:true});
  const concepts=[['project','Project',null],['hands','Detect hands','project'],['control','Control PC','project'],['camera','Camera capture','hands'],['processing','Processing','hands'],['opencv','OpenCV functions','camera']].map(([id,name,parent])=>({id,name,parent,description:name,sources:[{path:'.',kind:'folder',symbol:null,start:null,end:null}]}));
  fs.writeFileSync(path.join(project,'.dado','systems.json'),JSON.stringify({version:1,concepts,connections:[],revisions:{}}));
 }}),p=app.page;
 const group=name=>p.locator(`#scene-nodes g[aria-label="${name}"]`),dot=name=>group(name).locator('.node-dot');
 const point=async name=>{const b=await dot(name).boundingBox();return {x:b.x+b.width/2,y:b.y+b.height/2};};
 const record=()=>p.evaluate(()=>{window.navigationFrames=[];window.recordNavigation=true;function sample(time){if(!recordNavigation)return;navigationFrames.push({time,nodes:[...document.querySelectorAll('#scene-nodes>g[aria-label]')].filter(g=>g.style.display!=='none').map(g=>{const m=g.transform.baseVal.consolidate()?.matrix;return {name:g.getAttribute('aria-label'),x:m?.e,y:m?.f,opacity:Number(g.getAttribute('opacity'))};})});requestAnimationFrame(sample);}requestAnimationFrame(sample);});
 const finish=()=>p.evaluate(()=>{recordNavigation=false;return navigationFrames;});
 try{
  await dot('nested').waitFor();await p.waitForTimeout(1900);
  await record();await group('nested').dblclick();await dot('child.rs').waitFor();
  await p.waitForTimeout(180);await p.screenshot({path:'target/navigation-travel.png'});
  await p.waitForTimeout(1000);const frames=await finish();
  const paired=frames.map(f=>({root:f.nodes.find(n=>n.name==='project'),child:f.nodes.find(n=>n.name==='nested')})).filter(f=>f.root&&f.child&&f.root.opacity>.05);
  assert.ok(paired.length>8,'The actual desktop renders continuous shared-node travel');
  const first=paired[0],middle=paired.at(-1),length=f=>Math.hypot(f.child.x-f.root.x,f.child.y-f.root.y);
  assert.ok(middle.root.y<first.root.y-70,'Ancestors travel above the selected level');
  assert.ok(length(middle)>length(first)*1.2,'Connections stretch with focus scale rather than translating as a block');
  assert.ok(Math.abs(middle.root.x-first.root.x)>4,'Travel follows the selected branch instead of a vertical slide');
  assert.ok(Math.abs((await group('nested').getAttribute('transform')).match(/translate\([^,]+,([^\)]+)/)[1]-(-130))<1,'The new root settles at the established focus');
  const parentLine=await p.locator('.parent-connection').evaluate(e=>({x1:+e.getAttribute('x1'),y1:+e.getAttribute('y1'),x2:+e.getAttribute('x2'),y2:+e.getAttribute('y2')}));
  assert.ok(Math.abs(parentLine.x2-(first.root.x-first.child.x)*1.65)<1,'The upper connection retains the parent position after travel');
  assert.ok(Math.abs(parentLine.x2-parentLine.x1)>30,'The upper connection does not snap vertically');
  await p.locator('#folder-scene').hover();await p.mouse.wheel(0,120);await p.waitForTimeout(1100);await dot('main.rs').waitFor();
  const root=await point('project'),branch=await point('nested'),distance=Math.hypot(branch.x-root.x,branch.y-root.y);
  await p.locator('#folder-scene').focus();await p.keyboard.down('Space');await p.mouse.move(branch.x,branch.y);await p.mouse.down();
  await p.mouse.move(branch.x-(branch.x-root.x)/distance*180,branch.y-(branch.y-root.y)/distance*180,{steps:12});await p.mouse.up();await p.keyboard.up('Space');
  await dot('child.rs').waitFor();await p.waitForTimeout(1100);
  const center=await point('nested');await p.locator('#folder-scene').focus();await p.keyboard.down('Space');await p.mouse.move(center.x+70,center.y+35);await p.mouse.down();await p.mouse.move(center.x+70,center.y+215,{steps:12});await p.mouse.up();await p.keyboard.up('Space');
  await dot('main.rs').waitFor();await p.waitForTimeout(1100);
  await p.getByRole('button',{name:'Mental model',exact:true}).click();await dot('Detect hands').waitFor();await p.waitForTimeout(750);
  await record();await group('Detect hands').dblclick();await dot('Camera capture').waitFor();await p.waitForTimeout(1000);const mental=await finish();
  const outgoing=mental.flatMap(f=>f.nodes.filter(n=>n.name==='Project'&&n.opacity>.1));assert.ok(outgoing.length>3);assert.ok(outgoing.at(-1).y<outgoing[0].y-70,'Concept IDs descend along their parent relationships');
  assert.ok(await p.locator('.parent-connection').evaluate(e=>Math.abs(+e.getAttribute('x2')- +e.getAttribute('x1')))>30,'Mental-model parent connections retain direction too');
  for(const theme of ['light','dark']){await p.evaluate(theme=>dadoChat.updatePreferences({appearance:{theme}}),theme);await p.screenshot({path:`target/navigation-${theme}.png`});}
  await p.setViewportSize({width:900,height:650});await p.screenshot({path:'target/navigation-narrow.png'});
  await p.evaluate(()=>dadoChat.updatePreferences({appearance:{reducedMotion:true}}));await p.locator('#folder-scene').hover();await p.mouse.wheel(0,120);await dot('Control PC').waitFor();await p.waitForTimeout(100);
  assert.equal(await group('Detect hands').getAttribute('transform').then(t=>Number(t.match(/,([^\)]+)/)[1]))>0,true,'Reduced motion reaches the destination directly');
  assert.deepEqual(app.errors,[]);console.log('Native navigation passed: connected spring travel, folder/concept descent, rail panning both ways, themes, narrow layout and reduced motion.');
 }finally{await p.evaluate(()=>{window.recordNavigation=false;}).catch(()=>{});await app.close();console.log('Disposable fixture:',app.fixture);}
})().catch(e=>{console.error(e);process.exitCode=1;});
