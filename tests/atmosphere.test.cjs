const assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
let now=0,raf=null,appearance={},mediaReduced=false;
const document=new EventTarget();document.documentElement={dataset:{theme:'dark'},};document.hidden=false;
const context=new Proxy({}, {get:(o,k)=>o[k]||(()=>{}),set:(o,k,v)=>(o[k]=v,true)});
const host={dataset:{mode:'project'},getBoundingClientRect:()=>({x:0,y:0,left:0,top:0,right:1000,bottom:700,width:1000,height:700}),querySelector:()=>null,querySelectorAll:()=>[]};
const canvas={parentElement:host,getContext:()=>context,isConnected:true};document.getElementById=()=>canvas;
const media=new EventTarget();Object.defineProperty(media,'matches',{get:()=>mediaReduced});
class Observer{observe(){}disconnect(){}}
class Point{constructor(x,y){this.x=x;this.y=y;}matrixTransform(){return this;}}
const window={dadoChat:{preferences:()=>({appearance})}};
vm.runInNewContext(fs.readFileSync('src/atmosphere.js','utf8'),{window,document,performance:{now:()=>now},requestAnimationFrame:fn=>(raf=fn,1),cancelAnimationFrame:()=>raf=null,matchMedia:()=>media,AbortController,ResizeObserver:Observer,MutationObserver:Observer,DOMPoint:Point,devicePixelRatio:1,getComputedStyle:()=>({getPropertyValue:()=> '#888'})});
const api=window.dadoAtmosphere,svg={getScreenCTM:()=>({inverse(){return this;}})};
function tick(seconds){for(let i=0;i<seconds*60;i++){now+=1000/60;const fn=raf;raf=null;fn?.(now);}}
function prefs(patch){appearance={...appearance,...patch};const event=new Event('dado-appearance-change');event.detail=appearance;document.dispatchEvent(event);}
tick(3);const first=api.snapshot();tick(1);assert.notDeepEqual(api.snapshot().points,first.points,'Idle particles move');
api.setScene(svg,[{path:'main.rs',x:500,y:300}],{panX:0,panY:0,yaw:0,zoom:1});
api.setScene(svg,[{path:'main.rs',x:500,y:300}],{panX:30,panY:0,yaw:.1,zoom:1});tick(.1);let speed=api.snapshot().flow.x;assert.ok(speed>0);tick(.5);assert.ok(api.snapshot().flow.x>0 && api.snapshot().flow.x<speed,'Navigation momentum decays instead of snapping');
prefs({});api.setScene(svg,[{path:'main.rs',x:500,y:300}],{panX:30,panY:0,yaw:2,zoom:1});tick(.1);const turn=api.snapshot().flow;assert.ok(turn.turn>0 && turn.turn<=18);assert.equal(turn.y,0);assert.equal(turn.spin,undefined,'Turning never applies a rotational force to the background');tick(.5);assert.ok(api.snapshot().flow.turn<turn.turn,'Small lateral turning response settles');
prefs({backgroundMotion:false});tick(.1);const frozen=api.snapshot();tick(2);assert.deepEqual(api.snapshot().points,frozen.points,'Disabled background stays still');
prefs({backgroundMotion:true,motionIntensity:'lively'});tick(70);let state=api.snapshot();assert.ok(state.births>10 && state.deaths>10,'Particles spawn and decay');assert.ok(state.count>=44 && state.count<=64);
for(let i=0;i<30;i++){api.activity(['main.rs'],'read','completed');tick(1);assert.ok(api.snapshot().count<=80,'Read activity respects population cap');}
mediaReduced=true;prefs({respectReducedMotion:true});tick(.1);assert.equal(api.snapshot().moving,false);const reduced=api.snapshot().points;tick(1);assert.deepEqual(api.snapshot().points,reduced);
prefs({respectReducedMotion:false});tick(.1);assert.equal(api.snapshot().moving,true);
prefs({agentMotion:false});api.activity(['main.rs'],'read','completed');tick(3);assert.equal(api.snapshot().targets,0);
document.hidden=true;tick(.1);assert.equal(raf,null,'Hidden app stops scheduling frames');
api.dispose();console.log('Atmosphere lifetime bounds, inertia, disabled motion, reduced motion and hidden-app suspension passed.');
