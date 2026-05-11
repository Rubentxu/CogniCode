//! Entity-Relationship diagram renderer for Mermaid and D2 formats

use crate::model::er_types::{
    Cardinality, Constraint, ErModel,
};
use crate::render::d2::D2Options;

/// Escape text for safe inclusion in diagram syntax
fn escape_er(text: &str) -> String {
    text.replace('"', "'")
        .replace('[', "(")
        .replace(']', ")")
        .replace('{', "(")
        .replace('}', ")")
        .replace('<', "(")
        .replace('>', ")")
        .replace('&', "and")
        .replace('\n', " ")
        .replace('\r', "")
}

/// Renders an ER model as a Mermaid erDiagram.
///
/// Produces Mermaid erDiagram syntax showing entities (tables) with their columns
/// and relationships with cardinality indicators.
///
/// # Examples
///
/// ```
/// use cognicode_diagram::model::er_types::ErModel;
/// use cognicode_diagram::render::er::render_er_mermaid;
///
/// let model = ErModel::default();
/// let mermaid = render_er_mermaid(&model);
/// assert!(mermaid.starts_with("erDiagram"));
/// ```
pub fn render_er_mermaid(model: &ErModel) -> String {
    let mut lines = Vec::new();

    lines.push("erDiagram".to_string());

    // Render each entity
    for entity in &model.entities {
        lines.push(format!("    {}", entity.name));

        // Render columns
        for column in &entity.columns {
            let col_str = format_column_er(&entity.name, column);
            lines.push(format!("        {}", col_str));
        }

        // Add a blank line after each entity for readability (skip after last)
        if let Some(last_entity) = model.entities.last() {
            if !std::ptr::eq(last_entity, entity) {
                lines.push(String::new());
            }
        }
    }

    // Render relationships
    if !model.relationships.is_empty() {
        lines.push(String::new());
        for rel in &model.relationships {
            let cardinality = cardinality_mermaid(&rel.cardinality);
            let label = rel.label.as_deref().unwrap_or("");
            // Mermaid relationship syntax: ENTITY ||--|| ENTITY : label
            lines.push(format!(
                "    {} {} {} : {}",
                rel.source,
                cardinality,
                rel.target,
                escape_er(label)
            ));
        }
    }

    lines.join("\n")
}

/// Format a column for Mermaid erDiagram
fn format_column_er(_entity_name: &str, column: &crate::model::er_types::Column) -> String {
    let mut parts = Vec::new();

    // Column name
    parts.push(column.name.clone());

    // Type
    let type_str = format_sql_type(&column.sql_type);
    parts.push(format!(" {}", type_str));

    // Constraints
    for constraint in &column.constraints {
        match constraint {
            Constraint::PrimaryKey => parts.push(" PK".to_string()),
            Constraint::ForeignKey(fk) => parts.push(format!(" FK->{}", fk.ref_table)),
            Constraint::Unique => parts.push(" UNIQUE".to_string()),
            Constraint::NotNull => parts.push(" NOT NULL".to_string()),
            Constraint::Default(val) => parts.push(format!(" DEFAULT {}", val)),
            Constraint::Check(expr) => parts.push(format!(" CHECK({})", expr)),
        }
    }

    if column.nullable {
        parts.push(" NULL".to_string());
    }

    parts.join("")
}

