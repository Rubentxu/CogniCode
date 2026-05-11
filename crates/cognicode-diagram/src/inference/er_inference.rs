//! Entity-Relationship (ER) diagram inference from SQL schemas
//!
//! Scans for SQL files, Diesel schema.rs, and Prisma schema.prisma,
//! then parses them to extract entities, columns, and relationships.

use std::path::{Path, PathBuf};
use indexmap::IndexMap;
use regex::Regex;

use crate::model::er_types::{
    Cardinality, Column, Constraint, Entity, ErModel, ErRelationship, ForeignKey, SqlType,
};

/// Scan directories for SQL schema files
fn find_sql_files(project_dir: &Path) -> Vec<PathBuf> {
    let search_dirs = [
        "migrations",
        "db/migrations",
        "sql",
        "database",
        "db",
        "scripts",
    ];

    let mut sql_files = Vec::new();

    for dir_name in &search_dirs {
        let dir_path = project_dir.join(dir_name);
        if dir_path.exists() && dir_path.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&dir_path) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        if let Some(ext) = path.extension() {
                            let ext_str = ext.to_string_lossy().to_lowercase();
                            if ext_str == "sql" {
                                sql_files.push(path);
                            }
                        }
                    }
                }
            }
        }
    }

    // Also check for Diesel schema.rs
    let diesel_schema = project_dir.join("src").join("schema.rs");
    if diesel_schema.exists() {
        sql_files.push(diesel_schema);
    }

    // Also check for Prisma schema.prisma
    let prisma_schema = project_dir.join("prisma").join("schema.prisma");
    if prisma_schema.exists() {
        sql_files.push(prisma_schema);
    }

    sql_files
}

/// Infers ER diagram from SQL schema files.
///
/// Scans the project directory for SQL files in common locations:
/// - `migrations/`, `db/migrations/`, `sql/`, `database/`, `db/`, `scripts/`
/// - `src/schema.rs` (Diesel)
/// - `prisma/schema.prisma` (Prisma)
///
/// Parses CREATE TABLE statements to extract:
/// - Entities (tables)
/// - Columns (name, type, nullable, primary key)
/// - Relationships (foreign keys, many-to-many via join tables)
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use cognicode_diagram::inference::er_inference::infer_er_diagram;
/// use cognicode_diagram::render::er::render_er_mermaid;
///
/// # let project_dir = Path::new("/path/to/project");
/// let model = infer_er_diagram(project_dir)?;
/// let mermaid = render_er_mermaid(&model);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn infer_er_diagram(project_dir: &Path) -> anyhow::Result<ErModel> {
    let sql_files = find_sql_files(project_dir);

    if sql_files.is_empty() {
        return Ok(ErModel::empty());
    }

    let mut entities: Vec<Entity> = Vec::new();
    let mut relationships: Vec<ErRelationship> = Vec::new();

    for sql_file in &sql_files {
        let filename = sql_file.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        if filename.ends_with(".sql") {
            let content = std::fs::read_to_string(sql_file)?;
            let (file_entities, file_rels) = parse_sql_content(&content)?;
            entities.extend(file_entities);
            relationships.extend(file_rels);
        } else if filename == "schema.rs" {
            // Diesel schema
            let content = std::fs::read_to_string(sql_file)?;
            let (file_entities, file_rels) = parse_diesel_schema(&content)?;
            entities.extend(file_entities);
            relationships.extend(file_rels);
        } else if filename == "schema.prisma" {
            // Prisma schema
            let content = std::fs::read_to_string(sql_file)?;
            let (file_entities, file_rels) = parse_prisma_schema(&content)?;
            entities.extend(file_entities);
            relationships.extend(file_rels);
        }
    }

    // Deduplicate entities by name
    let mut unique_entities: IndexMap<String, Entity> = IndexMap::new();
    for entity in entities {
        if let Some(existing) = unique_entities.get_mut(&entity.name) {
            // Merge columns
            for col in entity.columns {
                if !existing.columns.iter().any(|c| c.name == col.name) {
                    existing.columns.push(col);
                }
            }
        } else {
            unique_entities.insert(entity.name.clone(), entity);
        }
    }

    // Deduplicate relationships
    let mut unique_rels: Vec<ErRelationship> = Vec::new();
    for rel in relationships {
        if !unique_rels.iter().any(|r| r.source == rel.source && r.target == rel.target) {
            unique_rels.push(rel);
        }
    }

    Ok(ErModel {
        entities: unique_entities.into_values().collect(),
        relationships: unique_rels,
    })
}

