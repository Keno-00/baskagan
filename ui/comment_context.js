(function(root,factory){const api=factory();if(typeof module==='object' && module.exports)module.exports=api;else root.dadoCommentContext=api;})(globalThis,function(){
    function label(item) {return item.source==='Chat' ? (item.quote || 'Comment').replace(/\s+/g,' ').slice(0,50) : `L${item.lineFrom}–L${item.lineTo}`;}
    function capture(items,keys) {
        const chosen=new Set(keys),result=[];
        for(const item of items) {
            if(!item.contextId || !chosen.has(item.contextId))continue;
            chosen.delete(item.contextId);
            result.push({id:item.contextId,displayId:item.id,source:item.source==='Chat'?'chat':'code',path:item.source==='Chat'?null:item.source,conversation:item.conversation || null,message:item.message || null,quote:item.quote || '',text:item.text || '',from:item.from,to:item.to,lineFrom:item.lineFrom || null,lineTo:item.lineTo || null});
        }
        if(result.length>24)throw new Error('Include at most 24 comments in one message.');
        return result;
    }
    const snapshotSignature = item => JSON.stringify(['id','displayId','source','path','conversation','message','quote','text','from','to','lineFrom','lineTo'].map(key=>item?.[key] ?? null));
    const signature = item => snapshotSignature(capture([item],[item.contextId])[0]);
    function pending(items,messages=[]) {
        const sent=new Set(messages.flatMap(message=>(message.comments || []).map(snapshotSignature)));
        return items.filter(item=>{
            const revision=signature(item);
            return item.contextId && revision!==item.contextSentSnapshot && revision!==item.contextDismissedSnapshot && (item.contextSentSnapshot || !sent.has(revision));
        });
    }
    function consume(items,snapshots) {
        for(const snapshot of snapshots) {
            const item=items.find(item=>item.contextId===snapshot.id);
            if(item)item.contextSentSnapshot=snapshotSignature(snapshot);
        }
    }
    return {label,capture,signature,pending,consume};
});
