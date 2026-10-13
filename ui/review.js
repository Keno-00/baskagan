(function(root,factory) {
    const api=factory();
    if(typeof module==='object' && module.exports) module.exports=api;
    else root.dadoReview=api;
})(globalThis,()=>{
    const normalizeText=text=>text.replace(/^\ufeff/,'').replace(/\r\n/g,'\n');
    function pathKey(path) {
        path=String(path).replace(/^\\\\\?\\/,'').replace(/\\/g,'/');
        const windows=/^[a-z]:\//i.test(path) || path.startsWith('//');
        const prefix=path.startsWith('//') ? '//' : path.startsWith('/') ? '/' : '';
        const parts=[];
        for(const part of path.split('/')) {if(!part || part==='.') continue; if(part==='..' && parts.length && parts.at(-1)!=='..') parts.pop(); else parts.push(part);}
        const value=prefix+parts.join('/');return windows ? value.toLowerCase() : value;
    }
    function applicationIssue(review) {
        if(review.changes?.some(file=>file.conflict))return 'A file changed on disk. Resolve the conflict before applying.';
        if(review.changes?.some(file=>file.operation && !['modify','create','delete'].includes(file.operation)))return 'This file operation is not supported.';
        if(review.structuralSaveAvailable!==true && review.changes?.some(file=>file.operation && file.operation!=='modify'))return 'Additions and deletions can be reviewed; applying them is not available yet. The live files are unchanged.';
        return null;
    }
    function diff(before,after,operation='modify') {
        if(operation==='create' || operation==='delete') {
            const text=normalizeText(operation==='create'?after:before);
            return text ? text.split('\n').map((text,i)=>operation==='create'?{kind:'add',newLine:i+1,text}:{kind:'remove',oldLine:i+1,text}):[];
        }
        const a=normalizeText(before).split('\n'),b=normalizeText(after).split('\n');
        let prefix=0,suffix=0;
        while(prefix<a.length && prefix<b.length && a[prefix]===b[prefix]) prefix++;
        if(prefix===a.length && prefix===b.length) return [];
        while(suffix<a.length-prefix && suffix<b.length-prefix && a.at(-1-suffix)===b.at(-1-suffix)) suffix++;
        const rows=[];
        if(prefix>3) rows.push({kind:'gap',text:`${prefix-3} unchanged lines`});
        for(let i=Math.max(0,prefix-3);i<prefix;i++) rows.push({kind:'context',oldLine:i+1,newLine:i+1,text:a[i]});
        for(let i=prefix;i<a.length-suffix;i++) rows.push({kind:'remove',oldLine:i+1,text:a[i]});
        for(let i=prefix;i<b.length-suffix;i++) rows.push({kind:'add',newLine:i+1,text:b[i]});
        for(let i=0;i<Math.min(3,suffix);i++) rows.push({kind:'context',oldLine:a.length-suffix+i+1,newLine:b.length-suffix+i+1,text:a[a.length-suffix+i]});
        if(suffix>3) rows.push({kind:'gap',text:`${suffix-3} unchanged lines`});
        return rows;
    }
    return {normalizeText,pathKey,diff,applicationIssue};
});
