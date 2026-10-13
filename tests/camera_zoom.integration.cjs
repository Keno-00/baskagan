const assert=require('node:assert/strict'),{nativeApp}=require('./native_app.cjs');
(async()=>{const app=await nativeApp({openPanel:false}),p=app.page;
 try{
  const file=p.locator('#scene-nodes g[aria-label="main.rs"] .node-dot'),root=p.locator('#scene-nodes g').first().locator('.node-dot');await file.waitFor();await p.waitForTimeout(1900);
  const center=async node=>{const b=await node.boundingBox();return {x:b.x+b.width/2,y:b.y+b.height/2};};
  const before=await center(file),origin=await center(root),distance=Math.hypot(before.x-origin.x,before.y-origin.y);
  await p.mouse.move(before.x,before.y);await p.waitForTimeout(100);
  const label=p.locator('#scene-nodes g[aria-label="main.rs"] text');const textBefore=await label.boundingBox();
  await p.mouse.move(before.x,before.y);await p.keyboard.down('Control');await p.mouse.wheel(0,-120);await p.keyboard.up('Control');await p.waitForTimeout(250);
  const after=await center(file),rootAfter=await center(root);assert.ok(Math.hypot(after.x-before.x,after.y-before.y)<1,'The point under the cursor stays anchored');assert.ok(Math.hypot(after.x-rootAfter.x,after.y-rootAfter.y)>distance*1.15,'Ctrl-scroll up zooms in');assert.equal(await p.locator('#scene-nodes g[aria-label="project"]').count(),1,'Ctrl-scroll does not enter a folder');
  const textAfter=await label.boundingBox();assert.ok(textAfter.width>textBefore.width*1.15,'Node text scales proportionally with zoom');await p.screenshot({path:'target/zoom-labels.png'});
  await p.keyboard.down('Control');await p.mouse.wheel(0,120);await p.keyboard.up('Control');await p.waitForTimeout(250);const restored=await center(root);assert.ok(Math.hypot(restored.x-origin.x,restored.y-origin.y)<1,'Ctrl-scroll down reverses the zoom at the same cursor');
  assert.ok(Math.abs((await label.boundingBox()).width-textBefore.width)<1,'Text size returns with the camera zoom');
  assert.deepEqual(app.errors,[]);console.log('Native Ctrl-scroll cursor anchoring, proportional text scaling, both zoom directions and hierarchy preservation passed. Fixture:',app.fixture);
 }finally{await app.close();}
})().catch(e=>{console.error(e);process.exitCode=1;});