/// Format SQL type for display
fn format_sql_type(sql_type: &crate::model::er_types::SqlType) -> String {
    match sql_type {
        crate::model::er_types::SqlType::Integer => "int".to_string(),
        crate::model::er_types::SqlType::BigInt => "bigint".to_string(),
        crate::model::er_types::SqlType::Serial => "serial".to_string(),
        crate::model::er_types::SqlType::BigSerial => "bigserial".to_string(),
        crate::model::er_types::SqlType::Varchar(Some(size)) => format!("varchar({})", size),
        crate::model::er_types::SqlType::Varchar(None) => "varchar".to_string(),
        crate::model::er_types::SqlType::Text => "text".to_string(),
        crate::model::er_types::SqlType::Boolean => "boolean".to_string(),
        crate::model::er_types::SqlType::Timestamp => "timestamp".to_string(),
        crate::model::er_types::SqlType::TimestampTz => "timestamptz".to_string(),
        crate::model::er_types::SqlType::Date => "date".to_string(),
        crate::model::er_types::SqlType::Uuid => "uuid".to_string(),
        crate::model::er_types::SqlType::Decimal(Some((prec, scale))) => format!("decimal({}, {})", prec, scale),
        crate::model::er_types::SqlType::Decimal(None) => "decimal".to_string(),
        crate::model::er_types::SqlType::Json => "json".to_string(),
        crate::model::er_types::SqlType::Jsonb => "jsonb".to_string(),
        crate::model::er_types::SqlType::Bytes => "bytes".to_string(),
        crate::model::er_types::SqlType::Float => "float".to_string(),
        crate::model::er_types::SqlType::Double => "double".to_string(),
        crate::model::er_types::SqlType::Custom(name) => name.clone(),
    }
}

/// Convert Cardinality to Mermaid notation
fn cardinality_mermaid(cardinality: &Cardinality) -> &'static str {
    match cardinality {
        Cardinality::OneToOne => "||--||",
        Cardinality::OneToMany => "||--o{",
        Cardinality::ManyToMany => "}o--o{",
    }
}

/// Renders an ER model as a D2 diagram.
///
/// Produces D2 diagram source showing entities (tables) with their columns,
/// data types, and foreign key relationships.
///
/// # Examples
///
/// ```
/// use cognicode_diagram::model::er_types::ErModel;
/// use cognicode_diagram::render::d2::D2Options;
/// use cognicode_diagram::render::er::render_er_d2;
///
/// let model = ErModel::default();
/// let options = D2Options::default();
/// let d2 = render_er_d2(&model, &options);
/// assert!(d2.contains("direction"));
/// ```
pub fn render_er_d2(model: &ErModel, options: &D2Options) -> String {
    let mut lines = Vec::new();

    // D2 header
    let direction = match options.direction {
        crate::render::d2::D2Direction::Down => "down",
        crate::render::d2::D2Direction::Right => "right",
    };
    lines.push(format!("direction: {}", direction));

    if options.sketch {
        lines.push("sketch: true".to_string());
    }

    lines.push(format!("pad: {}", options.pad));

    // Render entities as D2 shapes
    for entity in &model.entities {
        let entity_id = entity.name.replace(' ', "_").to_lowercase();
        lines.push(format!("{}: {{", entity_id));
        lines.push(format!("    label: \"{}\"", escape_er(&entity.name)));
        lines.push("    shape: rectangle".to_string());

        // Add columns as children (nested)
        lines.push("    columns: {".to_string());
        for column in &entity.columns {
            let col_id = format!("{}_{}", entity_id, column.name.replace(' ', "_").to_lowercase());
            let type_str = format_sql_type(&column.sql_type);
            let nullable = if column.nullable { "?" } else { "" };
            let pk = if column.constraints.iter().any(|c| matches!(c, Constraint::PrimaryKey)) { " [PK]" } else { "" };
            lines.push(format!("        {}: \"{}{}{}\"", col_id, column.name, nullable, pk));
            lines.push(format!("        {}.type: \"{}\"", col_id, type_str));
        }
        lines.push("    }".to_string());

        lines.push("}".to_string());
    }

    // Render relationships as D2 edges
    for rel in &model.relationships {
        let source_id = rel.source.replace(' ', "_").to_lowercase();
        let target_id = rel.target.replace(' ', "_").to_lowercase();
        let cardinality = cardinality_d2(&rel.cardinality);
        let label = rel.label.as_deref().unwrap_or("");

        lines.push(format!(
            "{} {}-> {} {}",
            source_id,
            cardinality.0,
            target_id,
            cardinality.1
        ));

        if !label.is_empty() {
            lines.push(format!("{} -> {} label: \"{}\"", source_id, target_id, escape_er(label)));
        }
    }

    lines.join("\n")
}

