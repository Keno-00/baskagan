const assert=require('node:assert/strict');const {nativeApp}=require('./native_app.cjs');
(async()=>{const app=await nativeApp();const {page:p}=app;try{
const text='# Heading\n\nA **bold** and *quiet* reply with `code`.\n\n- First\n- Second\n\n> Quoted text\n\n```rust\nfn main() {}\n```\n\n| Name | Value |\n| --- | --- |\n| One | Two |\n\n[Docs](https://example.com) [Unsafe](javascript:alert(1))\n\n<script>window.markdownInjected=true</script>';
await p.evaluate(text=>dadoChat.setConversation(dadoChat.snapshot().id,{messages:[{id:'md',role:'assistant',text,time:new Date().toISOString()},{id:'plain',role:'user',text:'Keep **my draft** literal.',time:new Date().toISOString()}]}),text);
const md=p.locator('[data-message="md"] .chat-message-text');await md.locator('h1').waitFor();
assert.equal(await md.locator('strong').innerText(),'bold');assert.equal(await md.locator('li').count(),2);assert.equal(await md.locator('pre code').innerText(),'fn main() {}\n');assert.equal(await md.locator('table').count(),1);assert.equal(await md.locator('a').count(),1);assert.equal(await md.locator('script').count(),0);assert.equal(await p.evaluate(()=>window.markdownInjected),undefined);
assert.equal(await p.locator('[data-message="plain"] .chat-message-text').innerText(),'Keep **my draft** literal.');
await md.locator('strong').evaluate(el=>{const r=document.createRange();r.selectNodeContents(el);getSelection().removeAllRanges();getSelection().addRange(r);el.dispatchEvent(new MouseEvent('mouseup',{bubbles:true}))});
await p.getByRole('button',{name:'Comment on selected text'}).click();await p.getByRole('textbox',{name:'Comment text'}).fill('Keep bold');await p.getByRole('button',{name:'Save',exact:true}).click();
assert.equal(await md.locator('.comment-highlight').innerText(),'bold');
assert.equal(await p.evaluate(()=>dadoComments.comments[0].quote),'bold');
await p.getByRole('button',{name:'Change chat layout'}).click();await p.getByRole('button',{name:'Chat larger',exact:true}).click();
await p.screenshot({path:'target/ui-reference/markdown-light.png'});
await p.evaluate(()=>dadoChat.updatePreferences({appearance:{theme:'dark'}}));await p.waitForFunction(()=>document.documentElement.dataset.theme==='dark');await p.screenshot({path:'target/ui-reference/markdown-dark.png'});
assert.deepEqual(app.errors,[]);console.log('Native Markdown formatting, literal HTML, safe links, user text and formatted-text annotations passed.');
}finally{await app.close()}})().catch(e=>{console.error(e);process.exitCode=1});
