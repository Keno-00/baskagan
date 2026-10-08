(() => {
    const clamp = (value, low, high) => Math.max(low, Math.min(high, value));
    const ease = t => { t = clamp(t, 0, 1); return t * t * (3 - 2 * t); };
    const mix = (a, b, t) => a + (b - a) * t;
    function project(position, camera, fit) {
        const [x, y, z] = position;
        const depth = x * Math.sin(camera.yaw) + z * Math.cos(camera.yaw);
        const horizontal = x * Math.cos(camera.yaw) - z * Math.sin(camera.yaw);
        const perspective = clamp(1600 / (1600 + depth * fit), 0.75, 1.25);
        return {
            x: horizontal * fit * camera.zoom * perspective + camera.panX,
            y: y * fit * camera.zoom + camera.panY - 130,
            opacity: clamp(0.82 - depth * fit / 1000, 0.38, 1),
        };
    }
    const visible = p => p.opacity > 0.01 && p.x >= -660 && p.x <= 660 && p.y >= -340 && p.y <= 390;
    const edgeVisible = (a, b) => Math.max(a.x, b.x) >= -620 && Math.min(a.x, b.x) <= 620
        && Math.max(a.y, b.y) >= -320 && Math.min(a.y, b.y) <= 370;
    function pickAnchor(nodes, selected, point, positions) {
        const folders = nodes.filter((n, i) => i !== 0 && n.directory && n.parent === 0);
        const anchor = folders.find(n => n.path === selected);
        if (anchor) return anchor;
        return folders.reduce((best, node) => {
            const p = positions.get(node.path);
            if (!p || !visible(p)) return best;
            const distance = (p.x - point.x) ** 2 + (p.y - point.y) ** 2;
            return !best || distance < best.distance ? { node, distance } : best;
        }, null)?.node ?? null;
    }
    globalThis.DadoMath = { clamp, ease, mix, project, visible, edgeVisible, pickAnchor };
})();