/// Parse SQL content and extract entities and relationships
fn parse_sql_content(content: &str) -> anyhow::Result<(Vec<Entity>, Vec<ErRelationship>)> {
    let mut entities: Vec<Entity> = Vec::new();
    let mut relationships: Vec<ErRelationship> = Vec::new();

    // Regex for CREATE TABLE statements
    let create_table_re = Regex::new(r"(?i)CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?([\w]+)\s*\(").unwrap();

    // Find all CREATE TABLE statements
    for cap in create_table_re.captures_iter(content) {
        if let Some(table_name) = cap.get(1) {
            let table_name = clean_identifier(table_name.as_str());
            let table_block = extract_table_block(content, table_name.as_str());

            if let Some((columns, pk, fks)) = parse_columns(&table_block) {
                // Detect join table: 2+ FKs and no PK (all columns are FK columns)
                let is_join_table = fks.len() >= 2 && pk.is_none();

                let entity = Entity {
                    name: table_name.clone(),
                    columns,
                    primary_key: pk,
                };
                entities.push(entity);

                if is_join_table {
                    // Create a single ManyToMany between the first two referenced tables
                    if fks.len() >= 2 {
                        let first_fk = &fks[0];
                        let second_fk = &fks[1];
                        relationships.push(ErRelationship {
                            source: first_fk.ref_table.clone(),
                            target: second_fk.ref_table.clone(),
                            cardinality: Cardinality::ManyToMany,
                            label: Some(format!("{} -> {}", first_fk.ref_table, second_fk.ref_table)),
                        });
                    }
                } else {
                    // Create relationships from foreign keys
                    for fk in fks {
                        let cardinality = determine_cardinality(&fk.columns, &fk.ref_columns);
                        relationships.push(ErRelationship {
                            source: table_name.clone(),
                            target: fk.ref_table.clone(),
                            cardinality,
                            label: Some(format!("{} -> {}", fk.columns.join(", "), fk.ref_columns.join(", "))),
                        });
                    }
                }
            }
        }
    }

    Ok((entities, relationships))
}

/// Extract the content between parentheses of a CREATE TABLE statement
fn extract_table_block(content: &str, table_name: &str) -> String {
    // Find the CREATE TABLE line
    let escaped_name = regex::escape(table_name);
    let create_pattern = format!("(?i)CREATE\\s+TABLE\\s+(?:IF\\s+NOT\\s+EXISTS\\s+)?{}\\s*\\(", escaped_name);

    if let Some(start_re) = Regex::new(&create_pattern).ok() {
        if let Some(start_match) = start_re.find(content) {
            // start_match.end() points right after the opening '('
            let start_pos = start_match.end();
            let mut paren_depth = 1; // We've already passed the opening '('
            let block_start = start_pos;
            let mut block_end = start_pos;

            for (i, c) in content[start_pos..].chars().enumerate() {
                match c {
                    '(' => {
                        paren_depth += 1;
                    }
                    ')' => {
                        paren_depth -= 1;
                        if paren_depth == 0 {
                            // Found the closing ')'
                            block_end = start_pos + i;
                            break;
                        }
                    }
                    _ => {}
                }
            }

            if block_end > block_start {
                return content[block_start..block_end].trim().to_string();
            }
        }
    }

    String::new()
}

