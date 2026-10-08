use std::path::{Path, PathBuf};

const NODE_LIMIT: usize = 2000;

#[derive(Clone, Debug, serde::Serialize)]
pub struct Node {
    pub path: PathBuf,
    pub label: String,
    pub directory: bool,
    pub has_children: bool,
    pub parent: Option<usize>,
    pub position: [f64; 3],
}

#[derive(Clone, Default, serde::Serialize)]
pub struct Tree {
    pub nodes: Vec<Node>,
    pub notices: Vec<String>,
}

impl Tree {
    pub fn read(path: &Path, overview: bool) -> Self {
        let mut tree = Self::default();
        tree.visit(path, None, 0, overview);
        if !path.is_dir() {
            tree.notices
                .push("This folder is no longer available.".into());
        }
        let mut cursor = 0.0;
        tree.layout(0, 0, &mut cursor);
        let center = tree.nodes[0].position[0];
        let width = cursor.max(1.0);
        for node in &mut tree.nodes {
            node.position[0] = (node.position[0] - center) * (860.0 / width).min(135.0);
        }
        tree.notices.sort();
        tree.notices.dedup();
        tree
    }

    fn visit(&mut self, path: &Path, parent: Option<usize>, depth: usize, overview: bool) {
        let directory = path.is_dir() && !path.is_symlink();
        let index = self.nodes.len();
        self.nodes.push(Node {
            path: path.to_path_buf(),
            label: path
                .file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into(),
            directory,
            has_children: false,
            parent,
            position: [0.0; 3],
        });
        if !directory {
            return;
        }
        if !overview && depth > 0 {
            match std::fs::read_dir(path) {
                Ok(mut entries) => self.nodes[index].has_children = entries.next().is_some(),
                Err(_) => self
                    .notices
                    .push(format!("Could not read {}", path.display())),
            }
            return;
        }
        if depth >= 64 {
            self.notices
                .push("Overview limited to 64 folder levels.".into());
            return;
        }
        let entries = match std::fs::read_dir(path) {
            Ok(entries) => entries,
            Err(_) => {
                self.notices
                    .push(format!("Could not read {}", path.display()));
                return;
            }
        };
        let mut paths = Vec::new();
        for entry in entries {
            match entry {
                Ok(entry) => paths.push(entry.path()),
                Err(_) => self.notices.push(format!(
                    "Some entries in {} could not be read.",
                    path.display()
                )),
            }
        }
        paths.sort_by_key(|p| {
            (
                !p.is_dir(),
                p.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase(),
            )
        });
        self.nodes[index].has_children = !paths.is_empty();
        for child in paths {
            if self.nodes.len() >= NODE_LIMIT {
                self.notices.push(format!(
                    "Showing the first {NODE_LIMIT} nodes. Enter a folder to explore further."
                ));
                break;
            }
            self.visit(&child, Some(index), depth + 1, overview);
        }
    }

    fn layout(&mut self, index: usize, depth: usize, cursor: &mut f64) -> f64 {
        let children: Vec<_> = self
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(i, n)| (n.parent == Some(index)).then_some(i))
            .collect();
        let x = if children.is_empty() {
            let x = *cursor;
            *cursor += 1.0;
            x
        } else {
            let positions: Vec<_> = children
                .into_iter()
                .map(|i| self.layout(i, depth + 1, cursor))
                .collect();
            (positions[0] + positions[positions.len() - 1]) / 2.0
        };
        // Vertical levels encode hierarchy; depth adds space without changing which level is above.
        let phase = index as f64 * 2.399963;
        self.nodes[index].position = [
            x,
            depth as f64 * 190.0
                + if depth == 0 {
                    0.0
                } else {
                    (index % 4) as f64 * 26.0 + phase.sin() * 10.0
                },
            if depth == 0 { 0.0 } else { phase.cos() * 105.0 },
        ];
        x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focused_and_overview_preserve_hierarchy() {
        let root = std::env::temp_dir().join(format!("dado-tree-test-{}", std::process::id()));
        std::fs::create_dir_all(root.join("folder/nested")).unwrap();
        std::fs::write(root.join("folder/file.txt"), "hello").unwrap();
        std::fs::write(root.join("root.txt"), "hello").unwrap();
        let focused = Tree::read(&root, false);
        assert_eq!(focused.nodes.len(), 3);
        assert!(
            focused
                .nodes
                .iter()
                .find(|n| n.directory && n.parent == Some(0))
                .unwrap()
                .has_children
        );
        assert!(focused.nodes.iter().skip(1).all(|n| n.parent == Some(0)));
        let overview = Tree::read(&root, true);
        assert_eq!(overview.nodes.len(), 5);
        for node in overview.nodes.iter().skip(1) {
            assert!(node.position[1] > overview.nodes[node.parent.unwrap()].position[1]);
        }
        assert_eq!(overview.nodes[0].position, [0.0, 0.0, 0.0]);
        // Remove only the explicitly created fixture, after confirming its parent and prefix.
        assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
        assert!(
            root.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("dado-tree-test-")
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
