(() => {
    window.dadoAtmosphere?.dispose();
    const canvas = document.getElementById('systems-atmosphere');
    if (!canvas) return;
    const host = canvas.parentElement, context = canvas.getContext('2d');
    const media = matchMedia('(prefers-reduced-motion: reduce)');
    const events = new AbortController();
    let frame = null, disposed = false, previous = 0, elapsed = 0;
    let width = 1, height = 1, color = '#888', brand = null, scene = [], light = false;
    let activity = "idle", energy = 0, activityTime = 0;
    let options = window.dadoChat?.preferences()?.appearance || {}, births = 0, deaths = 0, spawn = 0, topologyTime = 0;
    let targets = [], ripples = [], flow = {x:0,y:0,turn:0,zoom:0}, lastCamera = null;
    let transferred = new Set(), handoff = null;
    let previousBox = null, quietZones = [];
    let mode = host.dataset.mode, blend = mode === 'project' ? 1 : 0;
    let seed = 197;
    const random = () => { seed = (seed * 1664525 + 1013904223) >>> 0; return seed / 4294967296; };
    const particles = Array.from({ length: 52 }, (_, i) => ({
        u: random(), v: random(), angle: random() * Math.PI * 2,
        radius: i < 38 ? Math.sqrt((i + .5) / 38) : Math.sqrt(random()), phase: random() * Math.PI * 2,
        cluster: i < 38, x: null, y: null,
        vx:0,vy:0,age:4+random()*20,life:35+random()*25,
    }));
    particles.slice(0, 38).forEach((p, i) => { p.angle = i * 2.399963; });
    const topology = [];
    let linkAges=new Map();
    function connect() {const nextAges=new Map();topology.length=0;for (let i = 0; i < particles.length; i++) {
        const position = p => p.cluster ? [Math.cos(p.angle) * p.radius, Math.sin(p.angle) * p.radius] : [p.u * 4 - 2, p.v * 3 - 1.5];
        const a = particles[i].x===null ? position(particles[i]) : [particles[i].x/Math.max(width,1)*3,particles[i].y/Math.max(height,1)*3];
        const neighbors = particles.map((p, j) => {const b=p.x===null?position(p):[p.x/Math.max(width,1)*3,p.y/Math.max(height,1)*3];return { j, d: Math.hypot(a[0] - b[0], a[1] - b[1]) };})
            .filter(p => p.j !== i).sort((a, b) => a.d - b.d).slice(0, 2);
        for (const { j } of neighbors) if (!topology.some(([a, b]) => a === j && b === i)){const key=[particles[i].phase,particles[j].phase].sort().join(':');const born=linkAges.get(key)??(elapsed===0?-2:elapsed);nextAges.set(key,born);topology.push([i,j,born]);}
    }linkAges=nextAges;}connect();
    const reduced = () => (options.respectReducedMotion!==false && media.matches) || document.documentElement.dataset.reducedMotion === 'true';
    const intensity = () => options.motionIntensity==='low'?.5:options.motionIntensity==='lively'?1.65:1;
    function measure() {
        const box = host.getBoundingClientRect(), scale = Math.min(devicePixelRatio || 1, 2);
        if (previousBox) for (const p of particles) if (p.x !== null) { p.x += previousBox.x - box.x; p.y += previousBox.y - box.y; }
        previousBox = { x: box.x, y: box.y };
        width = box.width; height = box.height;
        canvas.width = Math.max(1, Math.round(width * scale));
        canvas.height = Math.max(1, Math.round(height * scale));
        context.setTransform(scale, 0, 0, scale, 0, 0);
        const style = getComputedStyle(host);
        color = style.getPropertyValue('--ui-muted').trim() || style.color;
        light = document.documentElement.dataset.theme !== 'dark';
        const label = host.querySelector('.welcome-brand')?.getBoundingClientRect();
        if (label) brand = { x: label.x - box.x + label.width / 2, y: label.y - box.y + label.height / 2, width: label.width, height: label.height };
        quietZones = [...host.querySelectorAll('.welcome-brand p, .welcome-baybayin, .welcome-content')].map(e => {
            const r = e.getBoundingClientRect(); return { x: r.x - box.x, y: r.y - box.y, width: r.width, height: r.height };
        });
        schedule();
    }
    function schedule() {
        if (frame === null && !disposed && !document.hidden) frame = requestAnimationFrame(draw);
    }
    function draw(now) {
        frame = null;
        if (!canvas.isConnected) { dispose(); return; }
        const still = reduced() || options.backgroundMotion===false, dt = Math.min((now - (previous || now)) / 1000, .05);
        previous = now;
        if (!still) elapsed += dt;
        const nextActivity = host.dataset.activity || 'idle';
        if (activity !== nextActivity) { activity = nextActivity; activityTime = elapsed; }
        const targetEnergy = options.agentMotion===false ? 0 : activity === 'working' ? 1 : activity === 'checking' ? .3 : activity === 'done' ? Math.max(0, 1 - (elapsed - activityTime) / 2) : 0;
        energy = still ? 0 : energy + (targetEnergy - energy) * (1 - Math.exp(-dt * 2));
        const nextMode = host.dataset.mode;
        if (nextMode !== mode) { mode = nextMode; if (mode === 'welcome') { scene = []; transferred.clear(); handoff = null; measure(); } }
        const target = mode === 'project' && handoff !== null ? 1 : 0;
        blend = still ? target : blend + (target - blend) * (1 - Math.exp(-dt * 3));
        if (Math.abs(target - blend) < .001) blend = target;
        const center = brand || { x: width * .3, y: height * .5, width: 320, height: 130 };
        targets=targets.filter(t=>t.until>now).map(t=>({...t,...scene.find(p=>p.path===t.path)}));
        ripples=ripples.filter(r=>now-r.born<1800);
        const gathering=[...targets,...scene.filter(p=>p.held).map(p=>({...p,kind:'hold'}))].slice(0,12);
        if(!still){
            for(const key of Object.keys(flow))flow[key]*=Math.exp(-dt*2.4);
            if(handoff!==null && now-handoff>700)transferred.clear();
            for(let i=particles.length-1;i>=0;i--){particles[i].age+=dt;if(particles[i].age>particles[i].life){particles.splice(i,1);deaths++;topologyTime=-1;}}
            const reading=gathering.some(p=>p.kind==='read');
            spawn=Math.min(1,spawn+dt*(1.2+(reading?3:0))*intensity());
            if((spawn>=1 || particles.length<44) && particles.length<(reading?80:64)){
                spawn=0;const near=gathering[Math.floor(random()*gathering.length)];
                const x=near?Math.max(0,Math.min(width,near.x+(random()-.5)*180)):random()*width;
                const y=near?Math.max(0,Math.min(height,near.y+(random()-.5)*160)):random()*height;
                particles.push({u:x/width,v:y/height,angle:random()*Math.PI*2,radius:random(),phase:random()*Math.PI*2,cluster:false,x,y,vx:0,vy:0,age:0,life:35+random()*25});births++;topologyTime=-1;
            }
            if(elapsed-topologyTime>.8 || topology.some(([i,j])=>i>=particles.length||j>=particles.length)){connect();topologyTime=elapsed;}
        }
        context.clearRect(0, 0, width, height);
        const points = particles.map((p, i) => {
            const angle = p.angle;
            const wx = p.cluster ? center.x + Math.cos(angle) * p.radius * Math.min(width * .32, 290) : p.u * width;
            const wy = p.cluster ? center.y + Math.sin(angle) * p.radius * Math.min(height * .4, 240) : p.v * height;
            let px = p.cluster ? width * .5 + Math.cos(p.angle) * p.radius * width * .36 : p.u * width;
            let py = p.cluster ? height * .47 + Math.sin(p.angle) * p.radius * height * .38 : p.v * height;
            let x = wx + (px - wx) * blend + (Math.sin(elapsed*.17+p.phase)*17+Math.sin(elapsed*.071+p.phase*2)*8)*intensity();
            let y = wy + (py - wy) * blend + (Math.cos(elapsed*.13+p.phase)*13+Math.sin(elapsed*.093+p.phase*3)*7)*intensity();
            if(gathering.length){const g=gathering[i%gathering.length],distance=Math.hypot(g.x-x,g.y-y),weight=Math.max(0,1-distance/360)*Math.min(.8,.65*intensity());
                const angle=p.phase+elapsed*(.16+.035*Math.sin(p.phase)),radius=28+Math.abs(Math.sin(p.phase*3))*65;
                x+=(g.x+Math.cos(angle)*radius-x)*weight;y+=(g.y+Math.sin(angle*.83+p.phase)*radius*.7-y)*weight;
            }
            if(p.x===null){p.x=x;p.y=y;}
            if(!still){
                p.vx+=(x-p.x)*dt*.9;p.vy+=(y-p.y)*dt*.9;
                p.vx+=(flow.x+flow.turn+(p.x-width/2)*flow.zoom)*dt*intensity();
                p.vy+=(flow.y+(p.y-height/2)*flow.zoom)*dt*intensity();
                for(const r of ripples){const dx=p.x-r.x,dy=p.y-r.y,d=Math.max(1,Math.hypot(dx,dy)),age=(now-r.born)/1800,force=Math.exp(-Math.pow((d-age*160)/55,2))*(1-age)*18*intensity();p.vx+=dx/d*force*dt;p.vy+=dy/d*force*dt;}
                p.vx*=Math.exp(-dt*1.5);p.vy*=Math.exp(-dt*1.5);p.x+=p.vx*dt;p.y+=p.vy*dt;
            }
            let strength = Math.min(1,p.age/2,Math.max(0,(p.life-p.age)/4)) * (transferred.has(i) ? Math.max(0, 1 - (now - handoff) / 650) : 1);
            if (blend < 1 && Math.abs(p.x - center.x) < center.width / 2 + 12 && Math.abs(p.y - center.y) < center.height / 2 + 12) strength *= .5 + .5 * blend;
            if (blend > 0 && scene.some(n => Math.hypot(n.x - p.x, n.y - p.y) < 40)) strength *= 1 - .8 * blend;
            return { x: p.x, y: p.y, strength };
        });
        context.strokeStyle = context.fillStyle = color;
        context.lineWidth = light ? .9 : .8;
        for (const [i, j, born] of topology) {
            const a = points[i], b = points[j];if(!a||!b)continue;const distance = Math.hypot(a.x - b.x, a.y - b.y);if(distance>280)continue;
            context.globalAlpha = (light ? .3 : .2) * Math.min(1,(elapsed-born)/1.5) * (1 + energy * .35 * (.5 + .5 * Math.sin(elapsed * 1.7 + i * .17))) * Math.min(1,(280-distance)/70) * Math.min(a.strength, b.strength);
            context.beginPath(); context.moveTo(a.x, a.y); context.lineTo(b.x, b.y); context.stroke();
        }
        for (let i = 0; i < points.length; i++) {
            const a = points[i];
            if (transferred.has(i)) continue;
            context.globalAlpha = (.32 + energy * .12 * (.5 + .5 * Math.sin(elapsed * 1.7 + i * .17))) * a.strength;
            context.beginPath(); context.arc(a.x, a.y, 1.5, 0, Math.PI * 2); context.fill();
        }
        context.globalAlpha = 1;
        context.save();
        context.globalCompositeOperation = 'destination-out';
        context.globalAlpha = .85;
        context.filter = 'blur(14px)';
        for (const zone of quietZones) context.fillRect(zone.x - 8, zone.y - 4, zone.width + 16, zone.height + 8);
        context.restore();
        if (!still) schedule();
    }
    const resize = new ResizeObserver(measure);
    resize.observe(host);
    const attributes = new MutationObserver(measure);
    attributes.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme', 'data-reduced-motion', 'style'] });
    attributes.observe(host, { attributes: true, attributeFilter: ['data-mode', 'data-activity', 'data-inspecting'] });
    document.addEventListener('visibilitychange', () => { previous = 0; schedule(); }, { signal: events.signal });
    media.addEventListener('change', schedule, { signal: events.signal });
    document.addEventListener('dado-appearance-change',e=>{options=e.detail;if(options.agentMotion===false)targets=[];flow={x:0,y:0,turn:0,zoom:0};previous=0;schedule();},{signal:events.signal});
    document.addEventListener('pointerdown',e=>{if(options.navigationMotion===false||reduced()||options.backgroundMotion===false)return;const box=host.getBoundingClientRect();if(e.clientX<box.left||e.clientX>box.right||e.clientY<box.top||e.clientY>box.bottom)return;ripples.push({x:e.clientX-box.left,y:e.clientY-box.top,born:performance.now()});ripples=ripples.slice(-6);schedule();},{signal:events.signal});
    function dispose() {
        disposed = true; cancelAnimationFrame(frame); events.abort(); resize.disconnect(); attributes.disconnect();
        if (window.dadoAtmosphere?.dispose === dispose) delete window.dadoAtmosphere;
    }
    window.dadoAtmosphere = {
        takeOrigins(svg, count) {
            measure();
            const matrix = svg.getScreenCTM(), box = host.getBoundingClientRect();
            if (!matrix) return [];
            const inverse = matrix.inverse();
            handoff = performance.now();
            const available = particles.map((p, i) => ({ p, i })).filter(({ p }) => p.x !== null);
            // Keep half the constellation as atmosphere, even for large projects.
            const selected = available.filter(({ i }) => i % 2 === 0).slice(0, count);
            transferred = new Set(selected.map(({ i }) => i));
            schedule();
            return selected.map(({ p }) => {
                const point = new DOMPoint(box.x + p.x, box.y + p.y).matrixTransform(inverse);
                return { x: point.x, y: point.y };
            });
        },
        setScene(svg, points, camera) {
            const matrix = svg.getScreenCTM(), box = host.getBoundingClientRect();
            if (!matrix) return;
            scene = points.slice(0, 150).map(p => { const screen = new DOMPoint(p.x, p.y).matrixTransform(matrix); return {...p, x: screen.x - box.x, y: screen.y - box.y }; });
            if(camera && lastCamera && options.navigationMotion!==false && !reduced() && options.backgroundMotion!==false){const cap=(n,max)=>Math.max(-max,Math.min(max,n));flow.x=cap(flow.x+(camera.panX-lastCamera.panX)*1.4,150);flow.y=cap(flow.y+(camera.panY-lastCamera.panY)*1.4,150);flow.turn=cap(flow.turn+(camera.yaw-lastCamera.yaw)*12,18);flow.zoom=cap(flow.zoom+(camera.zoom-lastCamera.zoom)*1.2,.5);}
            if(camera)lastCamera={...camera};
            schedule();
        },
        activity(nodes,kind,status){if(options.agentMotion===false)return;const now=performance.now();for(const path of nodes){targets=targets.filter(t=>t.path!==path);const point=scene.find(p=>p.path===path);if(point && kind!=='stopped')targets.push({...point,kind,until:now+(status==='running'?180000:kind==='read'?2400:1800)});}targets=targets.slice(-12);schedule();},
        snapshot(){return {count:particles.length,births,deaths,moving:!reduced()&&options.backgroundMotion!==false,flow:{...flow},targets:targets.length,points:particles.map(p=>({x:p.x,y:p.y,vx:p.vx,vy:p.vy}))};},
        dispose,
    };
    measure();
})();
