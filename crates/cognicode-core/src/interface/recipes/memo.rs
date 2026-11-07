//! El memo de una receta: lo que ya se calculó y sigue siendo verdad.
//!
//! ## La clave
//!
//! Un paso se memoiza por **tres** cosas:
//!
//! - el **goal** que se corrió,
//! - los **argumentos** con los que se corrió,
//! - el **hash del árbol** sobre el que se corrió.
//!
//! El hash del árbol es el `source_digest` del manifiesto de fuentes: la
//! misma autoridad que decide si una instantánea sigue siendo fresca. Un
//! fichero que cambió con el reloj intacto cambia el hash, así que el memo no
//! sirve una respuesta de un árbol que ya no existe. Un memo por `(goal,
//! args)` sin el hash serviría la respuesta de un árbol distinto, y esa
//! respuesta sería mentira.
//!
//! ## Dónde vive
//!
//! En la caché del usuario, junto al resto de lo que el producto deriva:
//! `<cache root>/cognicode/memo/<clave del workspace>/<clave del paso>.json`.
//! Nunca dentro del árbol analizado: correr una receta es una lectura, y una
//! lectura que deja un fichero detrás no es una lectura.
//!
//! ## La escritura
//!
//! Con el escritor atómico del core: el temporal se crea en exclusiva, se
//! escribe, se sincroniza, y sólo entonces el cambio de nombre lo publica.
//! Un memo a medio escribir no es un memo: es un memo que dice algo que no
//! se calculó.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::application::graph_cache;
use crate::application::services::atomic_file_writer::AtomicFileWriter;

/// Una entrada del memo: lo que se calculó, y lo que hay que comprobar para
/// que siga valiendo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoEntry {
    /// El goal que se corrió.
    pub goal: String,
    /// Los argumentos con los que se corrió.
    pub args: Map<String, Value>,
    /// El hash del árbol sobre el que se corrió.
    pub tree: String,
    /// La respuesta del goal.
    pub value: Value,
    /// Cuánto tardó, en milisegundos.
    pub duration_ms: u64,
}

/// El memo de un workspace.
#[derive(Debug, Clone)]
pub struct MemoStore {
    root: PathBuf,
}

impl MemoStore {
    /// El memo de `workspace`, dentro de la caché que se le pasó.
    pub fn new(cache_root: &Path, workspace: &Path) -> Self {
        Self {
            root: graph_cache::cache_dir(cache_root)
                .join("memo")
                .join(graph_cache::workspace_key(workspace)),
        }
    }

    /// El directorio donde vive el memo de este workspace.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Lo que el memo dice de un paso, si lo dice.
    ///
    /// `None` cuando no hay memo, cuando el memo no se puede leer, o cuando
    /// lo que dice no encaja con lo que se pide: una entrada que no encaja es
    /// un fallo, no un acierto.
    pub fn lookup(&self, goal: &str, args: &Map<String, Value>, tree: &str) -> Option<MemoEntry> {
        let path = self.path_of(&memo_key(goal, args, tree));
        let text = std::fs::read_to_string(path).ok()?;
        let entry: MemoEntry = serde_json::from_str(&text).ok()?;
        if entry.goal != goal
            || entry.tree != tree
            || canonical_args(&entry.args) != canonical_args(args)
        {
            return None;
        }
        Some(entry)
    }

    /// Guarda lo que un paso respondió.
    pub fn store(
        &self,
        goal: &str,
        args: &Map<String, Value>,
        tree: &str,
        value: &Value,
        duration_ms: u64,
    ) -> Result<(), String> {
        let entry = MemoEntry {
            goal: goal.to_string(),
            args: args.clone(),
            tree: tree.to_string(),
            value: value.clone(),
            duration_ms,
        };
        let bytes = serde_json::to_vec(&entry)
            .map_err(|error| format!("the memo entry is not serialisable: {error}"))?;
        std::fs::create_dir_all(&self.root)
            .map_err(|error| format!("the memo directory cannot be created: {error}"))?;
        let path = self.path_of(&memo_key(goal, args, tree));
        let mut writer = AtomicFileWriter::create(&path)
            .map_err(|error| format!("the memo entry cannot be written: {error}"))?;
        writer
            .write(&bytes)
            .map_err(|error| format!("the memo entry cannot be written: {error}"))?;
        writer
            .commit()
            .map_err(|error| format!("the memo entry cannot be published: {error}"))
    }

    /// El fichero donde vive una clave.
    fn path_of(&self, key: &str) -> PathBuf {
        self.root.join(format!("{key}.json"))
    }
}