/// Parse columns from a table definition block
fn parse_columns(block: &str) -> Option<(Vec<Column>, Option<Vec<String>>, Vec<ForeignKey>)> {
    let mut columns: Vec<Column> = Vec::new();
    let mut primary_key: Option<Vec<String>> = None;
    let mut foreign_keys: Vec<ForeignKey> = Vec::new();

    // Split by commas, but respect parentheses
    let lines = split_columns(block);

    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Check for PRIMARY KEY constraint
        let pk_re = Regex::new(r"(?i)^\s*PRIMARY\s+KEY\s*\(([^)]+)\)").unwrap();
        if let Some(pk_cap) = pk_re.captures(line) {
            if let Some(cols_str) = pk_cap.get(1) {
                let pk_cols: Vec<String> = cols_str.as_str()
                    .split(',')
                    .map(|s| clean_identifier(s.trim()))
                    .collect();
                primary_key = Some(pk_cols);
            }
            continue;
        }

        // Check for FOREIGN KEY constraint
        let fk_re = Regex::new(r"(?i)^\s*FOREIGN\s+KEY\s*\(([^)]+)\)\s*REFERENCES\s+([\w]+)\s*\(([^)]+)\)").unwrap();
        if let Some(fk_cap) = fk_re.captures(line) {
            if let (Some(cols_match), Some(table_match), Some(ref_cols_match)) =
                (fk_cap.get(1), fk_cap.get(2), fk_cap.get(3)) {
                let cols: Vec<String> = cols_match.as_str()
                    .split(',')
                    .map(|s| clean_identifier(s.trim()))
                    .collect();
                let ref_table = clean_identifier(table_match.as_str());
                let ref_cols: Vec<String> = ref_cols_match.as_str()
                    .split(',')
                    .map(|s| clean_identifier(s.trim()))
                    .collect();

                foreign_keys.push(ForeignKey {
                    columns: cols,
                    ref_table,
                    ref_columns: ref_cols,
                });
            }
            continue;
        }

        // Parse column definition
        if let Some((name, sql_type, nullable, constraints, inline_fk)) = parse_column_definition(line) {
            let col_constraints = constraints;

            // Check for primary key in column definition
            let is_pk = col_constraints.iter().any(|c| matches!(c, Constraint::PrimaryKey));
            if is_pk {
                if primary_key.is_none() {
                    primary_key = Some(vec![name.clone()]);
                }
            }

            // Handle inline FK if detected
            if let Some((ref_table, ref_columns)) = inline_fk {
                foreign_keys.push(ForeignKey {
                    columns: vec![name.clone()],
                    ref_table,
                    ref_columns,
                });
            }

            columns.push(Column {
                name,
                sql_type,
                nullable: nullable && !is_pk,
                constraints: col_constraints,
            });
        }
    }

    Some((columns, primary_key, foreign_keys))
}

/// Parse a single column definition
fn parse_column_definition(line: &str) -> Option<(String, SqlType, bool, Vec<Constraint>, Option<(String, Vec<String>)>)> {
    // Pattern: column_name TYPE [constraints]
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }

    let name = clean_identifier(parts[0]);
    let type_str = parts[1];
    let sql_type = parse_sql_type(type_str);

    let mut nullable = true;
    let mut constraints: Vec<Constraint> = Vec::new();
    let mut inline_fk: Option<(String, Vec<String>)> = None;

    // Check for inline REFERENCES pattern
    let refs_re = Regex::new(r"(?i)REFERENCES\s+([\w]+)\s*\(([^)]+)\)").unwrap();
    if let Some(refs_cap) = refs_re.captures(line) {
        if let (Some(table_match), Some(cols_match)) = (refs_cap.get(1), refs_cap.get(2)) {
            let ref_table = clean_identifier(table_match.as_str());
            let ref_columns: Vec<String> = cols_match.as_str()
                .split(',')
                .map(|s| clean_identifier(s.trim()))
                .collect();
            inline_fk = Some((ref_table, ref_columns));
        }
    }

    // Parse remaining parts as constraints
    for part in &parts[2..] {
        let part_upper = part.to_uppercase();
        match part_upper.as_str() {
            "NOT" => {
                nullable = false;
            }
            "NULL" => {
                nullable = true;
            }
            "PRIMARY" => {
                // Could be PRIMARY KEY
                constraints.push(Constraint::PrimaryKey);
            }
            "KEY" => {
                // Skip KEY after PRIMARY
            }
            "UNIQUE" => {
                constraints.push(Constraint::Unique);
            }
            _ => {
                // Check for DEFAULT value
                if part_upper.starts_with("DEFAULT") {
                    // Extract default value
                    let default_val = part_upper
                        .strip_prefix("DEFAULT")
                        .map(|s| s.trim().to_string())
                        .unwrap_or_default();
                    if !default_val.is_empty() {
                        constraints.push(Constraint::Default(default_val));
                    }
                }
            }
        }
    }

    Some((name, sql_type, nullable, constraints, inline_fk))
}

