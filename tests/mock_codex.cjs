// A protocol fixture. Command tests modify only their disposable owned work copy.
// It never authenticates or contacts a model.
const fs = require('node:fs');
const path = require('node:path');
const readline = require('node:readline');
const output = value => process.stdout.write(JSON.stringify(value) + '\n');
let timer, approvalId, budgetReplies = 0, fullMode = false, executions = 0, instructions='';
let childStop=false, childInterrupted=false;
readline.createInterface({input: process.stdin}).on('line', line => {
    const message = JSON.parse(line), {id, method, params} = message;
    const reply = result => output({id,result});
    if (!method) {
        if(id==='new-file-proposal') {
            if(message.result?.success!==true)process.exit(16);
            output({method:'turn/completed',params:{threadId:'fixture-thread',turn:{id:'fixture-turn',status:'completed',error:null}}});
        }
        if(id==='full-proposal') {
            if(message.result?.success!==false || !message.result.contentItems[0].text.includes('native tools'))process.exit(13);
            output({method:'turn/completed',params:{threadId:'fixture-thread',turn:{id:'fixture-turn',status:'completed',error:null}}});
        }
        if(id==='fixture-command') {
            output({method:'turn/completed',params:{threadId:'fixture-thread',turn:{id:'fixture-turn',status:'completed',error:null}}});
        }
        if(id==='proposal-read') {
            if(!message.result?.success) process.exit(7);
            const read=JSON.parse(message.result.contentItems[0].text);
            output({id:'proposal-write',method:'item/tool/call',params:{threadId:'fixture-thread',turnId:'fixture-turn',callId:'proposal-write',tool:'sde_propose_file',arguments:{path:'main.rs',expectedRevision:read.revision,text:'fn main() { println!("PROPOSED"); }\n'}}});
        }
        if(id==='proposal-write') {
            if(!message.result?.success || JSON.parse(message.result.contentItems[0].text).applied!==false) process.exit(8);
            output({method:'item/completed',params:{threadId:'fixture-thread',turnId:'fixture-turn',item:{type:'agentMessage',id:'proposal-reply',text:'Proposed a change for review.'}}});
            output({method:'turn/completed',params:{threadId:'fixture-thread',turn:{id:'fixture-turn',status:'completed',error:null}}});
        }
        if (String(id).startsWith('budget-')) {
            const index = Number(String(id).slice(7));
            if (message.result?.success !== (index < 64)) process.exit(6);
            if (++budgetReplies === 65) output({method:'turn/completed',params:{threadId:'fixture-thread',turn:{id:'fixture-turn',status:'completed',error:null}}});
        }
        if (id === 'fixture-read' && (!message.result?.success || !message.result.contentItems[0].text.includes('unsaved'))) process.exit(5);
        if (id === approvalId && message.result?.decision !== 'decline') process.exit(2);
        return;
    }
    switch (method) {
        case 'initialize': reply({userAgent:'codex-cli/0.162.0-alpha.2'}); break;
        case 'account/read': reply({account:{type:'chatgpt',email:'fixture@example.test',planType:'pro'}}); break;
        case 'model/list': reply({data:[{model:'fixture',displayName:'Protocol fixture',isDefault:true,defaultReasoningEffort:'low',supportedReasoningEfforts:[{reasoningEffort:'low'}]}],nextCursor:null}); break;
        case 'account/rateLimits/read': reply({rateLimits:{primary:{usedPercent:25,windowDurationMins:300}},rateLimitsByLimitId:null}); break;
        case 'skills/list':
            if(params.forceReload!==true || params.cwds.length!==1)process.exit(14);
            reply({data:[{cwd:params.cwds[0],skills:[],errors:[]}]});break;
        case 'thread/start':
        case 'thread/resume':
            instructions=params.baseInstructions;
            fullMode=params.sandbox==='danger-full-access';
            if (fullMode && params.dynamicTools?.length) process.exit(12);
            if (params.approvalPolicy !== (fullMode?'never':'on-request')) process.exit(3);
            if (params.config['features.shell_tool'] !== fullMode || params.config['features.unified_exec'] !== fullMode) process.exit(10);
            setTimeout(()=>reply({thread:{id:'fixture-thread'},approvalPolicy:params.approvalPolicy,sandbox:{type:fullMode?'dangerFullAccess':'readOnly'},cwd:params.cwd}),25); break;
        case 'turn/start': {
            if(params.input[0].text.includes('Explain one held node')){
                reply({turn:{id:'fixture-turn'}});
                const prompt=params.input[0].text;
                const fileCorrected=prompt.includes('File not found: missing-insight.rs');
                const symbolCorrected=prompt.includes('Expected one symbol name for main.rs') && prompt.includes('Symbol missing_capture missing from main.rs');
                if(fileCorrected && !symbolCorrected)process.exit(26);
                const corrected=fileCorrected && symbolCorrected && !prompt.includes('fixture: uncorrectable references');
                const explanation={description:'Camera capture produces frames for its connected responsibilities.',relationships:[],sources:corrected?[{path:'main.rs',kind:'file',symbol:'main',start:1,end:1}]:[{path:'missing-insight.rs',kind:'file',symbol:null,start:1,end:1},{path:'main.rs',kind:'file',symbol:'main, capture',start:1,end:1},{path:'main.rs',kind:'file',symbol:'missing_capture',start:1,end:1}]};
                output({method:'item/agentMessage/delta',params:{threadId:'fixture-thread',turnId:'fixture-turn',itemId:'insight-answer',delta:JSON.stringify(explanation)}});
                setTimeout(()=>{output({method:'item/completed',params:{threadId:'fixture-thread',turnId:'fixture-turn',item:{type:'agentMessage',id:'insight-answer',text:JSON.stringify(explanation)}}});output({method:'turn/completed',params:{threadId:'fixture-thread',turn:{id:'fixture-turn',status:'completed',error:null}}});},100);break;
            }
            if(params.input[0].text.includes('Build a source-backed mental model and system workflows')){
                if(!params.outputSchema?.properties?.concepts?.items?.properties?.sources?.items?.anyOf)process.exit(25);
                reply({turn:{id:'fixture-turn'}});
                const corrected=params.input[0].text.includes('File not found: missing.rs') && params.input[0].text.includes('File not found: also-missing.rs');
                if(corrected){const prior=JSON.parse(params.input[0].text.match(/Previous model: (.*?)\nChanged paths:/s)[1]);if(prior.concepts[0]?.sources[0]?.path!=='missing.rs')process.exit(24);}
                const model={version:1,concepts:[{id:'project',name:'Project purpose',description:'A source-backed test project.',parent:null,sources:corrected?[{path:'main.rs',kind:'file',symbol:null,start:1,end:1}]:['missing.rs','also-missing.rs'].map(path=>({path,kind:'file',symbol:null,start:1,end:1}))}],connections:[]};
                const prefix='{"version":1,"concepts":['+JSON.stringify(model.concepts[0]);
                output({method:'item/agentMessage/delta',params:{threadId:'fixture-thread',turnId:'fixture-turn',itemId:'model-answer',delta:prefix}});
                setTimeout(()=>{
                    output({method:'item/completed',params:{threadId:'fixture-thread',turnId:'fixture-turn',item:{type:'agentMessage',id:'model-answer',text:JSON.stringify(model)}}});
                    output({method:'turn/completed',params:{threadId:'fixture-thread',turn:{id:'fixture-turn',status:'completed',error:null}}});
                },450);break;
            }
            if (params.sandboxPolicy?.type !== (fullMode?'dangerFullAccess':'readOnly') || (!fullMode && params.sandboxPolicy?.networkAccess !== false)) process.exit(4);
            if (params.input[0].text.includes('TEST_ANNOTATIONS')) {
                const text = params.input[0].text;
                if (!text.includes('SELECTED_QUOTE') || !text.includes('SELECTED_FEEDBACK') ||
                    text.includes('UNSELECTED_QUOTE') || text.includes('UNSELECTED_DISK_CONTENT')) process.exit(9);
            }
            approvalId=id;
            output({id:approvalId,method:'item/commandExecution/requestApproval',params:{threadId:'fixture-thread',turnId:'fixture-turn',command:'SHOULD NEVER RUN'}});
            output({method:'item/agentMessage/delta',params:{threadId:'fixture-thread',turnId:'retired-turn',itemId:'stale-before-bind',delta:'MUST NOT DISPLAY'}});
            reply({turn:{id:'fixture-turn'}});
            if(params.input[0].text.includes('TEST_CHILD_STOP')) {
                childStop=true;
                output({method:'turn/started',params:{threadId:'owned-child',turn:{id:'child-turn'}}});
                if(params.input[0].text.includes('TEST_CHILD_STOP_METADATA')) {
                    output({method:'thread/started',params:{thread:{id:'owned-child',parentThreadId:'fixture-thread'}}});
                } else {
                output({method:'item/completed',params:{threadId:'fixture-thread',turnId:'fixture-turn',item:{type:'collabAgentToolCall',id:'spawn-child',tool:'spawnAgent',senderThreadId:'fixture-thread',receiverThreadIds:['owned-child'],agentsStates:{},status:'completed'}}});
                }
                output({method:'turn/started',params:{threadId:'unrelated-child',turn:{id:'unrelated-turn'}}});
                output({method:'turn/started',params:{threadId:'owned-child',turn:{id:'child-turn'}}});break;
            }
            if(params.input[0].text.includes('TEST_NEW_FILE_PROPOSAL')) {
                output({id:'new-file-proposal',method:'item/tool/call',params:{threadId:'fixture-thread',turnId:'fixture-turn',callId:'new-file-proposal',tool:'sde_propose_file',arguments:{path:'notes/new.txt',expectedRevision:'absent',text:'NEW_PROPOSED_FILE'}}});break;
            }
            // Cancellation tests explicitly own the completion boundary rather
            // than relying on the test thread beating a wall-clock timer.
            if(params.input[0].text.includes('TEST_HOLD_UNTIL_STOP'))break;
            if(params.input[0].text.includes('TEST_SKILL')) {
                const skill=params.input.find(i=>i.type==='skill');
                if(!skill || skill.name!=='fixture-skill' || !fs.readFileSync(skill.path,'utf8').includes('FROZEN_SKILL_INSTRUCTIONS') || fullMode || !params.input[0].text.includes('$fixture-skill'))process.exit(15);
                output({method:'item/completed',params:{threadId:'fixture-thread',turnId:'fixture-turn',item:{id:'skill-answer',type:'agentMessage',text:'Used the selected skill.'}}});
                output({method:'turn/completed',params:{threadId:'fixture-thread',turn:{id:'fixture-turn',status:'completed'}}});break;
            }
            if(params.input[0].text.includes('TEST_ITEM_EVENTS')) {
                const p={threadId:'fixture-thread',turnId:'fixture-turn'};
                const mcp={type:'mcpToolCall',id:'mcp-1',server:'fixture',tool:'inspect',status:'completed',error:null};
                output({method:'item/started',params:{...p,item:mcp}});
                const command={type:'commandExecution',id:'command-1',command:'check',aggregatedOutput:'failed check',status:'completed',exitCode:1};
                output({method:'item/started',params:{...p,item:command}});
                output({method:'item/completed',params:{...p,item:command}});
                output({method:'item/completed',params:{...p,item:command}});
                output({method:'item/started',params:{...p,item:command}});
                output({method:'item/started',params:{...p,item:{type:'fileChange',id:'file-1',changes:[]}}});
                output({method:'item/agentMessage/delta',params:{...p,itemId:'answer',delta:'Partial'}});
                output({method:'turn/completed',params:{threadId:'fixture-thread',turn:{id:'fixture-turn',status:'completed',items:[command,mcp,{type:'webSearch',id:'search-1',query:'fixture documentation'},{type:'agentMessage',id:'answer',text:'Final snapshot answer'}]}}});break;
            }
            if(params.input[0].text.includes('TEST_FULL_NO_PROPOSALS')) {
                output({id:'full-proposal',method:'item/tool/call',params:{threadId:'fixture-thread',turnId:'fixture-turn',callId:'full-proposal',tool:'sde_propose_file',arguments:{path:'new.txt',expectedRevision:'missing',text:'WRONG PROPOSAL'}}});break;
            }
            if(params.input[0].text.includes('TEST_COMMAND_GATE')) {
                if(!instructions.includes('reviewed text additions') || instructions.includes('cannot yet be applied'))process.exit(17);
                output({id:'fixture-command',method:'item/tool/call',params:{threadId:'fixture-thread',turnId:'fixture-turn',callId:'fixture-command',tool:'sde_run_command',arguments:{command:'fixture change main.rs',reason:'Verify one-time command approval',timeoutSeconds:30}}}); break;
            }
            if(params.input[0].text.includes('TEST_MALFORMED') || params.input[0].text.includes('TEST_OVERSIZED')) {
                output({method:'item/agentMessage/delta',params:{threadId:'fixture-thread',turnId:'fixture-turn',itemId:'partial',delta:'Retain this partial answer'}});
                process.stdout.write(params.input[0].text.includes('TEST_MALFORMED') ? 'SECRET invalid json\n' : 'x'.repeat(2*1024*1024+1));
                break;
            }
            if(params.input[0].text.includes('TEST_PROPOSE')) {
                output({id:'proposal-read',method:'item/tool/call',params:{threadId:'fixture-thread',turnId:'fixture-turn',callId:'proposal-read',tool:'sde_read_file',arguments:{path:'main.rs'}}});break;
            }
            if (params.input[0].text.includes('TEST_DISCONNECT')) {setTimeout(()=>process.exit(0),60);break;}
            if (params.input[0].text.includes('TEST_TOOL_BUDGET')) {
                for (let n=0; n<65; n++) output({id:`budget-${n}`,method:'item/tool/call',params:{threadId:'fixture-thread',turnId:'fixture-turn',callId:`budget-${n}`,tool:'sde_read_file',arguments:{path:'main.rs'}}});
                break;
            }
            if(params.input[0].text.includes('TEST_INSPECT')) output({id:'fixture-read',method:'item/tool/call',params:{threadId:'fixture-thread',turnId:'fixture-turn',callId:'read-1',tool:'sde_read_file',arguments:{path:'main.rs'}}});
            output({method:'item/agentMessage/delta',params:{threadId:'unrelated-thread',turnId:'fixture-turn',itemId:'bad',delta:'MUST NOT DISPLAY'}});
            output({method:'item/agentMessage/delta',params:{threadId:'fixture-thread',turnId:'fixture-turn',itemId:'reply',delta:'Hello'}});
            timer=setTimeout(()=> {
                const failure=params.input[0].text.includes('TEST_LATE_FAILURE');
                output({method:'item/completed',params:{threadId:'fixture-thread',turnId:'fixture-turn',item:{type:'agentMessage',id:'reply',text:'Hello fixture'}}});
                output({method:'turn/completed',params:{threadId:'fixture-thread',turn:{id:'fixture-turn',status:failure?'failed':'completed',error:failure?{message:'Fixture late stream failure'}:null}}});
            },150);
            break;
        }
        case 'command/exec':
            if (!params.cwd.includes(path.join('.dado','workspaces')) || path.basename(params.cwd)!=='work' || params.sandboxPolicy?.type!=='dangerFullAccess' || params.outputBytesCap!==65536 || params.timeoutMs!==30000 || params.command.at(-1)!=='fixture change main.rs' || ++executions!==1) process.exit(11);
            fs.writeFileSync(path.join(params.cwd,'main.rs'),'APPROVED COMMAND\n');
            reply({exitCode:0,stdout:'Executed once',stderr:''}); break;
        case 'turn/interrupt':
            if(childStop) {
                if(params.threadId==='owned-child' && params.turnId==='child-turn') {
                    childInterrupted=true;reply({});
                    output({method:'turn/completed',params:{threadId:'owned-child',turn:{id:'child-turn',status:'interrupted'}}});break;
                }
                if(params.threadId!=='fixture-thread' || !childInterrupted)process.exit(18);
            }
            clearTimeout(timer); reply({});
            output({method:'turn/completed',params:{threadId:'fixture-thread',turn:{id:'fixture-turn',status:'interrupted',error:null}}}); break;
        default: if (id !== undefined) output({id,error:{code:-32601,message:'Unsupported fixture request'}});
    }
});