/// La clave de un paso: `(goal, args, hash del árbol)`.
///
/// Es un hash, así que dos claves distintas pueden dar el mismo nombre: por
/// eso la entrada guarda las tres partes y al leer se comprueban. Un nombre
/// que no encaja es un fallo, no un acierto.
pub fn memo_key(goal: &str, args: &Map<String, Value>, tree: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(goal.as_bytes());
    hasher.update(b"\0");
    hasher.update(canonical_args(args).as_bytes());
    hasher.update(b"\0");
    hasher.update(tree.as_bytes());
    crate::application::graph_snapshot::hex_digest(&hasher.finalize())
}

/// Los argumentos de un paso, en una forma que no depende del orden.
///
/// El orden de las claves de un mapa no es un hecho —dos mapas con las mismas
/// entradas pueden escribirse en otro orden—, así que la clave del memo no
/// puede depender de él. Aquí las claves se ordenan y los valores se
/// canonizan igual: los mismos argumentos dan siempre la misma clave.
pub fn canonical_args(args: &Map<String, Value>) -> String {
    let mut names: Vec<&String> = args.keys().collect();
    names.sort();
    let mut text = String::from("{");
    for (index, name) in names.iter().enumerate() {
        if index > 0 {
            text.push(',');
        }
        text.push_str(&serde_json::to_string(name).unwrap_or_default());
        text.push(':');
        text.push_str(&canonical_value(&args[*name]));
    }
    text.push('}');
    text
}

/// Un valor, en una forma que no depende del orden de sus claves.
fn canonical_value(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let mut names: Vec<&String> = map.keys().collect();
            names.sort();
            let mut text = String::from("{");
            for (index, name) in names.iter().enumerate() {
                if index > 0 {
                    text.push(',');
                }
                text.push_str(&serde_json::to_string(name).unwrap_or_default());
                text.push(':');
                text.push_str(&canonical_value(&map[*name]));
            }
            text.push('}');
            text
        }
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(pairs: &[(&str, &str)]) -> Map<String, Value> {
        pairs
            .iter()
            .map(|(name, value)| (name.to_string(), Value::String(value.to_string())))
            .collect()
    }

    #[test]
    fn the_same_goal_the_same_args_and_the_same_tree_are_the_same_key() {
        let one = memo_key("find", &args(&[("symbol", "helper")]), "tree");
        let other = memo_key("find", &args(&[("symbol", "helper")]), "tree");

        assert_eq!(one, other);
    }

    #[test]
    fn the_order_of_the_arguments_does_not_change_the_key() {
        let mut one = Map::new();
        one.insert("symbol".to_string(), Value::String("helper".to_string()));
        one.insert("direction".to_string(), Value::String("both".to_string()));
        let mut other = Map::new();
        other.insert("direction".to_string(), Value::String("both".to_string()));
        other.insert("symbol".to_string(), Value::String("helper".to_string()));

        assert_eq!(
            memo_key("find", &one, "tree"),
            memo_key("find", &other, "tree")
        );
    }

    #[test]
    fn a_different_goal_a_different_args_or_a_different_tree_are_different_keys() {
        let base = memo_key("find", &args(&[("symbol", "helper")]), "tree");

        assert_ne!(
            base,
            memo_key("trace", &args(&[("symbol", "helper")]), "tree")
        );
        assert_ne!(base, memo_key("find", &args(&[("symbol", "otro")]), "tree"));
        assert_ne!(
            base,
            memo_key("find", &args(&[("symbol", "helper")]), "otro")
        );
    }

    #[test]
    fn the_memo_serves_what_it_stored_and_nothing_else() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = dir.path().join("ws");
        std::fs::create_dir_all(&workspace).expect("the workspace is created");
        let store = MemoStore::new(&dir.path().join("cache"), &workspace);
        let arguments = args(&[("symbol", "helper")]);

        store
            .store("find", &arguments, "tree", &Value::Bool(true), 12)
            .expect("the memo is written");

        let hit = store
            .lookup("find", &arguments, "tree")
            .expect("the memo answers");
        assert_eq!(hit.value, Value::Bool(true));
        assert_eq!(hit.duration_ms, 12);
        assert!(store.lookup("find", &arguments, "otro").is_none());
    }

    #[test]
    fn the_memo_does_not_live_inside_the_analysed_tree() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = dir.path().join("ws");
        std::fs::create_dir_all(&workspace).expect("the workspace is created");
        let store = MemoStore::new(&dir.path().join("cache"), &workspace);

        assert!(
            !store.root().starts_with(&workspace),
            "the memo lives in the cache, not in the tree: {:?}",
            store.root()
        );
    }
}
