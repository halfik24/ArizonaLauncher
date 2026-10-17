//! Разбор game.json: дерево -> плоский список записей.
//! Формат повторяет лаунчерный (main.js, модули 5878/4237):
//! type: dir | file | check | res | delete, плюс name, install_as, size,
//! hash (md5), date_change, variant_tag, вложенные data.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Root {
    pub data: Vec<Node>,
}

#[derive(Debug, Deserialize)]
pub struct Node {
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub name: Option<String>,
    pub install_as: Option<String>,
    pub size: Option<u64>,
    pub hash: Option<String>,
    pub date_change: Option<i64>,
    pub variant_tag: Option<String>,
    pub data: Option<Vec<Node>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Dir,
    File,
    Check,
    Delete,
}

#[derive(Debug, Clone)]
pub struct Entry {
    /// Относительный путь, разделитель `/`
    pub rel: String,
    pub kind: Kind,
    pub size: u64,
    pub hash: String,
    /// unix seconds из date_change
    pub date: i64,
    /// запись из секции `res` (лаунчер качает её только при починке)
    pub res: bool,
}

#[derive(Debug, Default)]
pub struct Flat {
    pub entries: Vec<Entry>,
    /// сколько файлов (не dir) и их суммарный размер, без res
    pub files: usize,
    pub bytes: u64,
    /// сколько узлов пропущено из-за variant_tag
    pub skipped_variant: usize,
    /// сколько записей относится к res
    pub res_entries: usize,
}

pub fn flatten(root: &Root, future: bool) -> Flat {
    let want = if future { "future" } else { "default" };
    let mut out = Flat::default();
    walk(&root.data, "", want, false, &mut out);
    for e in &out.entries {
        if e.res {
            out.res_entries += 1;
            continue;
        }
        if e.kind == Kind::Dir {
            continue;
        }
        out.files += 1;
        out.bytes += e.size;
    }
    out
}

fn join(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}/{name}")
    }
}

fn walk(nodes: &[Node], prefix: &str, want: &str, in_res: bool, out: &mut Flat) {
    for n in nodes {
        // variant_tag: при включённой «новой графике» берём future, иначе default
        if let Some(v) = &n.variant_tag {
            if v != want {
                out.skipped_variant += 1;
                continue;
            }
        }
        let is_res = in_res || n.kind.as_deref() == Some("res");
        let name = n.install_as.clone().or_else(|| n.name.clone());
        let rel = match &name {
            Some(nm) => join(prefix, nm),
            None => prefix.to_string(),
        };
        match n.kind.as_deref() {
            // узла без type может быть data (просто контейнер)
            None => {
                if let Some(d) = &n.data {
                    walk(d, prefix, want, is_res, out);
                }
            }
            Some("dir") => {
                out.entries.push(Entry {
                    rel: rel.clone(),
                    kind: Kind::Dir,
                    size: 0,
                    hash: String::new(),
                    date: 0,
                    res: is_res,
                });
                if let Some(d) = &n.data {
                    walk(d, &rel, want, is_res, out);
                }
            }
            Some("delete") => out.entries.push(Entry {
                rel,
                kind: Kind::Delete,
                size: 0,
                hash: String::new(),
                date: n.date_change.unwrap_or(0),
                res: is_res,
            }),
            Some(k) => {
                let kind = if k == "check" { Kind::Check } else { Kind::File };
                out.entries.push(Entry {
                    rel: rel.clone(),
                    kind,
                    size: n.size.unwrap_or(0),
                    hash: n.hash.clone().unwrap_or_default(),
                    date: n.date_change.unwrap_or(0),
                    res: is_res,
                });
                if let Some(d) = &n.data {
                    walk(d, &rel, want, is_res, out);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flatten_walks_tree_variant_and_res() {
        let json = r#"{
            "data": [
                {"type":"dir","name":"models","data":[
                    {"type":"file","name":"trees.img_default","install_as":"trees.img",
                     "size":9,"hash":"aaa","date_change":100,"variant_tag":"default"},
                    {"type":"file","name":"trees.img_future","install_as":"trees.img",
                     "size":24,"hash":"bbb","date_change":100,"variant_tag":"future"}
                ]},
                {"type":"res","name":"creaton","data":[
                    {"type":"file","name":"c.ini","size":3,"hash":"ccc","date_change":5}
                ]},
                {"type":"delete","name":"old.img","date_change":7},
                {"type":"check","name":"gta_sa.exe","size":10,"hash":"ddd","date_change":8}
            ]
        }"#;
        let root: Root = serde_json::from_str(json).unwrap();

        // future-вариант: default-узел пропущен
        let flat = flatten(&root, true);
        assert_eq!(flat.skipped_variant, 1);
        assert_eq!(flat.res_entries, 2); // узел res + файл внутри
        let trees: Vec<_> = flat
            .entries
            .iter()
            .filter(|e| e.rel == "models/trees.img")
            .collect();
        assert_eq!(trees.len(), 1);
        assert_eq!(trees[0].hash, "bbb");
        // файл внутри res помечен и не входит в счётчик обычных файлов.
        // delete-узлы считаются файлами с size 0 (как у оригинала).
        assert_eq!(flat.files, 3); // trees.img + old.img(delete) + gta_sa.exe
        assert_eq!(flat.bytes, 34);

        // default-вариант: берётся default-узел
        let flat = flatten(&root, false);
        let trees: Vec<_> = flat
            .entries
            .iter()
            .filter(|e| e.rel == "models/trees.img")
            .collect();
        assert_eq!(trees[0].hash, "aaa");
        assert_eq!(flat.skipped_variant, 1);
    }
}
