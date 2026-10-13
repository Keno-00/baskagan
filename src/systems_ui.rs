use crate::{
    Status,
    ui_controls::{Icon, UiButton},
};
use dioxus::prelude::*;
use serde_json::json;
use std::path::{Path, PathBuf};

#[derive(Clone, PartialEq)]
pub struct Context {
    pub paths: Vec<PathBuf>,
    pub command: Option<String>,
    pub folder: PathBuf,
    pub x: f64,
    pub y: f64,
    pub point: serde_json::Value,
}
fn js(source: String) {
    let _ = document::eval(&source);
}
fn checked(root: &Path, path: &Path) -> Result<PathBuf, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let path = path.canonicalize().map_err(|e| e.to_string())?;
    if !path.starts_with(&root) {
        return Err("This item is outside the project.".into());
    }
    Ok(path)
}
fn name(value: &str) -> Result<&str, String> {
    let value = value.trim();
    if value.is_empty() || value == "." || value == ".." || value.contains(['/', '\\', ':']) {
        return Err("Enter a single file or folder name.".into());
    }
    Ok(value)
}
fn copy(source: &Path, dest: &Path) -> Result<(), String> {
    if source.is_symlink() {
        return Err("Symbolic links cannot be pasted.".into());
    }
    if source.is_dir() {
        std::fs::create_dir(dest).map_err(|e| e.to_string())?;
        for entry in std::fs::read_dir(source).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            copy(&entry.path(), &dest.join(entry.file_name()))?;
        }
    } else {
        use std::io::Write;
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(dest)
            .map_err(|e| e.to_string())?;
        let mut input = std::fs::File::open(source).map_err(|e| e.to_string())?;
        std::io::copy(&mut input, &mut output).map_err(|e| e.to_string())?;
        output.flush().map_err(|e| e.to_string())?;
    }
    Ok(())
}
#[component]
pub fn SystemsMenu(root: PathBuf, mut context: Signal<Option<Context>>) -> Element {
    let mut status = use_context::<Status>();
    let mut dialog = use_signal(|| None::<(String, Context)>);
    let mut value = use_signal(String::new);
    let mut problem = use_signal(String::new);
    let mut clipboard = use_signal(|| None::<(bool, Vec<PathBuf>)>);
    let mut results = use_signal(Vec::<(PathBuf, usize, String)>::new);
    let mut close = move || {
        context.set(None);
        js("document.getElementById('folder-scene')?.focus();".into());
    };
    rsx! {
        style {{include_str!("systems_ui.css")}}
        if let Some(ctx)=context() {
            div {class:"systems-dismiss",onmousedown:move |_|close()}
            div {id:"systems-menu",class:"systems-menu",role:"menu",aria_label:"Systems actions",style:format!("left:clamp(8px,{}px,calc(100vw - 238px));top:clamp(8px,{}px,calc(100vh - 470px));",ctx.x,ctx.y),
                onmounted:{let command=ctx.command.clone();move |_|js(if let Some(command)=&command {format!("document.querySelector('#systems-menu button[data-action=\"{}\"]:not(:disabled)')?.click();",command)}else{"document.querySelector('#systems-menu button:not(:disabled)')?.focus();".into()})},
                onkeydown:move |event|{if event.key()==Key::Escape {close();}else if matches!(event.key(),Key::ArrowDown|Key::ArrowUp|Key::Home|Key::End){event.prevent_default();js(format!("(()=>{{const b=[...document.querySelectorAll('#systems-menu button:not(:disabled)')],i=b.indexOf(document.activeElement);b[{}]?.focus()}})();",match event.key(){Key::Home=>"0".into(),Key::End=>"b.length-1".into(),Key::ArrowUp=>"(i+b.length-1)%b.length".into(),_=>"(i+1)%b.length".to_string()}));}},
                for (action,label,icon) in [("new-file","New File…","editor"),("new-folder","New Folder…","folder"),("idea","New Idea/Thing","idea"),("comment","Annotation/Comment","comment"),("reveal","Reveal in File Explorer","external"),("preview","Open in Images Preview","external"),("find","Find in Folder…","editor"),("attach","Add Folder to Chat","folder"),("cut","Cut","edit"),("copy","Copy","editor"),("paste","Paste","editor"),("path","Copy Path","editor"),("relative","Copy Relative Path","editor"),("rename","Rename…","edit"),("delete","Delete…","delete")] {
                    button {r#type:"button",role:"menuitem","data-action":action,disabled:match action {"cut"|"copy"|"delete"=>ctx.paths.is_empty() || ctx.paths.contains(&root),"rename"=>ctx.paths.len()!=1 || ctx.paths.contains(&root),"paste"=>clipboard().is_none(),"preview"=>ctx.paths.len()!=1 || !ctx.paths[0].extension().is_some_and(|e|["png","jpg","jpeg","gif","webp","bmp","svg"].contains(&e.to_string_lossy().to_lowercase().as_str())),_=>false},
                        onmouseenter:move |_|status.hint.set(Some(if action=="idea" {"An uncertain idea for something new — you don’t know what yet, but this folder feels like where it could belong."}else{label}.into())),onmouseleave:move |_|status.hint.set(None),
                        onfocus:move |_|status.hint.set(Some(if action=="idea" {"An uncertain idea for something new — you don’t know what yet, but this folder feels like where it could belong."}else{label}.into())),onblur:move |_|status.hint.set(None),
                        onclick:{let ctx=ctx.clone();let root=root.clone();move |_|{
                            let target=ctx.paths.first().cloned().unwrap_or(ctx.folder.clone());
                            let folder=if target.is_dir(){target.clone()}else{target.parent().unwrap_or(&ctx.folder).to_path_buf()};
                            context.set(None);problem.set(String::new());
                            match action {
                                "new-file"|"new-folder"|"rename"|"delete"|"find"=>{value.set(if action=="rename" {target.file_name().unwrap_or_default().to_string_lossy().into()}else{String::new()});results.set(vec![]);dialog.set(Some((action.into(),ctx.clone())));}
                                "cut"|"copy"=>clipboard.set(Some((action=="cut",top_level(&ctx.paths)))),
                                "paste"=>{if let Some((cut,paths))=clipboard(){let result=(||{let dest_folder=checked(&root,&folder)?;for original in paths {let source=checked(&root,&original)?;let dest=dest_folder.join(source.file_name().ok_or("Invalid item")?);if dest.exists() || dest.starts_with(&source){return Err("Cannot paste over an existing item or into itself.".into());}
                                if cut {std::fs::rename(&source,&dest).map_err(|e|e.to_string())?;moved(&original,&folder.join(source.file_name().ok_or("Missing name")?));}else{copy(&source,&dest)?;}}Ok::<_,String>(())})();if let Err(error)=result{status.hint.set(Some(error));}else if cut{clipboard.set(None);}}}
                                "reveal"=>{let _=std::process::Command::new("explorer.exe").args(if target.is_dir(){vec![target.display().to_string()]}else{vec!["/select,".into(),target.display().to_string()]}).spawn();}
                                "preview"=>{if let Err(e)=open::that(&target){status.hint.set(Some(e.to_string()));}}
                                "path"|"relative"=>{let paths=if ctx.paths.is_empty(){vec![ctx.folder.clone()]}else{ctx.paths.clone()};let text=paths.iter().map(|p|if action=="relative"{p.strip_prefix(&root).unwrap_or(p).display().to_string()}else{p.display().to_string()}).collect::<Vec<_>>().join("\n");js(format!("navigator.clipboard.writeText({}).catch(e=>window.dadoScene?.report(e.message));",json!(text)));}
                                "attach"=>js(format!("window.dadoChat?.attachSystems({});",json!({"path":folder,"name":folder.file_name().unwrap_or_default().to_string_lossy(),"kind":"folder"}))),
                                "idea"|"comment"=>js(format!("window.dadoScene?.beginNote({},{},{},{},{});",json!(action),json!(folder),json!(ctx.paths),json!(ctx.point),json!({"left":ctx.x,"top":ctx.y,"right":ctx.x,"bottom":ctx.y,"width":0,"height":0}))),
                                _=>{}
                            }
                        }},Icon {name:icon} span {"{label}"}
                    }
                }
            }
        }
        if let Some((action,ctx))=dialog() {
            div {class:"systems-dismiss",onmousedown:move |_|dialog.set(None)}
            section {class:"systems-dialog",onmounted:move |_|js("document.querySelector('.systems-dialog input, .systems-dialog button')?.focus();".into()),role:"dialog",aria_modal:"true",aria_label:action.clone(),onkeydown:move |e|{if e.key()==Key::Escape{dialog.set(None);close();}else if e.key()==Key::Enter && !e.modifiers().shift(){e.prevent_default();js("document.querySelector('.systems-dialog-actions button:last-child')?.click();".into());}else if e.key()==Key::Tab {e.prevent_default();js(format!("(()=>{{const b=[...document.querySelectorAll('.systems-dialog input,.systems-dialog button:not(:disabled)')],i=b.indexOf(document.activeElement);b[(i+b.length+{})%b.length]?.focus()}})();",if e.modifiers().shift(){-1}else{1}));}},
                strong {{match action.as_str(){"new-file"=>"New File","new-folder"=>"New Folder","rename"=>"Rename","find"=>"Find in Folder",_=>"Move selected items to project trash?"}}}
                if action!="delete" {input {id:"systems-name",aria_label:if action=="find"{"Search text"}else{"Name"},value:"{value}",onmounted:move |_|js("document.getElementById('systems-name')?.focus();".into()),oninput:move |e|value.set(e.value())}}
                if !problem().is_empty(){p {role:"alert","{problem}"}}
                div {class:"systems-dialog-actions",
                    UiButton {label:"Cancel",text:"Cancel",class:"chat-compact-action",onpress:move |_|{dialog.set(None);close();}}
                    UiButton {label:if action=="find"{"Search"}else{"Confirm"},text:if action=="find"{"Search"}else{"Confirm"},class:"chat-compact-action",onpress:{let action=action.clone();let root=root.clone();move |_|{
                        let result=(||{
                            let target=ctx.paths.first().unwrap_or(&ctx.folder);
                            let folder=if target.is_dir(){target.as_path()}else{target.parent().ok_or("Missing parent")?};
                            let folder=checked(&root,folder)?;
                            match action.as_str(){
                                "new-file"=>{let dest=folder.join(name(&value())?);std::fs::OpenOptions::new().write(true).create_new(true).open(dest).map_err(|e|e.to_string())?;}
                                "new-folder"=>std::fs::create_dir(folder.join(name(&value())?)).map_err(|e|e.to_string())?,
                                "rename"=>{let source=checked(&root,target)?;let dest=source.parent().ok_or("Missing parent")?.join(name(&value())?);if dest.exists(){return Err("That name already exists.".into());}std::fs::rename(&source,&dest).map_err(|e|e.to_string())?;moved(target,&target.with_file_name(dest.file_name().ok_or("Missing name")?));}
                                "delete"=>{let trash=root.join(".dado").join("trash").join(uuid::Uuid::new_v4().to_string());std::fs::create_dir_all(&trash).map_err(|e|e.to_string())?;let trash=checked(&root,&trash)?;for path in top_level(&ctx.paths) {let path=checked(&root,&path)?;if path==root.canonicalize().map_err(|e|e.to_string())?{return Err("Cannot delete the project root.".into());}std::fs::rename(&path,trash.join(path.file_name().ok_or("Missing name")?)).map_err(|e|e.to_string())?;}}
                                "find"=>{if value().trim().is_empty(){return Err("Enter search text.".into());}let mut found=vec![];search(&folder,&value(),&mut found);results.set(found);return Ok(false);}
                                _=>{}
                            } Ok(true)
                        })();match result {Ok(true)=>{dialog.set(None);close();},Ok(false)=>{},Err(e)=>problem.set(e)}
                    }}}
                }
                if action=="find" {div {class:"systems-results",for (path,line,text) in results(){button {r#type:"button",onclick:move |_|{js(format!("window.dadoWorkbench?.openFile({});",json!(path)));dialog.set(None);},"{path.display()}:{line} {text}"}}}}
            }
        }
    }
}
fn search(folder: &Path, query: &str, found: &mut Vec<(PathBuf, usize, String)>) {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        if found.len() >= 100 {
            return;
        }
        let path = entry.path();
        if path.is_symlink()
            || [".dado", ".git", "node_modules", "target"]
                .contains(&entry.file_name().to_string_lossy().as_ref())
        {
            continue;
        }
        if path.is_dir() {
            search(&path, query, found);
        } else if entry.metadata().is_ok_and(|m| m.len() <= 1024 * 1024)
            && let Ok(text) = std::fs::read_to_string(&path)
        {
            for (i, line) in text.lines().enumerate() {
                if line.contains(query) {
                    found.push((path.clone(), i + 1, line.chars().take(160).collect()));
                    if found.len() >= 100 {
                        return;
                    }
                }
            }
        }
    }
}
fn top_level(paths: &[PathBuf]) -> Vec<PathBuf> {
    paths
        .iter()
        .filter(|p| {
            !paths
                .iter()
                .any(|parent| parent != *p && p.starts_with(parent))
        })
        .cloned()
        .collect()
}
fn moved(from: &Path, to: &Path) {
    js(format!(
        "window.dadoWorkbench?.movedPath({},{});",
        json!(from),
        json!(to)
    ));
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_selection_and_invalid_names() {
        let a = PathBuf::from("C:/project/src");
        let child = a.join("main.rs");
        assert_eq!(top_level(&[child, a.clone()]), vec![a]);
        for bad in ["", "..", "a/b", "a\\b", "C:other"] {
            assert!(name(bad).is_err());
        }
    }
    #[test]
    fn copying_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        let dest = dir.path().join("dest");
        std::fs::write(&source, "new").unwrap();
        std::fs::write(&dest, "existing").unwrap();
        assert!(copy(&source, &dest).is_err());
        assert_eq!(std::fs::read_to_string(dest).unwrap(), "existing");
    }
}
