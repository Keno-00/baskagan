(() => {
    window.dadoScene?.dispose();
    const svg = document.getElementById('folder-scene');
    if (!svg) return;
    const edgesLayer = document.getElementById('scene-edges');
    const nodesLayer = document.getElementById('scene-nodes');
    const { clamp, ease, mix, springStep, railPoint, zoomAt, project, visible, edgeVisible, pickAnchor } = DadoMath;
    const NS = 'http://www.w3.org/2000/svg';
    const labelMeasure = document.createElement('canvas').getContext('2d');
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
    let cameraMotion = false, cameraTime = 0;
    let velocity = { yaw: 0, panX: 0, panY: 0 };
    function stopCameraMotion() { cameraMotion = false; velocity = { yaw: 0, panX: 0, panY: 0 }; }
    let mode = 'folders', folderGraph = null, conceptGraphs = {}, modeStates = {};
    const conceptEvent = (action,id) => window.dispatchEvent(new CustomEvent('dado-concept',{detail:{action,id}}));
    const conceptId = path => path?.startsWith('concept:') ? path.slice(8) : null;
    let visits = [], visitIndex = -1, historyTarget = null;
    let selection = new Set(), space = false, hovered = null;
    let scene = null, selected = null, transition = null, screen = new Map();
    const parentPositions=new Map();
    let arrival = null;
    let pinTimer=null, pinned=false;
    let holdTimer=null, held=null, neighborhood=new Set(), holdConsumed=false, holdOriginal=null, holdGraph=null;
    function clearHold(){if(pinTimer!==null)clearTimeout(pinTimer);pinTimer=null;pinned=false;svg.removeAttribute("data-hold-pinned");if(holdOriginal && scene===holdGraph){scene=holdOriginal;scenePaths=new Set(scene.tree.nodes.map(n=>n.path));}holdOriginal=null;holdGraph=null;if(holdTimer!==null)clearTimeout(holdTimer);holdTimer=null;held=null;neighborhood.clear();svg.removeAttribute("data-holding");svg.closest(".systems-stage")?.removeAttribute("data-inspecting");schedule();}
    function inspectNode(path){
        const node=scene?.tree.nodes.find(n=>n.path===path);if(!node || node.forming)return;
        if(node.concept && scene.model){
            const model=scene.model, c=model.concepts.find(c=>c.id===node.concept), related=new Set();
            if(c?.parent)related.add(c.parent);
            for(const n of model.concepts)if(n.parent===node.concept)related.add(n.id);
            for(const e of model.connections)if(e.from===node.concept||e.to===node.concept)related.add(e.from===node.concept?e.to:e.from);
            const missing=model.concepts.filter(c=>related.has(c.id)&&!scene.tree.nodes.some(n=>n.concept===c.id)&&(scene.verified===null||scene.verified.includes(c.id))).slice(0,128);
            if(missing.length){
                holdOriginal=scene;const anchor=screen.get(path)||{x:0,y:0};
                const extra=missing.map((c,i)=>({path:'concept:'+c.id,concept:c.id,label:c.name,directory:model.concepts.some(n=>n.parent===c.id),has_children:model.concepts.some(n=>n.parent===c.id),parent:c.parent===node.concept?scene.tree.nodes.indexOf(node):null,position:node.position,inspectionCamera:{...camera},inspectionPosition:{x:clamp(anchor.x+Math.cos(i*2.399963+Math.PI/2)*(180+Math.floor(i/6)*90),-480,480),y:clamp(anchor.y+Math.sin(i*2.399963+Math.PI/2)*(100+Math.floor(i/6)*75),-220,270),opacity:1}}));
                const present=new Set([...scene.tree.nodes,...extra].map(n=>n.path));
                const connections=[...(scene.connections||[])];
                for(const e of model.connections)if((e.from===node.concept||e.to===node.concept)&&present.has('concept:'+e.from)&&present.has('concept:'+e.to)&&!connections.some(n=>n.from==='concept:'+e.from&&n.to==='concept:'+e.to&&n.kind===e.kind))connections.push({...e,from:'concept:'+e.from,to:'concept:'+e.to});
                if(c?.parent && extra.some(n=>n.concept===c.parent))connections.push({from:path,to:'concept:'+c.parent,kind:'part of',description:'Parent responsibility'});
                scene={...scene,tree:{...scene.tree,nodes:[...scene.tree.nodes,...extra]},connections};holdGraph=scene;scenePaths=present;
            }
        }
        held=path;holdConsumed=true;if(node.concept){conceptEvent('select',node.concept);window.dadoSidebar?.selectConcept('model-'+node.concept);}neighborhood=new Set([path]);
        const neighbors=[];
        for(const n of scene.tree.nodes){const parent=n.parent===null?null:scene.tree.nodes[n.parent]?.path;if(n.path===path && parent){neighborhood.add(parent);neighbors.push({path:parent,kind:"parent"});}else if(parent===path){neighborhood.add(n.path);neighbors.push({path:n.path,kind:"child"});}}
        for(const edge of scene.connections||[])if(edge.from===path||edge.to===path){const other=edge.from===path?edge.to:edge.from;neighborhood.add(other);neighbors.push({path:other,kind:edge.kind});}
        svg.setAttribute("data-holding",path);svg.closest(".systems-stage")?.setAttribute("data-inspecting","true");
        window.dispatchEvent(new CustomEvent("dado-node-hold",{detail:{path,mode,neighbors}}));schedule();
    }
    let dragging = null, moved = 0, wheel = 0, wheelTime = 0, navigationPending = false, blockedUntil = 0;
    let frame = null, disposed = false, fit = 1, childParents = new Set(), scenePaths = new Set();
    const nodeElements = new Map(), edgeElements = new Map(), noteElements = new Map();
    const formingNodes = new Map(), formingEdges = new Map();
    let motionOptions=window.dadoChat?.preferences()?.appearance || {};
    const nodeActivity=new Map();let activityWake=null;
    const reducedMotion = () => (motionOptions.respectReducedMotion!==false && matchMedia('(prefers-reduced-motion: reduce)').matches) || document.documentElement.dataset.reducedMotion === 'true';
    const normalize=p=>String(p||'').replaceAll('\\','/').replace(/^\/\/\?\//,'').replaceAll('/./','/').replace(/\/\.?$/,'').toLowerCase();
    listen(document,'visibilitychange',()=>{if(!document.hidden)schedule();});
    listen(document,'dado-appearance-change',e=>{motionOptions=e.detail;if(motionOptions.agentMotion===false)nodeActivity.clear();schedule();});
    listen(window,'dado-scene-activity',e=>{
        const event=e.detail,root=window.dadoProjectModel?.root;
        if(!scene || !root || normalize(event.root)!==normalize(root) || motionOptions.agentMotion===false)return;
        const paths=(event.paths||[]).slice(0,128).map(p=>normalize(/^(?:[a-z]:|[\\/])/i.test(p)?p:root+'/'+p));
        const ids=new Set((event.nodes||[]).slice(0,2000));
        let matches=scene.tree.nodes.filter(n=>n.concept ? ids.has(n.concept) || scene.model?.concepts.find(c=>c.id===n.concept)?.sources.some(s=>paths.includes(normalize(root+'/'+s.path))) : paths.includes(normalize(n.path)));
        if(!matches.length && paths.length && mode==='folders')matches=scene.tree.nodes.filter(n=>n.directory && paths.some(p=>p.startsWith(normalize(n.path)+'/'))).sort((a,b)=>b.path.length-a.path.length).slice(0,1);
        matches=matches.filter(n=>screen.has(n.path) && visible(screen.get(n.path))).slice(0,24);
        const now=performance.now(),kind=event.action;
        for(const n of matches){if(kind==='stopped')nodeActivity.delete(n.path);else nodeActivity.set(n.path,{kind,start:now,until:now+(event.status==='running'?180000:kind==='read'?2400:1800)});}
        while(nodeActivity.size>64)nodeActivity.delete(nodeActivity.keys().next().value);
        window.dadoAtmosphere?.activity(matches.map(n=>n.path),kind,event.status);schedule();
        if(activityWake!==null)clearTimeout(activityWake);activityWake=setTimeout(schedule,kind==='read'?2450:1850);
    });
    function el(tag, attrs = {}) {
        const element = document.createElementNS(NS, tag);
        for (const [key, value] of Object.entries(attrs)) element.setAttribute(key, value);
        return element;
    }
    function listen(element, event, callback, options = {}) {
        element.addEventListener(event, callback, { ...options, signal: events.signal });
    }
    function schedule() {
        if (frame === null && !disposed && !document.hidden) frame = requestAnimationFrame(render);
    }
    function animateCamera(next, duration = 280) {
        stopCameraMotion();
        target = { ...next };
        if (reducedMotion()) { camera = { ...next }; cameraTween = null; schedule(); return; }
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
        if (scene?.tree.nodes.find(n=>n.path===path)?.forming) return;
        if (!path || navigationPending || performance.now() < blockedUntil || path === scene?.current) return;
        navigationPending = true;
        wheel = 0;
        hint(null);
        if (mode !== 'folders') { conceptEvent('enter',conceptId(path)); return; }
        send({ action: 'enter', path });
    }
    function select(path, event = {}) {
        if (scene?.tree.nodes.find(n=>n.path===path)?.forming) return;
        if (event.ctrlKey || event.metaKey) { if (selection.has(path)) selection.delete(path); else selection.add(path); }
        else if (event.shiftKey) selection.add(path);
        else selection = new Set(path ? [path] : []);
        selected = selection.has(path) ? path : [...selection].at(-1) || null;
        const node = scene?.tree.nodes.find(n => n.path === path);
        if (mode !== 'folders') { conceptEvent('select',conceptId(selected)); if(selected)window.dadoSidebar?.selectConcept('model-'+conceptId(selected)); schedule(); return; }
        send({ action: 'select', path:selected, directory: scene?.tree.nodes.find(n=>n.path===selected)?.directory ?? false });
        hint(`Selected ${node?.directory ? 'folder' : 'file'}: ${path}`, 1400);
        schedule();
    }
    function makeNode(node) {
        const group = el('g', {
            class: node.concept ? 'concept-node folder-node' : node.directory ? 'folder-node' : 'file-node',
            ...(node.concept ? {'data-concept-anchor':node.concept} : {}),
            role: 'button',
            tabindex: '0',
            'aria-label': node.label,
        });
        group.append(el('circle', { class: 'hit-area', r: 23 }));
        const dot = el('circle', { class: 'node-dot' });
        group.append(dot);
        if(node.concept)for(const [kind,x] of [['comment',-36],['idea',12]])group.append(el('rect',{'data-note-kind':kind,x,y:-32,width:24,height:24,fill:'transparent','pointer-events':'none'}));
        listen(group, 'mouseenter', () => { hovered=node.path;if(node.concept && !scene?.tree.nodes.find(n=>n.path===node.path)?.forming)conceptEvent('preview',node.concept);schedule();hint(node.concept ? `${node.label} · Hold 1s to pin relationships · Alt+Enter to explain · Double-click to explore` : node.directory
            ? `${node.path} · Click to anchor; double-click to enter`
            : `${node.path} · Click to select; double-click to edit`);});
        listen(group, 'mouseleave', () => {hovered=null;if(node.concept)conceptEvent('preview',null);schedule();hint(null);});
        listen(group, 'focus', () => {hovered=node.path;if(node.concept && !scene?.tree.nodes.find(n=>n.path===node.path)?.forming)conceptEvent('preview',node.concept);schedule();hint(node.concept ? `${node.label} · Space + drag to pan; Enter to explore` : node.directory ? `${node.path} · Space + drag to pan; Shift + drag to lasso; Enter to enter` : node.path);});
        listen(group, 'blur', () => {hovered=null;if(node.concept)conceptEvent('preview',null);schedule();hint(null);});
        listen(group, 'click', event => {
            event.stopPropagation();
            if(pinned && neighborhood.has(node.path)){event.preventDefault();window.dispatchEvent(new CustomEvent("dado-node-hold",{detail:{relationship:node.path}}));return;}
            if(holdConsumed){event.preventDefault();return;}
            if (moved < 5 && !space) select(node.path, event);
        });
        listen(group, 'dblclick', event => {
            event.stopPropagation();
            if(pinned && neighborhood.has(node.path)){event.preventDefault();window.dispatchEvent(new CustomEvent("dado-node-hold",{detail:{relationship:node.path}}));return;}
            if(holdConsumed){event.preventDefault();return;}
            if (moved < 5) {
                if (node.directory || node.concept) navigate(node.path);
                else send({ action: 'open_file', path: node.path });
            }
        });
        listen(group, 'keydown', event => {
            if(event.key==='Enter' && event.altKey){event.preventDefault();event.stopPropagation();if(!event.repeat){clearHold();inspectNode(node.path);}return;}
            if (event.key === 'Enter' && pinned && neighborhood.has(node.path)) {event.preventDefault();event.stopPropagation();window.dispatchEvent(new CustomEvent("dado-node-hold",{detail:{relationship:node.path}}));return;}
            if (event.key === 'Enter') {
                event.preventDefault();
                if (node.directory || node.concept) navigate(node.path);
                else send({ action: 'open_file', path: node.path });
            }
            if (event.key === ' ' ) event.preventDefault();
        });
        nodesLayer.append(group);
        const lines=node.concept?['']:[node.label.length>32?node.label.slice(0,31)+'…':node.label];
        if(node.concept)for(const word of node.label.split(/\s+/)){const i=lines.length-1;if(lines[i] && (lines[i]+' '+word).length>24)lines.push(word);else lines[i]+=(lines[i]?' ':'')+word;}
        return { group, dot, label:null, directory: node.directory, text:node.label,lines };
    }
    function drawEdge(key, a, b, opacity, parent = false) {
        if (scene.forming && !parent && !key.startsWith('old:') && !key.startsWith('tail:')) {
            if (!formingEdges.has(key)) formingEdges.set(key, performance.now());
        }
        if (formingEdges.has(key) && !reducedMotion()) {
            const t = ease(clamp((performance.now()-formingEdges.get(key))/700,0,1));
            b = {...b,x:mix(a.x,b.x,t),y:mix(a.y,b.y,t)};
            if(t<1)schedule();
        }
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
                listen(line, 'mouseenter', () => hint(`${mode==='folders'?scene?.parent ?? '':'Parent concept'} · Scroll down or double-click to go up`));
                listen(line, 'mouseleave', () => hint(null));
                listen(line, 'dblclick', () => navigate(scene?.parent));
                listen(line, 'keydown', event => {
                    if (event.key === 'Enter') { event.preventDefault(); navigate(scene?.parent); }
                });
            }
            edgesLayer.append(line);
            edgeElements.set(key, line);
        }
        if(parent)line.setAttribute('aria-label',mode==='folders'?'Parent folder':'Parent concept');
        line.style.display = '';
        for (const [key, value] of Object.entries({ x1: a.x, y1: a.y, x2: b.x, y2: b.y, opacity })) line.setAttribute(key, value);
    }
    function render(now) {
        frame = null;
        if (disposed || !svg.isConnected) { dispose(); return; }
        if (!scene) return;
        if (cameraMotion) {
            const dt = clamp(now - cameraTime, 0, 50);
            cameraTime = now;
            if (reducedMotion()) {
                camera = { ...target };
                stopCameraMotion();
            } else {
                // Integrate exponential friction exactly so refresh rate does not change the glide.
                const decay = Math.exp(-dt / 150);
                if (!dragging) for (const key of Object.keys(velocity)) {
                    target[key] += velocity[key] * 150 * (1 - decay);
                    velocity[key] *= decay;
                    if (Math.abs(velocity[key]) < (key === 'yaw' ? 0.00002 : 0.003)) velocity[key] = 0;
                }
                let settled = true;
                for (const key of Object.keys(camera)) {
                    camera[key] = mix(camera[key], target[key], 1 - Math.exp(-dt / 55));
                    if (Math.abs(camera[key] - target[key]) > (key === 'yaw' ? 0.0001 : 0.02)) settled = false;
                }
                if (!dragging && settled && Object.values(velocity).every(v => v === 0)) {
                    camera = { ...target };
                    stopCameraMotion();
                }
            }
        }
        if (cameraTween) {
            const t = clamp((now - cameraTween.start) / cameraTween.duration, 0, 1);
            for (const key of Object.keys(camera)) camera[key] = mix(cameraTween.from[key], target[key], ease(t));
            if (t === 1) cameraTween = null;
        }
        const points = new Map();
        const arrivalProgress = arrival && !reducedMotion() && motionOptions.nodeAnimations!==false ? clamp((now - arrival.start) / 1700, 0, 1) : 1;
        const arrivalEase = arrivalProgress * arrivalProgress * (3 - 2 * arrivalProgress);
        if(transition){
            const step=springStep(transition.progress,transition.velocity,1,clamp(now-transition.time,0,64)/1000,transition.navigation?10:16);
            transition.time=now;transition.progress=step.value;transition.velocity=step.velocity;
            if(reducedMotion() || (Math.abs(1-step.value)<.001 && Math.abs(step.velocity)<.01))transition.progress=1;
        }
        const progress=transition?.progress ?? 1,t=progress;
        if(transition)transition.camera.yaw=camera.yaw;
        const transitionCamera=transition?.camera || camera;
        const cameraPoint=p=>transition?{...p,x:(p.x-transitionCamera.panX)*camera.zoom/transitionCamera.zoom+camera.panX,y:(p.y+130-transitionCamera.panY)*camera.zoom/transitionCamera.zoom+camera.panY-130}:p;
        for (const [i, node] of scene.tree.nodes.entries()) {
            const base=node.inspectionCamera,position=node.inspectionPosition;
            const to = position && base ? {x:(position.x-base.panX)*camera.zoom/base.zoom+camera.panX,y:(position.y+130-base.panY)*camera.zoom/base.zoom+camera.panY-130,opacity:position.opacity} : project(node.position, transitionCamera, fit);
            if (i === 0) to.opacity = 1;
            let point = to;
            if (arrival) {
                const from = arrival.points[i % arrival.points.length];
                point = { x: mix(from.x, to.x, arrivalEase), y: mix(from.y, to.y, arrivalEase), opacity: mix(i < arrival.points.length ? .3 : 0, to.opacity, arrivalEase) };
            }
            if (transition) {
                let from = transition.previous.get(node.path);
                if (!from) {
                    from={x:transition.from.x+(to.x-transition.to.x)/transition.scale,y:transition.from.y+(to.y-transition.to.y)/transition.scale,opacity:0};
                }
                const destination={x:transition.from.x+(to.x-transition.to.x)/transition.scale,y:transition.from.y+(to.y-transition.to.y)/transition.scale};
                const world={x:mix(from.x,destination.x,t),y:mix(from.y,destination.y,t)};
                point=cameraPoint({...railPoint(world,transition.from,transition.to,transition.scale,t),opacity:mix(from.opacity,to.opacity,t)});
            }
            points.set(node.path, point);
        }
        const activeNodes = new Map(scene.tree.nodes.map(n => [n.path, n]));
        window.dadoAtmosphere?.setScene(svg, [...points].map(([path,p])=>({...p,path,held:motionOptions.navigationMotion!==false && neighborhood.has(path)})).filter(visible), camera);
        if (transition && progress < 1) {
            for (const old of transition.old.tree.nodes) {
                if (activeNodes.has(old.path) || old.forming) continue;
                const start = transition.previous.get(old.path);
                if (!start) continue;
                points.set(old.path,cameraPoint({...railPoint(start,transition.from,transition.to,transition.scale,t),opacity:start.opacity*(1-ease(clamp((t-.25)/.75,0,1)))}));
                activeNodes.set(old.path, old);
            }
        }
        for (const edge of edgeElements.values()) edge.style.display = 'none';
        for (const node of scene.tree.nodes) {
            if (mode !== 'workflow' && node.parent !== null) {
                const parent = scene.tree.nodes[node.parent];
                const a = points.get(parent.path), b = points.get(node.path);
                drawEdge(node.path, a, b, Math.min(a.opacity, b.opacity) * (held ? (node.path===held||parent.path===held ? .9 : .06) : .55));
            }
        }
        for(const [i,edge] of (scene.connections || []).entries()) {
            const a=points.get(edge.from),b=points.get(edge.to);
            if(!a || !b)continue;
            const key='relation:'+edge.from+':'+edge.to+':'+edge.kind;drawEdge(key,a,b,Math.min(a.opacity,b.opacity)*(held?(edge.from===held||edge.to===held ? .9 : .06):.45));
            const line=edgeElements.get(key);if(line){line.setAttribute('class','concept-edge relationship');line.setAttribute('marker-end',mode==='workflow'?'url(#workflow-arrow)':'');line.setAttribute('stroke-dasharray',edge.uncertain?'4 5':'');}
        }
        if (transition && progress < 1) {
            for (const node of transition.old.tree.nodes) {
                if (node.parent === null) continue;
                const parent=transition.old.tree.nodes[node.parent],current=scene.tree.nodes.find(n=>n.path===node.path);
                if(current?.parent!==null && current && scene.tree.nodes[current.parent]?.path===parent.path)continue;
                const a = points.get(parent.path), b = points.get(node.path);
                if (a && b) drawEdge(`old:${node.path}`, a, b, Math.min(a.opacity, b.opacity) * (held ? (node.path===held||parent.path===held ? .9 : .06) : .55));
            }
            for(const edge of transition.old.connections || []){
                if(scene.connections?.some(e=>e.from===edge.from&&e.to===edge.to&&e.kind===edge.kind))continue;
                const a=points.get(edge.from),b=points.get(edge.to);
                if(a&&b)drawEdge(`old:relation:${edge.from}:${edge.to}:${edge.kind}`,a,b,Math.min(a.opacity,b.opacity)*.45*(1-t));
            }
        }
        const root = points.get(scene.current);
        if (scene.parent && root && !points.has(scene.parent)) {
            const position=parentPositions.get(`${mode}:${scene.current}`);
            const parent=position?project(position,camera,fit):{x:root.x,y:-500};
            drawEdge('parent',root,parent,root.opacity*.4,true);
        }
        for (const node of scene.tree.nodes) {
            if (node.directory && node.has_children && !childParents.has(node.path)) {
                const p = points.get(node.path);
                drawEdge(`tail:${node.path}`, { ...p, y: p.y + 7 }, { x: p.x, y: Math.max(600, p.y + 500) }, p.opacity * 0.32);
            }
        }
        // Cull dots and labels outside the viewport. Their connections are retained
        // when they cross the visible area and fade through the boundary mask.
        for (const [path, item] of nodeElements) { if (!activeNodes.has(path) && !transition) { item.waveLayer?.remove();item.pulse?.remove(); item.group.remove(); nodeElements.delete(path); } else if (!activeNodes.has(path) || !visible(points.get(path))) { item.group.style.display = 'none';if(item.pulse)item.pulse.style.display='none';if(item.waveLayer)item.waveLayer.style.display='none'; } }
        const priority=n=>n.path===scene.current || selection.has(n.path) || n.path===hovered ? 0 : 1;
        const sorted = [...activeNodes.values()].sort((a, b) => priority(a)-priority(b) || points.get(a.path).opacity - points.get(b.path).opacity);
        const matrix=svg.getScreenCTM();
        const scale=matrix?Math.max(.1,Math.hypot(matrix.a,matrix.b)):1;
        const textStyle=getComputedStyle(svg),baseFont=parseFloat(textStyle.getPropertyValue('--ui-text')) || 12;
        const textZoom=clamp(baseFont*camera.zoom,Math.max(10,baseFont*.8),baseFont*3)/baseFont;
        const fontSize=baseFont*textZoom/scale,fontFamily=textStyle.getPropertyValue('--ui-font').trim() || 'system-ui';
        const labelCells=new Set(), labelBounds=[];
        for (const node of sorted) {
            const p = points.get(node.path);
            if (!visible(p)) continue;
            let item = nodeElements.get(node.path);
            if (!item) { item = makeNode(node); nodeElements.set(node.path, item); }
            if(item.measureFont!==fontFamily){labelMeasure.font=`600 12px ${fontFamily}`;item.labelWidth=Math.max(...item.lines.map(line=>labelMeasure.measureText(line).width))/12;item.measureFont=fontFamily;}
            const isRoot = node.path === scene.current;
            const isSelected = selection.has(node.path);
            item.group.style.display = '';
            item.group.setAttribute('transform', `translate(${p.x},${p.y})`);
            item.group.setAttribute('opacity', p.opacity*(held&&!neighborhood.has(node.path) ? .12 : 1));
            item.group.classList.toggle('selected-node', isSelected);
            item.group.toggleAttribute('data-forming', !!node.forming);
            item.group.setAttribute('aria-disabled', node.forming ? 'true' : 'false');
            item.group.setAttribute('tabindex', node.forming ? '-1' : '0');
            item.group.setAttribute('aria-pressed', isSelected ? 'true' : 'false');
            item.group.querySelector('.hit-area')?.setAttribute('r',Math.max(23,15/scale));
            item.dot.setAttribute('r', isRoot ? 7 : isSelected ? 6.5 : node.directory ? 5 : 3.5);
            let activity=nodeActivity.get(node.path);
            if(activity && now>activity.until){nodeActivity.delete(node.path);activity=null;}
            item.group.toggleAttribute('data-active',!!activity);
            if(activity && motionOptions.nodeAnimations!==false){
                const age=(now-activity.start)/1000,staticMotion=reducedMotion();
                const breathing=staticMotion?1:1+.09*Math.sin(age*2.5)*(1-Math.exp(-age*3));
                item.dot.setAttribute('r',Number(item.dot.getAttribute('r'))*breathing);
                if(!item.halo){item.halo=el('circle',{fill:'none',stroke:'currentColor','stroke-width':1,'pointer-events':'none'});item.group.insertBefore(item.halo,item.dot);}
                const changed=['updated','changed'].includes(activity.kind),wave=staticMotion?0:age/1.8;
                item.halo.setAttribute('r',changed?8+wave*16:10+(staticMotion?0:Math.sin(age*1.7)*1.5));
                item.halo.setAttribute('opacity',changed?Math.max(0,.3*(1-wave)):.22);item.halo.style.display='';
                if(changed && !staticMotion){
                    if(!item.waveLayer){item.waveLayer=el('g',{class:'node-change-wave','pointer-events':'none'});edgesLayer.append(item.waveLayer);}
                    const related=new Set();
                    for(const n of scene.tree.nodes){if(n.parent!==null && scene.tree.nodes[n.parent]?.path===node.path)related.add(n.path);if(n.path===node.path && n.parent!==null)related.add(scene.tree.nodes[n.parent]?.path);}
                    for(const edge of scene.connections||[])if(edge.from===node.path||edge.to===node.path)related.add(edge.from===node.path?edge.to:edge.from);
                    const destinations=[...related].map(path=>points.get(path)).filter(point=>point&&visible(point)).slice(0,12);
                    while(item.waveLayer.children.length<destinations.length)item.waveLayer.append(el('circle',{r:2,fill:'currentColor'}));
                    [...item.waveLayer.children].forEach((dot,i)=>{const to=destinations[i];dot.style.display=to?'':'none';if(to){const t=ease(Math.min(1,wave));dot.setAttribute('cx',mix(p.x,to.x,t));dot.setAttribute('cy',mix(p.y,to.y,t));dot.setAttribute('opacity',.25*Math.sin(t*Math.PI));}});item.waveLayer.style.display='';
                }else if(item.waveLayer)item.waveLayer.style.display='none';
                if(activity.kind==='read' && node.parent!==null && !staticMotion){
                    const from=points.get(scene.tree.nodes[node.parent]?.path);
                    if(from){if(!item.pulse){item.pulse=el('circle',{r:2,fill:'currentColor','pointer-events':'none'});edgesLayer.append(item.pulse);}const t=ease((age%1.2)/1.2);item.pulse.setAttribute('cx',mix(from.x,p.x,t));item.pulse.setAttribute('cy',mix(from.y,p.y,t));item.pulse.setAttribute('opacity',.35*Math.sin(t*Math.PI));item.pulse.style.display='';}
                }else if(item.pulse)item.pulse.style.display='none';
                if(!staticMotion)schedule();
            }else{if(item.halo)item.halo.style.display='none';if(item.pulse)item.pulse.style.display='none';if(item.waveLayer)item.waveLayer.style.display='none';}
            if (arrival) item.dot.setAttribute('r', mix(2, isRoot ? 7 : node.directory ? 5 : 3.5, arrivalEase));
            if (node.arriving && !formingNodes.has(node.path)) formingNodes.set(node.path, now);
            if (formingNodes.has(node.path) && !reducedMotion() && motionOptions.nodeAnimations!==false) {
                const growth = ease(clamp((now-formingNodes.get(node.path))/700,0,1));
                item.dot.setAttribute('r', Number(item.dot.getAttribute('r'))*(node.forming?growth:mix(.65,1,growth)));
                if(growth<1)schedule();
            }
            if(node.forming)item.group.setAttribute('opacity',p.opacity*.65);
            const cell=`${Math.round(p.x/110)}:${Math.round(p.y/40)}`;
            const labelY=isRoot?-19*textZoom/scale-(item.lines.length-1)*fontSize*1.25:24*textZoom/scale;
            let showLabel=neighborhood.has(node.path) || isRoot || isSelected || node.path===hovered || (!scene.overview || (fit*camera.zoom>.65 && node.directory && !labelCells.has(cell)));
            if(showLabel){
                const width=item.labelWidth*fontSize;
                const box={x:p.x-width/2,y:p.y+labelY-fontSize,w:width,h:item.lines.length*fontSize*1.25};
                if(!neighborhood.has(node.path)&&!isRoot&&!isSelected&&node.path!==hovered && labelBounds.some(b=>box.x<b.x+b.w&&box.x+box.w>b.x&&box.y<b.y+b.h&&box.y+box.h>b.y))showLabel=false;
                else labelBounds.push(box);
            }
            if(showLabel){labelCells.add(cell);if(!item.label){item.label=el('text',{'text-anchor':'middle'});if(node.concept){
                    for(const [i,line] of item.lines.entries()){const span=el('tspan',{x:0,dy:i?'1.25em':0});span.textContent=line;item.label.append(span);}
                }else item.label.textContent=item.lines[0];item.group.append(item.label);}item.label.style.display='';item.label.setAttribute('opacity',arrival?clamp((arrivalProgress-.55)/.45,0,1):1);item.label.style.fontSize=fontSize+'px';item.label.setAttribute('y',labelY);}
            else if(item.label){item.label.remove();item.label=null;}
        }
        for (const entry of noteElements.values()) entry.style.display='none';
        for (const kind of ['idea','comment']) {
            const notes=()=> (window.dadoCommentContext?.pending(window.dadoComments?.comments || []) || []).filter(item=>item.source==='Systems' && item.kind===kind && (item.sceneFolder || item.folder)===scene.current);
            const item=notes()[0];if(!item)continue;
            const key=`${kind}:${scene.current}`;
            let group=noteElements.get(key);
            if(!group){
                group=el('g',{'aria-label':kind==='idea'?'New Idea/Thing':'Systems comment'});
                group.append(el('rect',{width:30,height:30,fill:'transparent','pointer-events':'none'}));
                nodesLayer.append(group);noteElements.set(key,group);
            }
            const p=project([item.position?.x || 0,item.position?.y || 0,0],camera,fit);
            group.setAttribute('transform',`translate(${p.x},${p.y})`);group.style.display='';
            window.dadoComments?.anchor(group,`scene-${key}`,notes,kind==='idea'?'ideas-scene':'scene-summary');
        }
        screen = points;
        if (transition && progress === 1) {
            transition = null;
            for (const [path, item] of nodeElements) if (!activeNodes.has(path)) { item.waveLayer?.remove();item.pulse?.remove(); item.group.remove(); nodeElements.delete(path); }
            for (const [key, item] of edgeElements) if (key.startsWith('old:')) { item.remove(); edgeElements.delete(key); }
            for (const [key, item] of edgeElements) if (item.style.display === 'none') { item.remove(); edgeElements.delete(key); }
        }
        if (arrivalProgress === 1) arrival = null;
        if (transition || cameraTween || cameraMotion || arrival) schedule();
    }
    function setGraph(next) {
        if (historyTarget !== null && visits[historyTarget] === next.current) { visitIndex = historyTarget; }
        else if (visits[visitIndex] !== next.current) { visits.splice(visitIndex + 1); visits.push(next.current); visitIndex = visits.length - 1; }
        historyTarget = null;
        window.dadoTitlebar?.navigation(visitIndex > 0, visitIndex < visits.length - 1);
        if (scene && next.mode === scene.mode && next.current === scene.current && next.overview === scene.overview
            && JSON.stringify(next.tree) === JSON.stringify(scene.tree) && JSON.stringify(next.connections) === JSON.stringify(scene.connections)) { navigationPending = false; return; }
        const previous = scene;
        const ongoing=transition && next.mode===previous.mode && next.current===previous.current && next.overview===previous.overview?transition:null;
        if(next.forming && !previous?.forming){formingNodes.clear();formingEdges.clear();}
        if(next.forming && previous?.mode===next.mode){
            for(const n of previous.tree.nodes)if(n.parent!==null && !formingEdges.has(n.path))formingEdges.set(n.path,performance.now()-700);
            for(const e of previous.connections || []){const key='relation:'+e.from+':'+e.to+':'+e.kind;if(!formingEdges.has(key))formingEdges.set(key,performance.now()-700);}
        }
        for(const key of formingNodes.keys())if(!next.tree.nodes.some(n=>n.path===key))formingNodes.delete(key);
        if (!previous) {
            const origins = window.dadoAtmosphere?.takeOrigins(svg, next.tree.nodes.length);
            if (origins?.length && !reducedMotion() && motionOptions.nodeAnimations!==false) {
                arrival = { points: origins, start: performance.now() };
            }
        }
        const navigation = previous && next.tree.nodes.length && previous.tree.nodes.length && next.mode === previous.mode && next.current !== previous.current;
        const direction=navigation?(previous.tree.nodes.some((n,i)=>i>0&&n.path===next.current)?'down':next.tree.nodes.some((n,i)=>i>0&&n.path===previous.current)?'up':'same'):'same';
        const previousPoints=new Map(screen),momentum=Math.hypot(velocity.panX,velocity.panY);
        scene = next;
        fit = Math.min(1, 420 / Math.max(420, ...next.tree.nodes.map(n => n.position[1])));
        scenePaths = new Set(next.tree.nodes.map(n => n.path));
        // Rejected provisional concepts disappear immediately, even during a journey.
        for(const node of previous?.tree.nodes || [])if(node.forming && !scenePaths.has(node.path)){
            const item=nodeElements.get(node.path);if(item){item.waveLayer?.remove();item.pulse?.remove();item.group.remove();nodeElements.delete(node.path);}
        }
        childParents = new Set(next.tree.nodes.filter(n => n.parent !== null).map(n => next.tree.nodes[n.parent].path));
        for (const node of next.tree.nodes) {
            const item = nodeElements.get(node.path);
            if (item && (item.directory !== node.directory || item.text !== node.label)) { item.waveLayer?.remove();item.pulse?.remove(); item.group.remove(); nodeElements.delete(node.path); }
        }
        navigationPending = false;
        if (navigation) {
            blockedUntil = performance.now() + 180;
            selected = direction === 'up' ? previous.current : null;
            stopCameraMotion();
            camera = { ...camera, panX: 0, panY: 0 };
            target = { ...camera };
            cameraTween = null;
        }
        const anchorPath=direction==='up'?previous.current:next.current;
        const destination=next.tree.nodes.find(n=>n.path===anchorPath);
        const from=previousPoints.get(anchorPath) || previousPoints.get(previous?.current) || {x:0,y:-130};
        const to=destination?project(destination.position,camera,fit):{x:0,y:-130};
        if(navigation && previousPoints.has(next.parent)){
            const scale=direction==='down'?1.65:direction==='up'?1/1.65:1;
            const parent=railPoint(previousPoints.get(next.parent),from,to,scale,1);
            const horizontal=(parent.x-camera.panX)/(fit*camera.zoom);
            parentPositions.set(`${mode}:${next.current}`,[horizontal*Math.cos(camera.yaw),(parent.y+130-camera.panY)/(fit*camera.zoom),-horizontal*Math.sin(camera.yaw)]);
        }
        transition=ongoing || (previous && !reducedMotion()?{old:previous,previous:previousPoints,from,to,scale:direction==='down'?1.65:direction==='up'?1/1.65:1,camera:{...camera},direction,navigation,progress:0,velocity:Math.min(2,momentum),time:performance.now()}:null);
        selection = new Set([...selection].filter(path => scenePaths.has(path)));
        if (navigation) selection = new Set(selected ? [selected] : []);
        if (selected && !scenePaths.has(selected)) selected = null;
        schedule();
    }
    listen(svg, 'pointerdown', event => {
        if (event.button !== 0) return;
        stopCameraMotion();
        cameraTween = null;
        target = { ...camera };
        holdConsumed=false;
        const group=event.target.closest?.('g[aria-label]');const path=[...nodeElements].find(([,item])=>item.group===group)?.[0];
        if(pinned && path && neighborhood.has(path))return;
        if(pinned)window.dispatchEvent(new CustomEvent("dado-node-hold",{detail:{close:true}}));
        clearHold();
        if(path && !space && !event.shiftKey && !event.ctrlKey && !event.metaKey)holdTimer=setTimeout(()=>{holdTimer=null;if(dragging && moved<5){inspectNode(path);svg.setPointerCapture(event.pointerId);pinTimer=setTimeout(()=>{pinTimer=null;if(held && dragging){pinned=true;svg.setAttribute("data-hold-pinned","true");hint("Relationships pinned · Click the canvas to release",2000);}},500);}},500);
        const p = svgPoint(event);
        const branch=space?pickAnchor(scene.tree.nodes,selected,p,screen):null;
        dragging = { x: p.x, y: p.y, origin:p, root:screen.get(scene.current),branch:branch?{path:branch.path,point:screen.get(branch.path)}:null, pan: space, lasso: event.shiftKey && !space, points: [p], time: performance.now(), id: event.pointerId };
        moved = 0;
        if (event.target === svg) svg.focus({ preventScroll: true });
    });
    function panRail(p){
        if(!dragging?.pan || dragging.railNavigated || !dragging.root)return null;
        const dx=p.x-dragging.origin.x,dy=p.y-dragging.origin.y;
        const parent=parentPositions.get(`${mode}:${scene.current}`);
        const branch=dy<0?dragging.branch:scene.parent?{path:scene.parent,point:parent?project(parent,camera,fit):{x:dragging.root.x,y:dragging.root.y-190*camera.zoom*fit}}:null;
        if(!branch?.point)return null;
        const x=branch.point.x-dragging.root.x,y=branch.point.y-dragging.root.y,length=Math.hypot(x,y);
        if(length<1)return null;
        const axis={x:x/length,y:y/length},travel=-(dx*axis.x+dy*axis.y),cross=Math.abs(dx*axis.y-dy*axis.x);
        return travel>18 && cross<travel*.65?{path:branch.path,axis,travel,threshold:clamp(length*.6,110,220)}:null;
    }
    function travelPanRail(p,releasing=false){
        const rail=panRail(p);
        if(!rail || navigationPending || performance.now()<blockedUntil)return;
        const coast=releasing?Math.max(0,-velocity.panX*rail.axis.x-velocity.panY*rail.axis.y)*90:0;
        if(rail.travel+coast<rail.threshold)return;
        dragging.railNavigated=true;navigate(rail.path);
    }
    listen(svg, 'pointermove', event => {
        if (!dragging || held) return;
        const p = svgPoint(event), dx = p.x - dragging.x, dy = p.y - dragging.y;
        moved += Math.abs(dx) + Math.abs(dy);
        if(moved>=5 && holdTimer!==null){clearTimeout(holdTimer);holdTimer=null;}
        if (moved >= 5 && !svg.hasPointerCapture(event.pointerId)) svg.setPointerCapture(event.pointerId);
        if(dragging.railNavigated){dragging.x=p.x;dragging.y=p.y;return;}
        if (dragging.lasso) {
            dragging.points.push(p);
            const overlay = document.getElementById('scene-lasso');
            overlay?.setAttribute('points', dragging.points.map(p => `${p.x},${p.y}`).join(' '));
            return;
        }
        const next = { ...target };
        if (dragging.pan) {
            const rail=panRail(p),along=rail?dx*rail.axis.x+dy*rail.axis.y:0;
            next.panX += rail?mix(dx,rail.axis.x*along,.75):dx;
            next.panY += rail?mix(dy,rail.axis.y*along,.75):dy;
        }
        else next.yaw += dx * 0.007;
        const now = performance.now(), dt = clamp(now - dragging.time, 8, 80);
        for (const key of Object.keys(velocity)) {
            const speed = (next[key] - target[key]) / dt;
            velocity[key] = mix(velocity[key], clamp(speed, key === 'yaw' ? -0.012 : -2, key === 'yaw' ? 0.012 : 2), 0.65);
        }
        target = next;
        cameraTween = null;
        if (reducedMotion()) { camera = { ...target }; stopCameraMotion(); }
        else { if (!cameraMotion) cameraTime = now; cameraMotion = true; }
        dragging.time = now;
        schedule();
        dragging.x = p.x; dragging.y = p.y;
        travelPanRail(p);
    });
    function endDrag(event) {
        if(pinTimer!==null)clearTimeout(pinTimer);pinTimer=null;
        if(!pinned || event.type==="pointercancel")clearHold();
        if (!dragging) return;
        if(event.type!=='pointercancel')travelPanRail(svgPoint(event),true);
        if (svg.hasPointerCapture(event.pointerId)) svg.releasePointerCapture(event.pointerId);
        if (event.type!=='pointercancel' && dragging.lasso && moved >= 5) {
            const polygon = dragging.points;
            const inside = p => {
                let hit = false;
                for (let i=0,j=polygon.length-1;i<polygon.length;j=i++) {
                    const a=polygon[i], b=polygon[j];
                    if ((a.y>p.y)!==(b.y>p.y) && p.x<(b.x-a.x)*(p.y-a.y)/(b.y-a.y)+a.x) hit=!hit;
                }
                return hit;
            };
            for (const [path,p] of screen) if (scenePaths.has(path) && visible(p) && inside(p)) selection.add(path);
            selected = [...selection].at(-1) || null;
            if(mode!=='folders'){conceptEvent('select',conceptId(selected));if(selected)window.dadoSidebar?.selectConcept('model-'+conceptId(selected));}
            else send({action:'select',path:selected,directory:scene?.tree.nodes.find(n=>n.path===selected)?.directory});
            schedule();
        }
        document.getElementById('scene-lasso')?.setAttribute('points','');
        if (event.type === 'pointercancel' || dragging.lasso || performance.now() - dragging.time > 90) velocity = { yaw: 0, panX: 0, panY: 0 };
        dragging = null;
    }
    listen(svg, 'pointerup', endDrag);
    listen(svg, 'pointercancel', endDrag);
    listen(svg, 'click', event => {
        if (!holdConsumed && event.target === svg && moved < 5 && !event.shiftKey && !event.ctrlKey && !space) { selection.clear(); selected = null; send({action: 'select', path: null}); if(mode!=='folders')conceptEvent('select',null); hint(null); schedule(); }
    });
    function context(event) {
        event.preventDefault();
        if(mode !== 'folders')return;
        const group = event.target.closest?.('g[aria-label]');
        const path = [...nodeElements].find(([,item])=>item.group===group)?.[0];
        if (path && !selection.has(path)) select(path);
        if (!path) { selection.clear(); selected=null; send({action:'select',path:null});schedule(); }
        const point=svgPoint(event);
        send({action:'context',paths:path?[...selection]:[],x:event.clientX,y:event.clientY,point});
    }
    listen(svg,'contextmenu',context);
    listen(svg,'keydown',event=>{
        if(event.key==='Escape'){clearHold();window.dispatchEvent(new CustomEvent('dado-node-hold',{detail:{close:true}}));}
        if(event.key==='Escape' && mode!=='folders'){selection.clear();selected=null;conceptEvent('select',null);conceptEvent('preview',null);svg.focus({preventScroll:true});schedule();}
        if(event.code==='Space'){event.preventDefault();space=true;}
        if(event.key==='ContextMenu' || (event.key==='F10' && event.shiftKey)) {
            const rect=event.target.getBoundingClientRect();
            context({preventDefault:()=>event.preventDefault(),target:event.target,clientX:rect.x+rect.width/2,clientY:rect.y+rect.height/2});
        }
    });
    listen(document,'keydown',event=>{if(event.code==='Space' && !event.target.closest?.('input,textarea,[contenteditable=true],button'))space=true;});
    let copyChord=false;
    listen(svg,'keydown',event=>{
        if(mode !== 'folders')return;
        let action=null;
        const ctrl=event.ctrlKey || event.metaKey;
        if(ctrl && event.key.toLowerCase()==='k'){copyChord=true;event.preventDefault();return;}
        if(copyChord && ctrl && event.shiftKey && event.key.toLowerCase()==='c')action='relative';
        copyChord=false;
        if(ctrl && !event.altKey && !event.shiftKey)action=({x:'cut',c:'copy',v:'paste'})[event.key.toLowerCase()] || action;
        if(event.key==='F2')action='rename';
        if(event.key==='Delete')action='delete';
        if(event.shiftKey && event.altKey && event.key.toLowerCase()==='c')action='path';
        if(event.shiftKey && event.altKey && event.key.toLowerCase()==='r')action='reveal';
        if(event.shiftKey && (ctrl || event.altKey) && event.key.toLowerCase()==='f')action='find';
        if(!action)return;
        event.preventDefault();event.stopPropagation();
        const rect=svg.getBoundingClientRect();
        send({action:'context',command:action,paths:[...selection],x:rect.left+30,y:rect.top+30,point:{x:0,y:0}});
    });
    listen(window,'keyup',event=>{if(event.code==='Space')space=false;if(event.key==='Enter' && !pinned)clearHold();});
    listen(window,'pointerup',endDrag);listen(window,'blur',()=>{clearHold();dragging=null;stopCameraMotion();});
    listen(window,'dado-insight-close',clearHold);
    listen(window,'blur',()=>{space=false;dragging=null;stopCameraMotion();target={...camera};cameraTween=null;document.getElementById('scene-lasso')?.setAttribute('points','');});
    listen(svg, 'wheel', event => {
        event.preventDefault();
        if (!scene) return;
        if (event.ctrlKey) {
            event.stopPropagation();wheel=0;wheelTime=0;
            const delta=event.deltaY*(event.deltaMode===1?24:event.deltaMode===2?650:1);
            const zoom=target.zoom*Math.exp(-clamp(delta,-240,240)*.0018);
            animateCamera(zoomAt(camera,svgPoint(event),zoom),180);
            hint(`Scale ${Math.round(target.zoom*100)}%`,1400);
            return;
        }
        if (navigationPending || performance.now() < blockedUntil) return;
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
            else { wheel = 0; hint(mode==='folders'?'No deeper folder at this level':'No deeper concept at this level', 1400); }
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
        disposed = true;clearHold();if(activityWake!==null)clearTimeout(activityWake);
        events.abort();
        unsubscribeNotes?.();
        if (hintTimer !== null) clearTimeout(hintTimer);
        if (frame !== null) cancelAnimationFrame(frame);
        observer.disconnect();
        resizeObserver.disconnect();
        appearanceObserver.disconnect();
        window.dadoTitlebar?.navigation(false, false);
        if (window.dadoScene?.dispose === dispose) delete window.dadoScene;
    }
    const observer = new MutationObserver(() => { if (!svg.isConnected) dispose(); });
    observer.observe(document.getElementById('main'), { childList: true, subtree: true });
    const resizeObserver=new ResizeObserver(schedule);resizeObserver.observe(svg);
    const appearanceObserver=new MutationObserver(schedule);appearanceObserver.observe(document.documentElement,{attributes:true});
    window.dadoScene = {
        setGraph, dispose, report: hint,
        focusNode(path){nodeElements.get(path)?.group.focus({preventScroll:true});},
        setFolderGraph(next){folderGraph={...next,mode:'folders'};if(mode==='folders')setGraph(folderGraph);},
        setConceptGraph(model,focus,kind,overview,verified=null,arrived=null){
            if(kind==='folders')return;
            const current=model.concepts.find(n=>n.id===focus) || model.concepts.find(n=>!n.parent);
            if(!current){const empty={mode:kind,current:'concept:empty',parent:null,overview,tree:{nodes:[],notices:[]},connections:[]};conceptGraphs[kind]=empty;if(mode===kind)setGraph(empty);return;}
            const included=new Set([current.id]);
            for(let depth=0;depth<(overview?40:1);depth++){const parents=new Set(included);for(const c of model.concepts)if(parents.has(c.parent))included.add(c.id);}
            const concepts=[current,...model.concepts.filter(n=>n.id!==current.id && included.has(n.id))];
            const nodes=concepts.map(c=>({path:'concept:'+c.id,concept:c.id,arriving:arrived?.includes(c.id) || (arrived===null && verified!==null && !verified.includes(c.id)),forming:verified!==null && !verified.includes(c.id),label:c.name,directory:model.concepts.some(n=>n.parent===c.id),has_children:model.concepts.some(n=>n.parent===c.id),parent:c.id===current.id?null:concepts.findIndex(n=>n.id===c.parent),position:[0,0,0]}));
            const descendants=nodes.map(()=>[]);for(const [i,n] of nodes.entries())if(n.parent!==null)descendants[n.parent].push(i);
            let cursor=0;function layout(i,depth){const children=descendants[i];const xs=children.map(j=>layout(j,depth+1));const x=xs.length?(xs[0]+xs.at(-1))/2:cursor++;const phase=i*2.399963;nodes[i].position=[x,depth*190+(depth?(i%4)*26+Math.sin(phase)*10:0),depth?Math.cos(phase)*105:0];return x;}
            layout(0,0);const center=nodes[0].position[0];for(const n of nodes)n.position[0]=(n.position[0]-center)*Math.min(260,900/Math.max(cursor,1));
            const next={mode:kind,model,verified,forming:verified!==null,current:'concept:'+current.id,parent:current.parent?'concept:'+current.parent:null,overview,tree:{nodes,notices:[]},connections:model.connections.filter(e=>included.has(e.from)&&included.has(e.to)).map(e=>({...e,from:'concept:'+e.from,to:'concept:'+e.to}))};
            if(kind==='workflow' && next.connections.length){
                const connected=new Set(next.connections.flatMap(e=>[e.from,e.to]));
                next.tree.nodes=nodes.filter(n=>connected.has(n.path));
                const rank=new Map(next.tree.nodes.map(n=>[n.path,0]));
                // Cycles stay in a bounded lane instead of forcing an endless topological descent.
                for(let pass=0;pass<next.tree.nodes.length;pass++){let changed=false;for(const edge of next.connections){if(edge.from===edge.to)continue;const value=rank.get(edge.from)+1;if(value>rank.get(edge.to)&&value<next.tree.nodes.length){rank.set(edge.to,value);changed=true;}}if(!changed)break;}
                const lanes=new Map();for(const n of next.tree.nodes){const level=rank.get(n.path);if(!lanes.has(level))lanes.set(level,[]);lanes.get(level).push(n);}
                for(const [level,lane] of lanes)for(const [i,n] of lane.entries()){n.parent=null;n.position=[(i-(lane.length-1)/2)*260,level*190,Math.sin(i*2.4)*40];}
            }
            conceptGraphs[kind]=next;if(mode===kind)setGraph(next);
        },
        setMode(next){
            if(next===mode)return;
            clearHold();window.dispatchEvent(new CustomEvent('dado-node-hold',{detail:{close:true}}));
            stopCameraMotion();
            modeStates[mode]={camera:{...camera},target:{...target},selection:new Set(selection),selected,visits:[...visits],visitIndex};
            mode=next;const saved=modeStates[mode];camera=saved?.camera || initialCamera();target=saved?.target || {...camera};cameraTween=null;selection=saved?.selection || new Set();selected=saved?.selected || null;visits=saved?.visits || [];visitIndex=saved?.visitIndex ?? -1;
            svg.setAttribute('aria-label',mode==='folders'?'Folder structure':mode==='mental'?'Hierarchy of thought':'System workflow concepts');
            const graph=mode==='folders'?folderGraph:conceptGraphs[mode];if(graph)setGraph(graph);schedule();
        },
        openFolder(path){if(mode!=='folders'){folderGraph={...folderGraph};this.setMode('folders');}navigate(path);},
        beginNote(kind,folder,paths,point,rect) {
            const horizontal=(point.x-camera.panX)/(fit*camera.zoom);
            const divisor=Math.cos(camera.yaw)-horizontal*Math.sin(camera.yaw)*fit/1600;
            const x=Math.abs(divisor)>.05?horizontal/divisor:horizontal,y=((point.y-camera.panY+130)/(fit*camera.zoom));
            window.dadoComments?.begin({source:'Systems',kind,folder,sceneFolder:scene.current,position:{x,y},quote:kind==='idea'?`New Idea/Thing — uncertain possibility in folder: ${folder}`:`Systems comment in folder: ${folder}${paths.length?' · '+paths.join(', '):''}`,from:0,to:0},rect);
        },
        goHistory(direction) {
            const index = visitIndex + direction;
            if (index < 0 || index >= visits.length || navigationPending || performance.now() < blockedUntil) return;
            historyTarget = index; navigate(visits[index]);
        },
        selectPath: path => { if (selection.has(path)) return; selected = path; selection = new Set(path ? [path] : []); schedule(); },
        selection: () => [...selection],
        reset: () => { animateCamera(initialCamera(), 450); hint(null); },
        cancelNavigation: () => { historyTarget = null; navigationPending = false; wheel = 0; hint('Folder is no longer available', 1400); },
    };
    const unsubscribeNotes = window.dadoComments?.subscribe(schedule);
    send({ action: 'ready' });
})();