/// Convert Cardinality to D2 edge notation (source arrow, target arrow)
fn cardinality_d2(cardinality: &Cardinality) -> (&'static str, &'static str) {
    match cardinality {
        Cardinality::OneToOne => ("--", "--"),
        Cardinality::OneToMany => ("--", "->"),
        Cardinality::ManyToMany => ("->", "->"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::er_types::{
        Column, Constraint, Entity, ErModel, ErRelationship, ForeignKey, SqlType,
    };

    fn create_test_model() -> ErModel {
        let users = Entity {
            name: "users".to_string(),
            columns: vec![
                Column {
                    name: "id".to_string(),
                    sql_type: SqlType::Serial,
                    nullable: false,
                    constraints: vec![Constraint::PrimaryKey],
                },
                Column {
                    name: "name".to_string(),
                    sql_type: SqlType::Varchar(Some(255)),
                    nullable: false,
                    constraints: vec![],
                },
                Column {
                    name: "email".to_string(),
                    sql_type: SqlType::Varchar(Some(255)),
                    nullable: true,
                    constraints: vec![Constraint::Unique],
                },
            ],
            primary_key: Some(vec!["id".to_string()]),
        };

        let orders = Entity {
            name: "orders".to_string(),
            columns: vec![
                Column {
                    name: "id".to_string(),
                    sql_type: SqlType::Serial,
                    nullable: false,
                    constraints: vec![Constraint::PrimaryKey],
                },
                Column {
                    name: "user_id".to_string(),
                    sql_type: SqlType::Integer,
                    nullable: false,
                    constraints: vec![Constraint::ForeignKey(ForeignKey {
                        columns: vec!["user_id".to_string()],
                        ref_table: "users".to_string(),
                        ref_columns: vec!["id".to_string()],
                    })],
                },
                Column {
                    name: "total".to_string(),
                    sql_type: SqlType::Decimal(Some((10, 2))),
                    nullable: false,
                    constraints: vec![],
                },
            ],
            primary_key: Some(vec!["id".to_string()]),
        };

        let relationships = vec![
            ErRelationship {
                source: "orders".to_string(),
                target: "users".to_string(),
                cardinality: Cardinality::ManyToMany, // Would be OneToMany in reality
                label: Some("user_id -> id".to_string()),
            },
        ];

        ErModel {
            entities: vec![users, orders],
            relationships,
        }
    }

    #[test]
    fn test_render_er_mermaid() {
        let model = create_test_model();
        let result = render_er_mermaid(&model);
        assert!(result.contains("erDiagram"));
        assert!(result.contains("users"));
        assert!(result.contains("orders"));
        assert!(result.contains("id serial"));
        assert!(result.contains("email varchar(255)"));
    }

    #[test]
    fn test_render_er_d2() {
        let model = create_test_model();
        let options = D2Options::default();
        let result = render_er_d2(&model, &options);
        assert!(result.contains("direction: down"));
        assert!(result.contains("users"));
        assert!(result.contains("orders"));
        assert!(result.contains("columns"));
    }

    #[test]
    fn test_render_er_mermaid_empty() {
        let model = ErModel::empty();
        let result = render_er_mermaid(&model);
        assert!(result.contains("erDiagram"));
    }

    #[test]
    fn test_format_sql_type() {
        assert_eq!(format_sql_type(&SqlType::Integer), "int");
        assert_eq!(format_sql_type(&SqlType::Varchar(Some(255))), "varchar(255)");
        assert_eq!(format_sql_type(&SqlType::Text), "text");
        assert_eq!(format_sql_type(&SqlType::Boolean), "boolean");
        assert_eq!(format_sql_type(&SqlType::Custom("JSONB".to_string())), "JSONB");
    }
}
