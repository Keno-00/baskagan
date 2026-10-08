const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const context = {};
vm.runInNewContext(fs.readFileSync('src/scene_math.js', 'utf8'), context);
const { ease, project, visible, edgeVisible, pickAnchor } = context.DadoMath;

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