/// Split column definitions by commas, respecting nested parentheses
fn split_columns(block: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut paren_depth = 0;

    for c in block.chars() {
        match c {
            '(' => {
                paren_depth += 1;
                current.push(c);
            }
            ')' => {
                paren_depth -= 1;
                current.push(c);
            }
            ',' if paren_depth == 0 => {
                result.push(current.trim().to_string());
                current.clear();
            }
            _ => {
                current.push(c);
            }
        }
    }

    if !current.trim().is_empty() {
        result.push(current.trim().to_string());
    }

    result
}

/// Parse SQL type string to SqlType enum
fn parse_sql_type(type_str: &str) -> SqlType {
    let upper = type_str.to_uppercase();

    // Remove size specifiers like (255)
    let base_type = if let Some(paren_pos) = upper.find('(') {
        &upper[..paren_pos]
    } else {
        &upper
    };

    match base_type.trim() {
        "INT" | "INTEGER" => SqlType::Integer,
        "BIGINT" => SqlType::BigInt,
        "SERIAL" => SqlType::Serial,
        "BIGSERIAL" => SqlType::BigSerial,
        "VARCHAR" | "CHAR" | "CHARACTER" => {
            // Try to extract size
            if let Some(paren_pos) = type_str.find('(') {
                if let Some(size_str) = type_str[paren_pos + 1..].split(')').next() {
                    if let Ok(size) = size_str.parse() {
                        return SqlType::Varchar(Some(size));
                    }
                }
            }
            SqlType::Varchar(None)
        }
        "TEXT" | "STRING" => SqlType::Text,
        "BOOL" | "BOOLEAN" => SqlType::Boolean,
        "TIMESTAMP" => {
            if type_str.to_uppercase().contains("TZ") {
                SqlType::TimestampTz
            } else {
                SqlType::Timestamp
            }
        }
        "DATE" => SqlType::Date,
        "UUID" => SqlType::Uuid,
        "DECIMAL" | "NUMERIC" => {
            if let Some(paren_pos) = type_str.find('(') {
                let size_str = &type_str[paren_pos + 1..];
                if let Some(comma_pos) = size_str.find(',') {
                    let int_part: usize = size_str[..comma_pos].parse().unwrap_or(10);
                    let frac_part: usize = size_str[comma_pos + 1..].split(')').next().and_then(|s| s.parse().ok()).unwrap_or(2);
                    return SqlType::Decimal(Some((int_part, frac_part)));
                }
            }
            SqlType::Decimal(None)
        }
        "JSON" => SqlType::Json,
        "JSONB" => SqlType::Jsonb,
        "BYTEA" | "BLOB" | "BYTES" => SqlType::Bytes,
        "FLOAT" | "REAL" => SqlType::Float,
        "DOUBLE" | "DOUBLE PRECISION" => SqlType::Double,
        _ => SqlType::Custom(type_str.to_string()),
    }
}

/// Clean identifier by removing quotes and backticks
fn clean_identifier(s: &str) -> String {
    s.trim()
        .trim_matches('`')
        .trim_matches('"')
        .trim_matches('\'')
        .to_string()
}

/// Determine cardinality based on column counts
fn determine_cardinality(columns: &[String], ref_columns: &[String]) -> Cardinality {
    if columns.len() == 1 && ref_columns.len() == 1 {
        Cardinality::OneToMany
    } else {
        Cardinality::ManyToMany
    }
}

