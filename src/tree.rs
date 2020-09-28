use std::collections::HashMap;
use std::path::{Path, PathBuf, MAIN_SEPARATOR};

use lazy_static::lazy_static;

lazy_static! {
    static ref SEP: String = MAIN_SEPARATOR.to_string();
}

#[derive(Debug)]
pub struct Tree {
    root: Node,
}

impl Tree {
    pub fn new() -> Self {
        Tree {
            root: Node::new("".to_string(), false, true),
        }
    }

    /// Adds a path to the tree
    pub fn add<P: AsRef<Path>>(&mut self, path: P, is_dir: bool) {
        // FIXME: ugly and not fully correct.
        // We should work with paths and Components instead of strings!
        let path = path.as_ref().to_str().unwrap();

        let path_parts: Vec<_> = path.split(&*SEP).collect();
        let part_count = path_parts.len();

        let mut current_node = &mut self.root;
        for (index, mut part) in path_parts.into_iter().enumerate() {
            let is_real = index == part_count - 1;

            // Root (FIXME: not Windows-friendly)
            if part == "" {
                part = "/";
            }

            if !current_node.nodes.contains_key(part) {
                let child_node = Node::new(part.to_string(), is_real, true);
                current_node.nodes.insert(part.to_string(), child_node);
            }
            // Safe to unwrap, since we just added it
            let child_node = current_node.nodes.get_mut(part).unwrap();
            if is_real && !child_node.is_real {
                child_node.is_real = true;
            }

            current_node.is_dir = true;
            current_node = child_node;
        }

        current_node.is_dir = is_dir;
    }

    /// Returns the set of paths in the tree
    pub fn paths(&self) -> Vec<PathBuf> {
        let mut results = vec![];
        self.root.paths(&mut results, "".to_string());
        results.sort();
        results.into_iter().map(PathBuf::from).collect()
    }

    /// Builds a top-level tree
    pub fn top_level(&self) -> Tree {
        let mut result_tree = Tree::new();
        self.root.find_top_level(&mut result_tree.root);
        result_tree
    }
}

#[derive(Debug)]
struct Node {
    name: String,
    nodes: HashMap<String, Node>,
    is_real: bool,
    is_dir: bool,
}

impl Node {
    fn new(name: String, is_real: bool, is_dir: bool) -> Self {
        Node {
            name,
            nodes: HashMap::new(),
            is_real,
            is_dir,
        }
    }

    fn paths(&self, results: &mut Vec<String>, prefix: String) {
        let new_prefix = if prefix == "" {
            self.name.clone()
        } else if prefix == "/" {
            // FIXME: not Windows-friendly
            prefix + &self.name
        } else {
            prefix + &SEP + &self.name
        };

        if self.is_real {
            results.push(new_prefix.clone());
        }

        for child_node in self.nodes.values() {
            child_node.paths(results, new_prefix.clone());
        }
    }

    fn find_top_level(&self, result_node: &mut Node) {
        result_node.is_real = self.is_real;
        if self.is_real {
            return;
        }

        for child_node in self.nodes.values() {
            let result_child_node = Node::new(child_node.name.clone(), false, child_node.is_dir);
            result_node
                .nodes
                .insert(child_node.name.clone(), result_child_node);

            // Safe to unwrap, since we just added it
            let mut result_child_node = result_node.nodes.get_mut(&child_node.name).unwrap();
            child_node.find_top_level(&mut result_child_node);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_same_paths(actual: Vec<PathBuf>, expected_str: Vec<&str>) {
        let expected_paths: Vec<_> = expected_str.into_iter().map(|s| PathBuf::from(s)).collect();
        assert_eq!(actual, expected_paths);
    }

    #[test]
    fn test_paths() {
        let mut tree = Tree::new();
        tree.add("/a/b/c", false);
        tree.add("/a/b/d", false);
        tree.add("/a/b", true);
        tree.add("/a/b/e", false);
        tree.add("/a/f", false);
        tree.add("/a/b", true);
        tree.add("/j/k/l", false);
        tree.add("/j/k/m", false);

        let paths = tree.paths();
        let expected = vec![
            "/a/b", "/a/b/c", "/a/b/d", "/a/b/e", "/a/f", "/j/k/l", "/j/k/m",
        ];
        assert_same_paths(paths, expected);
    }

    #[test]
    fn test_top_level() {
        let mut tree = Tree::new();
        tree.add("/a/b/c", false);
        tree.add("/a/b/d", false);
        tree.add("/a/b", true);
        tree.add("/a/b/e", false);
        tree.add("/a/f", true);
        tree.add("/a/b", true);
        tree.add("/j/k/l", false);
        tree.add("/j/k/m", false);

        let paths = tree.top_level().paths();
        let expected = vec!["/a/b", "/a/f", "/j/k/l", "/j/k/m"];
        assert_same_paths(paths, expected);
    }
}
