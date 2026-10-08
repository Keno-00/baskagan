(() => {
    window.dadoScene?.dispose();
    const svg = document.getElementById('folder-scene');
    if (!svg) return;
    const edgesLayer = document.getElementById('scene-edges');
    const nodesLayer = document.getElementById('scene-nodes');
    const { clamp, ease, mix, project, visible, edgeVisible, pickAnchor } = DadoMath;
    const NS = 'http://www.w3.org/2000/svg';
    const events = new AbortController();
    const send = message => dioxus.send(message);
    let hintTimer = null;
    const hint = (text, duration = 0) => {
        if (hintTimer !== null) { clearTimeout(hintTimer); hintTimer = null; }
        send({ action: 'hint', text: text || null });
        if (text && duration) hintTimer = setTimeout(() => hint(null), duration);
    };
    const initialCamera = () => ({ yaw: 0, zoom: 1, panX: 0, panY: 0 });
    let camera = initialCamera(), target = initialCamera(), cameraTween = null;
    let scene = null, selected = null, transition = null, screen = new Map();
    let dragging = null, moved = 0, wheel = 0, wheelTime = 0, navigationPending = false, blockedUntil = 0;
    let frame = null, disposed = false, fit = 1, childParents = new Set(), scenePaths = new Set();
    const nodeElements = new Map(), edgeElements = new Map();
    function el(tag, attrs = {}) {
        const element = document.createElementNS(NS, tag);
        for (const [key, value] of Object.entries(attrs)) element.setAttribute(key, value);
        return element;
    }
    function listen(element, event, callback, options = {}) {
        element.addEventListener(event, callback, { ...options, signal: events.signal });
    }
    function schedule() {
        if (frame === null && !disposed) frame = requestAnimationFrame(render);
    }
    function animateCamera(next, duration = 280) {
        target = { ...next };
        // Preserve the in-progress timing while dragging; otherwise every mouse event
        // would restart ease-in and prevent the camera from catching the pointer.
        if (!cameraTween || !dragging) cameraTween = { from: { ...camera }, start: performance.now(), duration };
        schedule();
    }
    function svgPoint(event) {
        const matrix = svg.getScreenCTM();
        if (!matrix) return { x: 0, y: 0 };
        return new DOMPoint(event.clientX, event.clientY).matrixTransform(matrix.inverse());
    }
    function navigate(path) {
        if (!path || navigationPending || performance.now() < blockedUntil || path === scene?.current) return;
        navigationPending = true;
        wheel = 0;
        hint(null);
        send({ action: 'enter', path });
    }
    function select(path) {
        selected = path;
        const node = scene?.tree.nodes.find(n => n.path === path);
        send({ action: 'select', path, directory: node?.directory ?? false });
        hint(`Selected ${node?.directory ? 'folder' : 'file'}: ${path}`, 1400);
        schedule();
    }
    function makeNode(node) {
        const group = el('g', {
            class: node.directory ? 'folder-node' : 'file-node',
            role: 'button',
            tabindex: '0',
            'aria-label': node.label,
        });
        group.append(el('circle', { class: 'hit-area', r: 23 }));
        const dot = el('circle', { class: 'node-dot' });
        const label = el('text', { 'text-anchor': 'middle' });
        label.textContent = node.label;
        group.append(dot, label);
        listen(group, 'mouseenter', () => hint(node.directory
            ? `${node.path} · Click to anchor; double-click to enter`
            : `${node.path} · Click to select; double-click to edit`));
        listen(group, 'mouseleave', () => hint(null));
        listen(group, 'focus', () => hint(node.directory ? `${node.path} · Space to anchor; Enter to enter` : node.path));
        listen(group, 'blur', () => hint(null));
        listen(group, 'click', event => {
            event.stopPropagation();
            if (moved < 5) select(node.path);
        });
        listen(group, 'dblclick', event => {
            event.stopPropagation();
            if (moved < 5) {
                if (node.directory) navigate(node.path);
                else send({ action: 'open_file', path: node.path });
            }
        });
        listen(group, 'keydown', event => {
            if (event.key === 'Enter') {
                event.preventDefault();
                if (node.directory) navigate(node.path);
                else send({ action: 'open_file', path: node.path });
            }
            if (event.key === ' ') { event.preventDefault(); select(node.path); }
        });
        nodesLayer.append(group);
        return { group, dot, label, directory: node.directory };
    }
    function drawEdge(key, a, b, opacity, parent = false) {
        let line = edgeElements.get(key);
        if (!edgeVisible(a, b) || opacity < 0.01) {
            if (line) line.style.display = 'none';
            return;
        }
        if (!line) {
            line = el('line');
            if (parent) {
                line.setAttribute('class', 'parent-connection');
                line.setAttribute('role', 'button');
                line.setAttribute('tabindex', '0');
                line.setAttribute('aria-label', 'Parent folder');
                listen(line, 'mouseenter', () => hint(`${scene?.parent ?? ''} · Scroll down or double-click to go up`));
                listen(line, 'mouseleave', () => hint(null));
                listen(line, 'dblclick', () => navigate(scene?.parent));
                listen(line, 'keydown', event => {
                    if (event.key === 'Enter') { event.preventDefault(); navigate(scene?.parent); }
                });
            }
            edgesLayer.append(line);
            edgeElements.set(key, line);
        }
        line.style.display = '';
        for (const [key, value] of Object.entries({ x1: a.x, y1: a.y, x2: b.x, y2: b.y, opacity })) line.setAttribute(key, value);
    }
    function render(now) {
        frame = null;
        if (disposed || !svg.isConnected) { dispose(); return; }
        if (!scene) return;
        if (cameraTween) {
            const t = clamp((now - cameraTween.start) / cameraTween.duration, 0, 1);
            for (const key of Object.keys(camera)) camera[key] = mix(cameraTween.from[key], target[key], ease(t));
            if (t === 1) cameraTween = null;
        }
        const points = new Map();
        const progress = transition ? clamp((now - transition.start) / transition.duration, 0, 1) : 1;
        const t = ease(progress);
        const rootStart = transition?.anchor ?? { x: 0, y: -130 };
        for (const [i, node] of scene.tree.nodes.entries()) {
            const to = project(node.position, camera, fit);
            if (i === 0) to.opacity = 1;
            let point = to;
            if (transition) {
                let from = transition.previous.get(node.path);
                if (!from) {
                    from = transition.direction === 'up'
                        ? { x: to.x, y: to.y - 400, opacity: 0 }
                        : transition.direction === 'down'
                            ? { x: rootStart.x + to.x * 0.3, y: rootStart.y + 280 + node.position[1] * fit * 0.25, opacity: 0 }
                            : { ...to, opacity: 0 };
                }
                point = { x: mix(from.x, to.x, t), y: mix(from.y, to.y, t), opacity: mix(from.opacity, to.opacity, t) };
            }
            points.set(node.path, point);
        }
        const activeNodes = new Map(scene.tree.nodes.map(n => [n.path, n]));
        if (transition && progress < 1) {
            for (const old of transition.old.tree.nodes) {
                if (activeNodes.has(old.path)) continue;
                const start = transition.previous.get(old.path);
                if (!start) continue;
                points.set(old.path, {
                    x: start.x,
                    y: start.y + (transition.direction === 'up' ? 450 : -450) * t,
                    opacity: start.opacity * (1 - t),
                });
                activeNodes.set(old.path, old);
            }
        }
        for (const edge of edgeElements.values()) edge.style.display = 'none';
        for (const node of scene.tree.nodes) {
            if (node.parent !== null) {
                const parent = scene.tree.nodes[node.parent];
                const a = points.get(parent.path), b = points.get(node.path);
                drawEdge(node.path, a, b, Math.min(a.opacity, b.opacity) * 0.55);
            }
        }
        if (transition && progress < 1) {
            for (const node of transition.old.tree.nodes) {
                if (node.parent === null || scenePaths.has(node.path)) continue;
                const a = points.get(transition.old.tree.nodes[node.parent].path), b = points.get(node.path);
                if (a && b) drawEdge(`old:${node.path}`, a, b, Math.min(a.opacity, b.opacity) * 0.55);
            }
        }
        const root = points.get(scene.current);
        if (scene.parent && root) drawEdge('parent', root, { x: root.x, y: -500 }, root.opacity * 0.4, true);
        for (const node of scene.tree.nodes) {
            if (node.directory && node.has_children && !childParents.has(node.path)) {
                const p = points.get(node.path);
                drawEdge(`tail:${node.path}`, { ...p, y: p.y + 7 }, { x: p.x, y: Math.max(600, p.y + 500) }, p.opacity * 0.32);
            }
        }
        // Cull dots and labels outside the viewport. Their connections are retained
        // when they cross the visible area and fade through the boundary mask.
        for (const [path, item] of nodeElements) if (!activeNodes.has(path) || !visible(points.get(path))) item.group.style.display = 'none';
        const sorted = [...activeNodes.values()].sort((a, b) => points.get(a.path).opacity - points.get(b.path).opacity);
        for (const node of sorted) {
            const p = points.get(node.path);
            if (!visible(p)) continue;
            let item = nodeElements.get(node.path);
            if (!item) { item = makeNode(node); nodeElements.set(node.path, item); }
            const isRoot = node.path === scene.current;
            const isSelected = node.path === selected;
            item.group.style.display = '';
            item.group.setAttribute('transform', `translate(${p.x},${p.y})`);
            item.group.setAttribute('opacity', p.opacity);
            item.group.classList.toggle('selected-node', isSelected);
            item.group.setAttribute('aria-pressed', isSelected ? 'true' : 'false');
            item.dot.setAttribute('r', isRoot ? 7 : isSelected ? 6.5 : node.directory ? 5 : 3.5);
            item.label.setAttribute('y', isRoot ? -19 : 24);
        }
        screen = points;
        if (transition && progress === 1) {
            transition = null;
            for (const [path, item] of nodeElements) if (!activeNodes.has(path)) { item.group.remove(); nodeElements.delete(path); }
            for (const [key, item] of edgeElements) if (key.startsWith('old:')) { item.remove(); edgeElements.delete(key); }
            for (const [key, item] of edgeElements) if (item.style.display === 'none') { item.remove(); edgeElements.delete(key); }
        }
        if (transition || cameraTween) schedule();
    }
    function setGraph(next) {
        if (scene && next.current === scene.current && next.overview === scene.overview
            && JSON.stringify(next.tree) === JSON.stringify(scene.tree)) { navigationPending = false; return; }
        const previous = scene;
        const navigation = previous && next.current !== previous.current;
        const separator = next.current.includes('\\') ? '\\' : '/';
        const direction = navigation ? (next.current.startsWith(previous.current + separator) ? 'down' : 'up') : 'same';
        const anchor = screen.get(next.current) ?? { x: 0, y: -130 };
        transition = previous ? { old: previous, previous: new Map(screen), anchor, direction, start: performance.now(), duration: navigation ? 650 : 320 } : null;
        scene = next;
        fit = Math.min(1, 420 / Math.max(420, ...next.tree.nodes.map(n => n.position[1])));
        scenePaths = new Set(next.tree.nodes.map(n => n.path));
        childParents = new Set(next.tree.nodes.filter(n => n.parent !== null).map(n => next.tree.nodes[n.parent].path));
        for (const node of next.tree.nodes) {
            const item = nodeElements.get(node.path);
            if (item && item.directory !== node.directory) { item.group.remove(); nodeElements.delete(node.path); }
        }
        navigationPending = false;
        if (navigation) {
            blockedUntil = performance.now() + 650;
            selected = direction === 'up' ? previous.current : null;
            camera = { ...camera, zoom: 1, panX: 0, panY: 0 };
            target = { ...camera };
            cameraTween = null;
        }
        if (selected && !next.tree.nodes.some(n => n.path === selected)) selected = null;
        schedule();
    }
    listen(svg, 'pointerdown', event => {
        if (event.button !== 0) return;
        const p = svgPoint(event);
        dragging = { x: p.x, y: p.y, pan: event.shiftKey, id: event.pointerId };
        moved = 0;
        if (event.target === svg) svg.focus({ preventScroll: true });
    });
    listen(svg, 'pointermove', event => {
        if (!dragging) return;
        const p = svgPoint(event), dx = p.x - dragging.x, dy = p.y - dragging.y;
        moved += Math.abs(dx) + Math.abs(dy);
        if (moved >= 5 && !svg.hasPointerCapture(event.pointerId)) svg.setPointerCapture(event.pointerId);
        const next = { ...target };
        if (dragging.pan) { next.panX += dx; next.panY += dy; }
        else next.yaw += dx * 0.007;
        animateCamera(next, 180);
        dragging.x = p.x; dragging.y = p.y;
    });
    function endDrag(event) {
        if (!dragging) return;
        if (svg.hasPointerCapture(event.pointerId)) svg.releasePointerCapture(event.pointerId);
        dragging = null;
    }
    listen(svg, 'pointerup', endDrag);
    listen(svg, 'pointercancel', endDrag);
    listen(svg, 'click', event => {
        if (event.target === svg && moved < 5) { selected = null; send({action: 'select', path: null}); hint(null); schedule(); }
    });
    listen(svg, 'wheel', event => {
        event.preventDefault();
        if (!scene || event.ctrlKey || navigationPending || performance.now() < blockedUntil) return;
        const now = performance.now();
        if (now - wheelTime > 250 || Math.sign(event.deltaY) !== Math.sign(wheel)) wheel = 0;
        wheelTime = now;
        wheel += event.deltaY * (event.deltaMode === 1 ? 24 : event.deltaMode === 2 ? 650 : 1);
        if (Math.abs(wheel) < 90) return;
        if (wheel > 0) {
            if (scene.parent) navigate(scene.parent);
            else { wheel = 0; hint('Project root', 1400); }
        } else {
            const selectedNode = scene.tree.nodes.find(n => n.path === selected && n.directory && n.path !== scene.current);
            const anchor = selectedNode ?? pickAnchor(scene.tree.nodes, selected, svgPoint(event), screen);
            if (anchor) { selected = anchor.path; navigate(anchor.path); }
            else { wheel = 0; hint('No deeper folder at this level', 1400); }
        }
    }, { passive: false });
    listen(document, 'keydown', event => {
        if (!svg.isConnected || !event.ctrlKey || event.altKey || event.target.closest?.('input, textarea, [contenteditable="true"]')) return;
        if (['+', '=', '-', '_'].includes(event.key) || ['NumpadAdd', 'NumpadSubtract'].includes(event.code)) {
            event.preventDefault(); event.stopPropagation();
            const direction = event.key === '-' || event.key === '_' || event.code === 'NumpadSubtract' ? -1 : 1;
            animateCamera({ ...target, zoom: clamp(target.zoom * (direction > 0 ? 1.2 : 1 / 1.2), 0.15, 8) });
            hint(`Scale ${Math.round(target.zoom * 100)}%`, 1400);
        }
    }, { capture: true });
    function dispose() {
        if (disposed) return;
        disposed = true;
        events.abort();
        if (hintTimer !== null) clearTimeout(hintTimer);
        if (frame !== null) cancelAnimationFrame(frame);
        observer.disconnect();
        if (window.dadoScene?.dispose === dispose) delete window.dadoScene;
    }
    const observer = new MutationObserver(() => { if (!svg.isConnected) dispose(); });
    observer.observe(document.getElementById('main'), { childList: true, subtree: true });
    window.dadoScene = {
        setGraph, dispose,
        selectPath: path => { selected = path; schedule(); },
        reset: () => { animateCamera(initialCamera(), 450); hint(null); },
        cancelNavigation: () => { navigationPending = false; wheel = 0; hint('Folder is no longer available', 1400); },
    };
    send({ action: 'ready' });
})();
