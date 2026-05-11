//! Entity-Relationship model types for database schema diagrams

use serde::{Deserialize, Serialize};

/// ER model representing database schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErModel {
    pub entities: Vec<Entity>,
    pub relationships: Vec<ErRelationship>,
}

impl ErModel {
    pub fn empty() -> Self {
        Self { entities: vec![], relationships: vec![] }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub name: String,
    pub columns: Vec<Column>,
    pub primary_key: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Column {
    pub name: String,
    pub sql_type: SqlType,
    pub nullable: bool,
    pub constraints: Vec<Constraint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SqlType {
    Integer,
    BigInt,
    Serial,
    BigSerial,
    Varchar(Option<usize>),
    Text,
    Boolean,
    Timestamp,
    TimestampTz,
    Date,
    Uuid,
    Decimal(Option<(usize, usize)>),
    Json,
    Jsonb,
    Bytes,
    Float,
    Double,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Constraint {
    Unique,
    Default(String),
    Check(String),
    NotNull,
    PrimaryKey,
    ForeignKey(ForeignKey),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForeignKey {
    pub columns: Vec<String>,
    pub ref_table: String,
    pub ref_columns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Cardinality {
    OneToOne,
    OneToMany,
    ManyToMany,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErRelationship {
    pub source: String,
    pub target: String,
    pub cardinality: Cardinality,
    pub label: Option<String>,
}