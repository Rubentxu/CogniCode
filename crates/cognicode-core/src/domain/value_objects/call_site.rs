//! A call, as the reader saw it, before anyone tried to resolve it.
//!
//! A call is a fact about what the code says: a declaration holds it, a
//! file writes it, a line carries it, and it names something. What it
//! names may still turn out to be one declaration, several of them, or
//! none — so a call is not a relation yet, and it travels as its own
//! value until the resolution decides what it becomes.
//!
//! It exists because the identity of a call is not recoverable from the
//! name it names. Two calls in the same file can name the same thing, and
//! a consumer that is told "a call could not be tied" without knowing
//! which one cannot do anything with the fact.

use crate::domain::aggregates::call_graph::SymbolId;

/// A call, as the reader saw it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallSite {
    /// The declaration that holds the call.
    pub source: SymbolId,
    /// The file that writes the call.
    pub origin_file: String,
    /// The line the call is written on, counting from one.
    pub line: u32,
    /// The name the call names, as it was written.
    pub target: String,
    /// The type the call is qualified by, when the path spells one:
    /// `Duration` in `Duration::from_millis(10)`.
    ///
    /// `target` keeps the last segment of the path, so on its own it cannot
    /// tell the project's `EventTime::from_millis` from the standard
    /// library's `Duration::from_millis`. The qualifier is what can, and a
    /// resolver that finds no declaration of it in the project knows the call
    /// is somebody else's. `None` when the call is written without a path,
    /// through a value, or in a language whose spelling does not say which
    /// segment is a type.
    pub type_qualifier: Option<String>,
    /// The head of the path the call is written through: the crate, module
    /// or package the path starts at, `serde_json` in `serde_json::to_value(1)`.
    ///
    /// The same name can belong to the project and to a crate the project
    /// pulls: `to_value` is the project's own function and `serde_json`'s,
    /// and the last segment of the path cannot tell them apart. The head of
    /// the path can — a package that pulls `serde_json` from a registry does
    /// not own what that crate calls, so a call written through it is not a
    /// call to the project's declaration of the same name.
    ///
    /// `None` when the call is written without a path, through a value, or
    /// through a type: `Duration::from_millis` starts with a type, and the
    /// type qualifier carries that call, not this.
    pub path_head: Option<String>,
    /// What the file says about the receiver of the call.
    ///
    /// A call written on a value (`bytes.as_bytes()`) names a method of the
    /// receiver's type, and the text of the call does not carry that type:
    /// the file is the one that knows it. The judgment of the call depends
    /// on it — a receiver whose type belongs to somebody else is not a call
    /// to the project's declaration of the same name — and it is made here,
    /// in the file that knows, so that every route answers the same
    /// question about the same call.
    ///
    /// `None` when the call is not written on a value: a call through a path
    /// names what the path names, and the path is the qualifier's business.
    pub receiver: Option<Receiver>,
}

/// What a file says about the receiver of a call.
///
/// A call written on a value names a method of the receiver's type, and
/// the type is the file's to say: the `impl` that holds `self`, the type a
/// parameter declares, the type a local declares, the type a constructor
/// constructs. What the file cannot say, nobody says.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Receiver {
    /// The file does not state the type of the receiver: the call is
    /// uncertain, and the resolver keeps the answer it had.
    ///
    /// The receiver is a field (`session.config.path()`), the result of a
    /// call (`Workspace::new().build()`), or a name the file binds without
    /// a type. The graph cannot say whose method the call names, and it
    /// does not guess: it publishes the call as the project's, which is
    /// what it has always published, and the count of these is published
    /// with it.
    Uncertain,
    /// The receiver's type is the project's own.
    ///
    /// The file declares it, or it imports it from the project's own root
    /// (`crate`, `self`, `super`), or it is written through a path that
    /// starts there.
    Project,
    /// The receiver's type belongs to the crate, module or package named
    /// here.
    ///
    /// The file imports the type from it, so the file does not own it.
    /// Whether it is outside the project is the manifest's question, the
    /// same one the head of a call's path is judged by.
    Crate(String),
    /// The receiver's type is a name the file states and does not bind.
    ///
    /// In a language where a type must be named to be used, a name the file
    /// does not bind is the standard library's or a dependency's: the
    /// prelude's `Vec`, or a type the file never imports.
    Unbound,
}
