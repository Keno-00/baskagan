const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {createHash}=require('node:crypto');
const {nativeApp}=require('./native_app.cjs');
(async()=>{
 const app=await nativeApp({openPanel:false,setup(project){
  const inventory={};for(const file of ['main.rs','README.md','nested/child.rs'])inventory[file]=createHash('sha256').update(fs.readFileSync(path.join(project,file))).digest('hex');
  inventory['.wrangler/cache/generated.json']='old-generated-data';
  fs.mkdirSync(path.join(project,'.dado'));
  fs.writeFileSync(path.join(project,'.dado','systems.json'),JSON.stringify({version:1,analysis_version:1,inventory,concepts:[{id:'project',name:'Sample project',description:'Established purpose',parent:null,sources:[{path:'.',kind:'folder',symbol:null,start:null,end:null}]}],connections:[]}));
 }});
 try {
  const p=app.page;const modelFile=path.join(app.project,'.dado/systems.json');const saved=fs.readFileSync(modelFile,'utf8');
  await p.waitForFunction(()=>window.dadoProjectModel?.model?.concepts.length===1);
  await p.evaluate(()=>dadoChat.updatePreferences({smallModel:'fixture',smallReasoningEffort:'low'}));
  fs.mkdirSync(path.join(app.project,'.wrangler/cache'),{recursive:true});fs.writeFileSync(path.join(app.project,'.wrangler/cache/generated.json'),'changed generated data');
  await p.waitForTimeout(3600);
  assert.equal(await p.locator('.status-activity').getAttribute('data-state'),'idle','Prompt version, generated caches and model choice do not regenerate an unchanged project');
  assert.equal(fs.readFileSync(modelFile,'utf8'),saved);
  await p.getByRole('button',{name:'Mental model',exact:true}).click();await p.locator('[data-concept-anchor="project"]').waitFor();
  assert.equal(await p.getByRole('button',{name:'Refresh project model',exact:true}).count(),0);
  const motion=await p.evaluate(async()=>{
   const host=document.querySelector('.systems-stage'),canvas=document.querySelector('#systems-atmosphere'),ctx=canvas.getContext('2d');
   const stroke=ctx.stroke,initial=host.dataset.activity,reduced=document.documentElement.dataset.reducedMotion;let samples=[];
   ctx.stroke=function(...args){samples.push(this.globalAlpha);return stroke.apply(this,args);};
   try{
    await new Promise(r=>setTimeout(r,200));const idle=Math.max(...samples);samples=[];
    host.dataset.activity='working';await new Promise(r=>setTimeout(r,1800));const working=Math.max(...samples);
    document.documentElement.dataset.reducedMotion='true';await new Promise(r=>setTimeout(r,200));const stopped=samples.length;await new Promise(r=>setTimeout(r,200));
    return {idle,working,reducedStops:samples.length===stopped};
   }finally{ctx.stroke=stroke;host.dataset.activity=initial;document.documentElement.dataset.reducedMotion=reduced;}
  });
  assert.ok(motion.working>motion.idle*1.1,'Shared background gently responds to work activity');assert.equal(motion.reducedStops,true,'Activity animation respects reduced motion');
  fs.writeFileSync(path.join(app.project,'main.rs'),'fn main() { println!("changed"); }\n');
  await p.waitForFunction(()=>document.querySelector('.status-activity').dataset.state==='error');
  assert.equal(await p.locator('.systems-stage').getAttribute('data-activity'),'error','Dioxus drives the shared background activity state');
  assert.equal(fs.readFileSync(modelFile,'utf8'),saved,'A failed update leaves saved understanding intact');
  assert.equal(await p.evaluate(()=>dadoProjectModel.model.concepts[0].name),'Sample project');
  const left=await p.locator('.status-context').boundingBox(),right=await p.locator('.status-activity').boundingBox();assert.ok(right.x>=left.x+left.width,'Activity is reserved on the right');
  await p.evaluate(()=>dadoSidebar.ui('settings','general'));
  const rebuild=p.getByRole('button',{name:'Rebuild project understanding',exact:true});await rebuild.waitFor();await rebuild.focus();
  await p.waitForFunction(()=>document.querySelector('.status-context').textContent.startsWith('Warning:'));
  await p.screenshot({path:'target/understanding-general-light.png'});
  await rebuild.click();await p.getByRole('progressbar',{name:'Project understanding in progress'}).waitFor();assert.equal(await rebuild.isDisabled(),true);await p.screenshot({path:'target/understanding-loading.png'});await p.waitForFunction(()=>document.querySelector('.status-activity').dataset.state==='error');assert.equal(fs.readFileSync(modelFile,'utf8'),saved,'A failed explicit rebuild retains the old graph');
  await p.evaluate(()=>dadoChat.updatePreferences({appearance:{theme:'dark'}}));await p.waitForFunction(()=>document.documentElement.dataset.theme==='dark');await p.screenshot({path:'target/understanding-general-dark.png'});
  await p.setViewportSize({width:760,height:640});await p.screenshot({path:'target/understanding-general-narrow.png'});
  assert.deepEqual(app.errors,[]);
  console.log('Native cached understanding, change detection, failed-update preservation, General-only warning action and right-side status passed. Fixture:',app.fixture);
 }finally{await app.close();}
})().catch(e=>{console.error(e);process.exitCode=1});
