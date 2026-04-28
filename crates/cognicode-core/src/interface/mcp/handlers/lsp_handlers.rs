use super::*;

pub async fn handle_go_to_definition(
    ctx: &HandlerContext,
    input: GoToDefinitionInput,
) -> HandlerResult<GoToDefinitionOutput> {
    use crate::domain::traits::code_intelligence::CodeIntelligenceProvider;
    use crate::infrastructure::lsp::providers::CompositeProvider;

    let provider = CompositeProvider::new(&ctx.working_dir);
    let location = crate::domain::value_objects::Location::new(
        input.file_path.clone(),
        input.line,
        input.column,
    );

    match provider.get_definition(&location).await {
        Ok(Some(def_loc)) => {
            let source = std::fs::read_to_string(def_loc.file()).ok();
            let context = source.as_ref().map(|s| {
                let lines: Vec<&str> = s.lines().collect();
                let line_idx = (def_loc.line() as usize).saturating_sub(1);
                if line_idx < lines.len() {
                    lines[line_idx].to_string()
                } else {
                    String::new()
                }
            });
            Ok(GoToDefinitionOutput {
                found: true,
                file: Some(def_loc.file().to_string()),
                line: Some(def_loc.line()),
                column: Some(def_loc.column()),
                context,
                message: None,
            })
        }
        Ok(None) => Ok(GoToDefinitionOutput {
            found: false,
            file: None,
            line: None,
            column: None,
            context: None,
            message: Some("No definition found at this position".to_string()),
        }),
        Err(e) => Ok(GoToDefinitionOutput {
            found: false,
            file: None,
            line: None,
            column: None,
            context: None,
            message: Some(e.to_string()),
        }),
    }
}

/// Handler for hover tool
pub async fn handle_hover(
    ctx: &HandlerContext,
    input: HoverInput,
) -> HandlerResult<HoverOutput> {
    use crate::domain::traits::code_intelligence::CodeIntelligenceProvider;
    use crate::infrastructure::lsp::providers::CompositeProvider;

    let provider = CompositeProvider::new(&ctx.working_dir);
    let location = crate::domain::value_objects::Location::new(
        input.file_path.clone(),
        input.line,
        input.column,
    );

    match provider.hover(&location).await {
        Ok(Some(info)) => Ok(HoverOutput {
            found: true,
            content: Some(info.content),
            documentation: info.documentation,
            kind: Some(format!("{:?}", info.kind)),
        }),
        Ok(None) => Ok(HoverOutput {
            found: false,
            content: None,
            documentation: None,
            kind: None,
        }),
        Err(_) => Ok(HoverOutput {
            found: false,
            content: None,
            documentation: None,
            kind: None,
        }),
    }
}

/// Handler for find_references tool
pub async fn handle_find_references(
    ctx: &HandlerContext,
    input: FindReferencesInput,
) -> HandlerResult<FindReferencesOutput> {
    use crate::domain::traits::code_intelligence::CodeIntelligenceProvider;
    use crate::infrastructure::lsp::providers::CompositeProvider;

    let provider = CompositeProvider::new(&ctx.working_dir);
    let location = crate::domain::value_objects::Location::new(
        input.file_path.clone(),
        input.line,
        input.column,
    );

    match provider.find_references(&location, input.include_declaration).await {
        Ok(refs) => {
            let entries: Vec<ReferenceEntry> = refs.iter().map(|r| ReferenceEntry {
                file: r.location.file().to_string(),
                line: r.location.line(),
                column: r.location.column(),
                kind: format!("{:?}", r.reference_kind),
                context: r.container.clone().unwrap_or_default(),
            }).collect();
            let total = entries.len();
            Ok(FindReferencesOutput {
                symbol: input.file_path,
                references: entries,
                total,
            })
        }
        Err(_) => Ok(FindReferencesOutput {
            symbol: input.file_path,
            references: vec![],
            total: 0,
        }),
    }
}

// ============================================================================
