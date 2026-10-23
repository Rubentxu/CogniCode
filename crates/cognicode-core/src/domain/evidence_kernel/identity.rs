//! The identity vocabulary: the shapes the product writes when it names a
//! thing, and the one place that reads them back.
//!
//! A symbol names itself `"{file}:{name}:{line}"`, where the trailing
//! segment is a line number. The readers of infrastructure files name what
//! they find differently: a Terraform resource is `"{file}:{address}"` behind
//! a `tf:` prefix, and an Ansible declaration is `"{file}:{kind}:{name}"`
//! behind an `ansible:` prefix. Those identities carry a file and a name,
//! and no line: the line is where the thing was found, not what it is.
//!
//! Reading an identity is not something each consumer should improvise. A
//! consumer that splits the string by hand answers a different question
//! depending on who wrote the identity, and the answer it invents can be a
//! file name where a symbol name was asked for. This module declares the
//! vocabulary and reads it, so every consumer answers the same question the
//! same way — and a consumer that meets an identity it cannot read says so
//! instead of inventing a file or a line.

use super::symbol_fqn::SymbolFqn;

/// A thing the product names, read back from the identity it wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Identity {
    /// A symbol that lives at a line.
    Symbol(SymbolFqn),
    /// A declaration a reader named: a Terraform resource, an Ansible play.
    /// It has a file and a name, and no line.
    Declaration {
        /// The grammar that wrote it.
        grammar: DeclarationGrammar,
        /// The file the declaration lives in.
        file: String,
        /// The name the declaration carries.
        name: String,
    },
}

/// The grammar a declaration was written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclarationGrammar {
    /// A Terraform resource, written as `"{file}:{address}"` behind `tf:`.
    Terraform,
    /// An Ansible declaration, written as `"{file}:{kind}:{name}"` behind
    /// `ansible:`.
    Ansible,
}

impl Identity {
    /// Reads an identity the product wrote.
    ///
    /// The prefixes are read first, because a declaration may end in
    /// something that looks like a line number while a symbol never carries
    /// a prefix. An identity in no known grammar reads as `None`: a consumer
    /// that cannot read it says so instead of inventing a file or a line.
    pub fn parse(identity: &str) -> Option<Self> {
        if let Some(rest) = identity.strip_prefix("tf:") {
            let (file, address) = rest.rsplit_once(':')?;
            return Some(Self::Declaration {
                grammar: DeclarationGrammar::Terraform,
                file: file.to_string(),
                name: address.to_string(),
            });
        }
        if let Some(rest) = identity.strip_prefix("ansible:") {
            let (file_and_kind, name) = rest.rsplit_once(':')?;
            let (file, _kind) = file_and_kind.rsplit_once(':')?;
            return Some(Self::Declaration {
                grammar: DeclarationGrammar::Ansible,
                file: file.to_string(),
                name: name.to_string(),
            });
        }
        SymbolFqn::parse(identity).map(Self::Symbol)
    }

    /// The name of the thing, as its identity spells it.
    pub fn name(&self) -> &str {
        match self {
            Self::Symbol(fqn) => fqn.name(),
            Self::Declaration { name, .. } => name,
        }
    }

    /// The file the thing lives in.
    pub fn file(&self) -> &str {
        match self {
            Self::Symbol(fqn) => fqn.file(),
            Self::Declaration { file, .. } => file,
        }
    }

    /// The line the thing lives at, when its grammar has one. A declaration
    /// has none: the line is where it was found, not what it is.
    pub fn line(&self) -> Option<u32> {
        match self {
            Self::Symbol(fqn) => Some(fqn.line()),
            Self::Declaration { .. } => None,
        }
    }

    /// The grammar a declaration was written in, when the identity is a
    /// declaration. A symbol was written in the symbol grammar.
    pub fn declaration_grammar(&self) -> Option<DeclarationGrammar> {
        match self {
            Self::Symbol(_) => None,
            Self::Declaration { grammar, .. } => Some(*grammar),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_symbol_reads_its_file_name_and_line() {
        let identity = Identity::parse("src/lib.rs:helper:42").expect("the symbol grammar");
        assert_eq!(identity.file(), "src/lib.rs");
        assert_eq!(identity.name(), "helper");
        assert_eq!(identity.line(), Some(42));
    }

    #[test]
    fn a_terraform_resource_reads_its_file_and_address() {
        let identity =
            Identity::parse("tf:main.tf:aws_s3_bucket.app_bucket").expect("the resource grammar");
        assert_eq!(identity.file(), "main.tf");
        assert_eq!(identity.name(), "aws_s3_bucket.app_bucket");
        assert_eq!(identity.line(), None);
    }

    #[test]
    fn an_ansible_declaration_reads_its_file_and_name() {
        let identity = Identity::parse("ansible:site.yml:play:Configure_infrastructure")
            .expect("the declaration grammar");
        assert_eq!(identity.file(), "site.yml");
        assert_eq!(identity.name(), "Configure_infrastructure");
        assert_eq!(identity.line(), None);
    }

    #[test]
    fn a_file_path_with_separators_still_reconstructs() {
        let identity = Identity::parse("tf:infra/modules:vpc/main.tf:aws_vpc.main")
            .expect("the resource grammar");
        assert_eq!(identity.file(), "infra/modules:vpc/main.tf");
        assert_eq!(identity.name(), "aws_vpc.main");
    }

    #[test]
    fn an_identity_in_no_known_grammar_is_not_read() {
        assert!(Identity::parse("not an identity").is_none());
        assert!(Identity::parse("src/lib.rs:helper").is_none());
    }
}
