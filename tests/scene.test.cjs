const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const context = {};
vm.runInNewContext(fs.readFileSync('src/scene_math.js', 'utf8'), context);
const { ease, springStep, railPoint, project, zoomAt, visible, edgeVisible, pickAnchor } = context.DadoMath;

for(const hz of [30,60,144]){
    let value=0,velocity=0,previous=0;
    for(let i=0;i<hz;i++){
        const step=springStep(value,velocity,1,1/hz);value=step.value;velocity=step.velocity;
        assert.ok(value>=previous && value<=1,'The travel spring settles without bouncing');previous=value;
    }
    assert.ok(Math.abs(value-springStep(0,0,1,1).value)<1e-10,'Spring travel is independent of frame rate');
}
const moving=springStep(0,0,1,.15),reversed=springStep(moving.value,moving.velocity,0,.01);
assert.ok(reversed.value>moving.value,'Retargeting preserves momentum before reversing');
const pivot={x:100,y:100},focus={x:0,y:-130};
assert.equal(railPoint(pivot,pivot,focus,1.65,1).y,focus.y);
assert.ok(railPoint({x:100,y:-130},pivot,focus,1.65,1).y<focus.y,'Ancestors travel above the focus');
assert.ok(railPoint({x:100,y:330},pivot,focus,1.65,1).y-focus.y>230,'Descendant spacing expands below the focus');

assert.equal(ease(0), 0);
assert.equal(ease(1), 1);
assert.equal(ease(0.5), 0.5);
assert.ok(ease(0.1) < 0.1, 'ease-in must accelerate from rest');
assert.ok(ease(0.9) > 0.9, 'ease-out must decelerate into its destination');
for (const yaw of [0, Math.PI / 2, Math.PI, 7]) {
    const camera = { yaw, zoom: 1, panX: 0, panY: 0 };
    const root = project([0, 0, 0], camera, 1);
    const child = project([100, 190, 105], camera, 1);
    assert.ok(child.y > root.y, 'rotation must preserve vertical hierarchy');
}
const camera = { yaw: 0, zoom: 1, panX: 0, panY: 0 };
for(const yaw of [0,.7,2])for(const zoom of [.2,1.8,7]){
    const before={yaw,zoom:.9,panX:37,panY:-18},world=[140,230,70],cursor=project(world,before,.8);
    const after=project(world,zoomAt(before,cursor,zoom),.8);
    assert.ok(Math.abs(after.x-cursor.x)<1e-8 && Math.abs(after.y-cursor.y)<1e-8,'Cursor-anchored zoom preserves the pointed world position at any rotation');
}
assert.equal(zoomAt(camera,{x:100,y:100},100).zoom,8);assert.equal(zoomAt(camera,{x:100,y:100},.001).zoom,.15);
assert.ok(project([0, 0, -105], camera, 1).opacity > project([0, 0, 105], camera, 1).opacity);
assert.equal(visible({ x: 0, y: 900, opacity: 1 }), false);
assert.equal(edgeVisible({ x: 0, y: -500 }, { x: 0, y: 500 }), true);
assert.equal(edgeVisible({ x: 900, y: -500 }, { x: 900, y: 500 }), false);
const nodes = [
    { path: '/root', directory: true, parent: null },
    { path: '/root/a', directory: true, parent: 0 },
    { path: '/root/b', directory: true, parent: 0 },
    { path: '/root/file', directory: false, parent: 0 },
    { path: '/root/a/nested', directory: true, parent: 1 },
];
const points = new Map([
    ['/root/a', { x: -100, y: 50, opacity: 1 }],
    ['/root/b', { x: 100, y: 50, opacity: 1 }],
    ['/root/file', { x: 95, y: 50, opacity: 1 }],
    ['/root/a/nested', { x: 98, y: 50, opacity: 1 }],
]);
assert.equal(pickAnchor(nodes, null, { x: 90, y: 50 }, points).path, '/root/b');
assert.equal(pickAnchor(nodes, '/root/a', { x: 90, y: 50 }, points).path, '/root/a');
assert.equal(pickAnchor([nodes[0], nodes[3]], null, { x: 90, y: 50 }, points), null);
console.log('Scene tests passed: easing, hierarchy, depth fading, culling, and anchor selection.');