/// Parse Diesel schema.rs file
fn parse_diesel_schema(content: &str) -> anyhow::Result<(Vec<Entity>, Vec<ErRelationship>)> {
    let mut entities: Vec<Entity> = Vec::new();
    let relationships: Vec<ErRelationship> = Vec::new();

    // Pattern: table! { table_name (columns) => ... }
    let table_macro_re = Regex::new(r"(?i)table!\s*\{\s*([\w]+)\s*\(").unwrap();

    for cap in table_macro_re.captures_iter(content) {
        if let Some(table_name) = cap.get(1) {
            let name = table_name.as_str().to_string();

            // Find the block for this table
            let start_pos = cap.get(0).map(|m| m.end()).unwrap_or(0);
            let block = extract_diesel_table_block(content, start_pos);

            // Parse columns from diesel format: column_name: Type
            let column_re = Regex::new(r"(\w+)\s*:\s*[\w<>, ]+").unwrap();
            let mut columns: Vec<Column> = Vec::new();
            let mut primary_key: Option<Vec<String>> = None;

            for col_cap in column_re.captures_iter(&block) {
                if let Some(col_name) = col_cap.get(1) {
                    let col_name_str = col_name.as_str().to_string();

                    // Check if this is primary key (usually "id")
                    if col_name_str == "id" && primary_key.is_none() {
                        primary_key = Some(vec![col_name_str.clone()]);
                        columns.push(Column {
                            name: col_name_str,
                            sql_type: SqlType::Serial,
                            nullable: false,
                            constraints: vec![Constraint::PrimaryKey],
                        });
                    } else {
                        columns.push(Column {
                            name: col_name_str,
                            sql_type: SqlType::Text, // Diesel uses Rust types, approximate
                            nullable: true,
                            constraints: vec![],
                        });
                    }
                }
            }

            if !columns.is_empty() {
                entities.push(Entity {
                    name,
                    columns,
                    primary_key,
                });
            }
        }
    }

    Ok((entities, relationships))
}

/// Extract a Diesel table! block
fn extract_diesel_table_block(content: &str, start_pos: usize) -> String {
    let mut depth = 0;
    let mut end_pos = start_pos;

    for (i, c) in content[start_pos..].chars().enumerate() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end_pos = start_pos + i;
                    break;
                }
            }
            _ => {}
        }
    }

    content[start_pos..end_pos].to_string()
}

/// Parse Prisma schema.prisma file
fn parse_prisma_schema(content: &str) -> anyhow::Result<(Vec<Entity>, Vec<ErRelationship>)> {
    let mut entities: Vec<Entity> = Vec::new();
    let mut relationships: Vec<ErRelationship> = Vec::new();

    // Pattern: model ModelName { ... }
    let model_re = Regex::new(r"(?i)model\s+(\w+)\s*\{").unwrap();

    for cap in model_re.captures_iter(content) {
        if let Some(model_name) = cap.get(1) {
            let name = model_name.as_str().to_string();
            let start_pos = cap.get(0).map(|m| m.end()).unwrap_or(0);
            let block = extract_prisma_model_block(content, start_pos);

            let mut columns: Vec<Column> = Vec::new();
            let mut primary_key: Option<Vec<String>> = None;

            // Parse fields
            for line in block.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with("//") {
                    continue;
                }

                // Field pattern: fieldName Type [modifier]
                let field_re = Regex::new(r"^(\w+)\s+(\w+)").unwrap();
                if let Some(field_cap) = field_re.captures(line) {
                    if let (Some(field_name), Some(field_type)) = (field_cap.get(1), field_cap.get(2)) {
                        let field_name_str = field_name.as_str().to_string();
                        let field_type_str = field_type.as_str().to_uppercase();

                        let sql_type = match field_type_str.as_str() {
                            "INT" | "INTEGER" => SqlType::Integer,
                            "BIGINT" => SqlType::BigInt,
                            "STRING" => SqlType::Varchar(None),
                            "TEXT" => SqlType::Text,
                            "BOOLEAN" => SqlType::Boolean,
                            "DATETIME" | "TIMESTAMP" => SqlType::Timestamp,
                            "DATE" => SqlType::Date,
                            "UUID" => SqlType::Uuid,
                            "JSON" => SqlType::Json,
                            "FLOAT" => SqlType::Float,
                            "DECIMAL" => SqlType::Decimal(None),
                            "BYTES" => SqlType::Bytes,
                            _ => SqlType::Custom(field_type_str),
                        };

                        let nullable = line.contains("?");
                        let is_pk = line.contains("@id") || line.contains("@primary");

                        if is_pk && primary_key.is_none() {
                            primary_key = Some(vec![field_name_str.clone()]);
                        }

                        let mut constraints = Vec::new();
                        if is_pk {
                            constraints.push(Constraint::PrimaryKey);
                        }

                        columns.push(Column {
                            name: field_name_str,
                            sql_type,
                            nullable,
                            constraints,
                        });
                    }
                }

                // Check for relation fields
                let relation_re = Regex::new(r"(\w+)\s+(\w+)\s*@relation").unwrap();
                if let Some(rel_cap) = relation_re.captures(line) {
                    if let (Some(from_field), Some(to_model)) = (rel_cap.get(1), rel_cap.get(2)) {
                        relationships.push(ErRelationship {
                            source: name.clone(),
                            target: to_model.as_str().to_string(),
                            cardinality: Cardinality::OneToMany,
                            label: Some(format!("{} -> {}", from_field.as_str(), to_model.as_str())),
                        });
                    }
                }
            }

            entities.push(Entity {
                name,
                columns,
                primary_key,
            });
        }
    }

    Ok((entities, relationships))
}

