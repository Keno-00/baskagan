(() => {
    const clamp = (value, low, high) => Math.max(low, Math.min(high, value));
    const ease = t => { t = clamp(t, 0, 1); return t * t * (3 - 2 * t); };
    const mix = (a, b, t) => a + (b - a) * t;
    // Exact critically damped spring: preserves momentum without bounce or frame-rate dependence.
    function springStep(value, velocity, destination, seconds, frequency = 11) {
        const offset = value - destination, impulse = velocity + frequency * offset;
        const decay = Math.exp(-frequency * seconds);
        return {value: destination + (offset + impulse * seconds) * decay,
            velocity: (velocity - frequency * impulse * seconds) * decay};
    }
    function railPoint(point, from, to, scale, progress) {
        const zoom = mix(1, scale, progress);
        return {x: mix(from.x, to.x, progress) + (point.x-from.x)*zoom,
            y: mix(from.y, to.y, progress) + (point.y-from.y)*zoom};
    }
    function zoomAt(camera, point, zoom) {
        zoom = clamp(zoom, .15, 8);
        const ratio = zoom / camera.zoom;
        return {...camera, zoom, panX: point.x-(point.x-camera.panX)*ratio,
            panY: point.y+130-(point.y+130-camera.panY)*ratio};
    }
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
    globalThis.DadoMath = { clamp, ease, mix, springStep, railPoint, zoomAt, project, visible, edgeVisible, pickAnchor };
})();
