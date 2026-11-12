//! Il protocollo fra l'app e l'helper root: una riga JSON per richiesta, una
//! per risposta, sul socket privato che l'helper eredita come fd 3 (nessun
//! file nel filesystem: nessun altro processo ci può parlare).

use serde::{Deserialize, Serialize};

/// La prima riga che l'helper scrive quando è pronto.
pub const HELLO: &str = "rootree-helper 1";

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    ListDir { path: String, hidden: bool, key: u8, desc: bool, dirs_first: bool },
    Stat { path: String },
    ReadText { path: String, max: u64 },
    WriteText { path: String, text: String },
    MakeDir { parent: String, name: String },
    MakeFile { parent: String, name: String },
    /// il cestino di root (.Trash-0) della memoria di `scope`
    TrashOp { scope: String, action: String, item: String, days: u32 },
    Rename { path: String, name: String },
    SetMode { path: String, mode: u32 },
    SetOwner { path: String, user: String, group: String },
    JobCopy { sources: Vec<String>, dest: String },
    JobMove { sources: Vec<String>, dest: String },
    JobDelete { paths: Vec<String> },
    JobTrash { paths: Vec<String>, days: u32 },
    JobSize { paths: Vec<String> },
    JobExtract { archive: String, dest: String },
    JobCompress { sources: Vec<String>, dest: String },
    JobStatuses,
    JobCancel { id: u64 },
    JobResolve { id: u64, action: u8, all: bool },
    JobForget { id: u64 },
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Response {
    /// vuoto = riuscita
    #[serde(default)]
    pub error: String,
    #[serde(default)]
    pub value: serde_json::Value,
}

impl Response {
    pub fn ok(v: impl Serialize) -> Self {
        Response { error: String::new(), value: serde_json::to_value(v).unwrap_or_default() }
    }
    pub fn err(e: impl std::fmt::Display) -> Self {
        Response { error: e.to_string(), value: serde_json::Value::Null }
    }
    pub fn from<T: Serialize, E: std::fmt::Display>(r: Result<T, E>) -> Self {
        match r {
            Ok(v) => Self::ok(v),
            Err(e) => Self::err(e),
        }
    }
}