/// Extract a Prisma model block
fn extract_prisma_model_block(content: &str, start_pos: usize) -> String {
    let mut depth = 0;
    let mut end_pos = start_pos;

    for (i, c) in content[start_pos..].chars().enumerate() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end_pos = start_pos + i;
                    break;
                }
            }
            _ => {}
        }
    }

    content[start_pos..end_pos].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_create_table_simple() {
        let sql = r#"
CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    email VARCHAR(255) UNIQUE
);
"#;
        let (entities, _) = parse_sql_content(sql).unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].name, "users");
        assert_eq!(entities[0].columns.len(), 3);
    }

    #[test]
    fn test_parse_create_table_pk() {
        let sql = r#"
CREATE TABLE orders (
    id INTEGER,
    user_id INTEGER,
    total DECIMAL(10,2),
    PRIMARY KEY (id, user_id)
);
"#;
        let (entities, _) = parse_sql_content(sql).unwrap();
        assert_eq!(entities.len(), 1);
        assert!(entities[0].primary_key.is_some());
        let pk = entities[0].primary_key.as_ref().unwrap();
        assert_eq!(pk.len(), 2);
    }

    #[test]
    fn test_parse_create_table_fk() {
        let sql = r#"
CREATE TABLE orders (
    id INTEGER PRIMARY KEY,
    user_id INTEGER REFERENCES users(id),
    total DECIMAL(10,2)
);
"#;
        let (entities, relationships) = parse_sql_content(sql).unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(relationships.len(), 1);
        assert_eq!(relationships[0].source, "orders");
        assert_eq!(relationships[0].target, "users");
    }

    #[test]
    fn test_detect_join_table_n_n() {
        // A join table typically has exactly 2 foreign keys and no other columns
        let sql = r#"
CREATE TABLE user_roles (
    user_id INTEGER REFERENCES users(id),
    role_id INTEGER REFERENCES roles(id)
);
"#;
        let (entities, relationships) = parse_sql_content(sql).unwrap();
        assert_eq!(entities.len(), 1);
        // Join table detection should result in ManyToMany
        assert_eq!(relationships[0].cardinality, Cardinality::ManyToMany);
    }

    #[test]
    fn test_infer_er_no_sql() {
        let temp_dir = tempfile::tempdir().unwrap();
        let model = infer_er_diagram(temp_dir.path()).unwrap();
        assert!(model.entities.is_empty());
        assert!(model.relationships.is_empty());
    }

    #[test]
    fn test_parse_sql_type() {
        assert!(matches!(parse_sql_type("VARCHAR(255)"), SqlType::Varchar(Some(255))));
        assert!(matches!(parse_sql_type("TEXT"), SqlType::Text));
        assert!(matches!(parse_sql_type("INTEGER"), SqlType::Integer));
        assert!(matches!(parse_sql_type("BOOLEAN"), SqlType::Boolean));
        assert!(matches!(parse_sql_type("UUID"), SqlType::Uuid));
    }

    #[test]
    fn test_clean_identifier() {
        assert_eq!(clean_identifier("`users`"), "users");
        assert_eq!(clean_identifier("\"users\""), "users");
        assert_eq!(clean_identifier("'users'"), "users");
        assert_eq!(clean_identifier("users"), "users");
    }
}
