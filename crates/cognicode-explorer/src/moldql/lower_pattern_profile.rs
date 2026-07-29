//! Lowering from Pattern Profile AST (`PatternQuery`) to `GraphPlan`.
//!
//! Part of e28-3-moldql-pattern-profile-v1: PR1 Foundation.

use cognicode_core::domain::plan::{
    GraphPlan, NeighborKind, OrderClause as DomainOrderClause, OrderDirection as DomainOrderDirection,
    PathPredicate, PathProjection, PathQuantifier, PlanError, PlanHash, PlanLimits, PlanMetadata,
    PlanVersion,
};
use std::str::FromStr;
use cognicode_core::domain::value_objects::DependencyType;

use super::ast::{
    Aggregation, Binding, EdgeDirection, EdgePattern, OrderClause, OrderDirection, PatternPredicate,
    PatternProjection, PatternQuery, PatternValue, PredicateTarget,
};

/// Default maximum hops for `+` quantifier in Pattern Profile.
const DEFAULT_PATTERN_MAX_HOPS: u32 = 8;

impl super::MoldqlAstLowerer {
    /// Lower a `PatternQuery` to a `GraphPlan`.
    pub fn lower_pattern_profile(&self, q: &PatternQuery) -> Result<GraphPlan, PlanError> {
        // 1. Validate all quantifiers are bounded (defence in depth — parser rejects unbounded).
        for edge in &q.edges {
            if edge.quantifier.max_hops.is_none() {
                return Err(PlanError::UnsupportedConstruct(
                    cognicode_core::domain::plan::UnsupportedConstruct::new(
                        cognicode_core::domain::plan::ConstructId::UnboundedPath,
                        format!(
                            "Pattern Profile rejects unbounded paths; use *m..n with finite n (edge `{:?}`)",
                            edge.kind
                        ),
                    ),
                ));
            }
        }

        // 2. Identify anchor (src) and target (dst) bindings.
        let src: String = q
            .bindings
            .first()
            .and_then(|b| b.name.clone())
            .ok_or_else(|| {
                let uc = cognicode_core::domain::plan::UnsupportedConstruct::new(
                    cognicode_core::domain::plan::ConstructId::Other("missing anchor binding".into()),
                    String::from("Pattern must have at least one named node binding"),
                );
                PlanError::UnsupportedConstruct(uc)
            })?;
        let dst: String = q
            .bindings
            .last()
            .and_then(|b| b.name.clone())
            .ok_or_else(|| {
                let uc = cognicode_core::domain::plan::UnsupportedConstruct::new(
                    cognicode_core::domain::plan::ConstructId::Other("missing target binding".into()),
                    String::from("Pattern must have at least one named node binding"),
                );
                PlanError::UnsupportedConstruct(uc)
            })?;

        // 3. Build edge kind filter from edge patterns.
        let edge_kind_filter = if q.edges.iter().any(|e| !e.kind.eq_ignore_ascii_case("calls")) {
            Some(
                q.edges
                    .iter()
                    .filter_map(|e| e.kind.parse::<DependencyType>().ok())
                    .collect(),
            )
        } else {
            None
        };

        // 4. Build predicates from PatternPredicate list.
        let predicates = self.lower_pattern_predicates(&q.predicates);

        // 5. Lower projection.
        let projection = self.lower_pattern_projection(&q.projection, q.shortest);

        // 6. Compute max hops for limits.
        let max_hops = q
            .edges
            .iter()
            .filter_map(|e| e.quantifier.max_hops)
            .max()
            .unwrap_or(1);

        // 7. Pick the right GraphPlan variant based on projection.
        let metadata = self.plan_metadata();
        match &q.projection {
            PatternProjection::Row { .. } => {
                // COUNT / ORDER BY / LIMIT → Cluster variant.
                let (by, aggregations, ordering, limit) = self.lower_row_projection(&q.projection);
                Ok(GraphPlan::Cluster {
                    by,
                    aggregations,
                    ordering,
                    limit,
                    limits: PlanLimits::builder().max_hops(max_hops).build(),
                    metadata,
                })
            }
            _ => {
                // Path variant.
                let quantifier = q
                    .edges
                    .first()
                    .map(|e| {
                        PathQuantifier::new(e.quantifier.max_hops, e.quantifier.min_hops)
                            .unwrap_or_else(|| PathQuantifier::new(Some(1), 1).unwrap())
                    })
                    .unwrap_or_else(|| PathQuantifier::new(Some(1), 1).unwrap());

                Ok(GraphPlan::Path {
                    src,
                    dst,
                    quantifier,
                    edge_kind_filter,
                    predicates,
                    projection,
                    limits: PlanLimits::builder().max_hops(max_hops).build(),
                    metadata,
                })
            }
        }
    }

    fn lower_pattern_predicates(
        &self,
        preds: &[PatternPredicate],
    ) -> Vec<PathPredicate> {
        preds
            .iter()
            .filter_map(|p| {
                match p {
                    PatternPredicate::Property { target, field, op, value } => {
                        let label = match target {
                            PredicateTarget::Node(n) => format!("{}.{}", n, field),
                            PredicateTarget::Edge(e) => format!("{}.{}", e, field),
                            PredicateTarget::Anonymous => field.clone(),
                        };
                        let value = match value {
                            PatternValue::String(s) => {
                                cognicode_core::domain::plan::TypedValue::String(s.clone())
                            }
                            PatternValue::Number(n) => {
                                cognicode_core::domain::plan::TypedValue::Float(*n)
                            }
                        };
                        Some(PathPredicate { label, value })
                    }
                    PatternPredicate::Provenance { target, source } => {
                        let label = match target {
                            Some(n) => format!("{}.provenance", n),
                            None => "provenance".into(),
                        };
                        let value =
                            cognicode_core::domain::plan::TypedValue::String(source.clone());
                        Some(PathPredicate { label, value })
                    }
                    PatternPredicate::Confidence { target, op, value } => {
                        let label = match target {
                            PredicateTarget::Node(n) => format!("{}.confidence", n),
                            PredicateTarget::Edge(e) => format!("{}.confidence", e),
                            PredicateTarget::Anonymous => "confidence".into(),
                        };
                        let v = cognicode_core::domain::plan::TypedValue::Float(*value);
                        Some(PathPredicate { label, value: v })
                    }
                }
            })
            .collect()
    }

    fn lower_pattern_projection(
        &self,
        proj: &PatternProjection,
        shortest: bool,
    ) -> PathProjection {
        match proj {
            PatternProjection::Path { bindings } => PathProjection {
                nodes: bindings.clone(),
                edges: vec![],
                shortest,
            },
            PatternProjection::Node { binding } => PathProjection {
                nodes: vec![binding.clone()],
                edges: vec![],
                shortest,
            },
            PatternProjection::Edge { binding } => PathProjection {
                nodes: vec![],
                edges: vec![binding.clone()],
                shortest,
            },
            PatternProjection::Row { .. } => {
                // Row projection is handled separately in lower_row_projection.
                // Here we return a minimal projection.
                PathProjection {
                    nodes: vec![],
                    edges: vec![],
                    shortest,
                }
            }
        }
    }

    fn lower_row_projection(
        &self,
        proj: &PatternProjection,
    ) -> (
        Vec<String>,
        Vec<cognicode_core::domain::plan::TypedValue>,
        Option<DomainOrderClause>,
        Option<usize>,
    ) {
        match proj {
            PatternProjection::Row {
                fields: _,
                group_by,
                aggregations,
                ordering,
                limit,
            } => {
                let by = group_by.clone();
                let aggs = aggregations
                    .iter()
                    .map(|a| match a {
                        Aggregation::Count { binding: _, alias: _ } => {
                            cognicode_core::domain::plan::TypedValue::Int(0)
                        }
                    })
                    .collect();
                let ordering = ordering.as_ref().map(|o| DomainOrderClause {
                    by: o.by.clone(),
                    direction: match o.direction {
                        OrderDirection::Asc => DomainOrderDirection::Asc,
                        OrderDirection::Desc => DomainOrderDirection::Desc,
                    },
                });
                (by, aggs, ordering, limit.clone())
            }
            _ => (vec![], vec![], None, None),
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moldql::ast::{
        Binding, EdgeDirection, EdgePattern, MoldQLQuery, PathQuantifier, PatternProjection,
        PatternQuery, PatternPredicate, PredicateTarget,
    };
    use crate::moldql::lower_plan::MoldqlAstLowerer;
    use cognicode_core::domain::plan::lower::AstLowerer;

    /// `PatternQuery` with bounded quantifier lowers to `GraphPlan::Path`.
    #[test]
    fn lower_pattern_profile_bounded_path() {
        let lowerer = MoldqlAstLowerer::new();
        let ast = MoldQLQuery::Pattern(PatternQuery {
            shortest: false,
            bindings: vec![
                Binding { name: Some("r".into()), kind: "Route".into() },
                Binding { name: Some("f".into()), kind: "Function".into() },
            ],
            edges: vec![EdgePattern {
                name: Some("c".into()),
                kind: "Calls".into(),
                quantifier: PathQuantifier::new(Some(3), 1).unwrap(),
                direction: EdgeDirection::Outgoing,
            }],
            predicates: vec![],
            projection: PatternProjection::Path {
                bindings: vec!["r".into(), "c".into(), "f".into()],
            },
        });
        let result = lowerer.lower(&ast);
        assert!(result.is_ok(), "lowering should succeed: {:?}", result);
        let plan = result.unwrap();
        assert!(matches!(plan, GraphPlan::Path { .. }), "expected Path variant");
        if let GraphPlan::Path { src, dst, quantifier, projection, .. } = &plan {
            assert_eq!(src, "r");
            assert_eq!(dst, "f");
            assert_eq!(quantifier.max_hops, Some(3));
            assert_eq!(quantifier.min_hops, 1);
            assert!(!projection.shortest);
        }
    }

    /// `PatternQuery` with unbounded quantifier is rejected.
    #[test]
    fn lower_pattern_profile_unbounded_rejected() {
        let lowerer = MoldqlAstLowerer::new();
        let ast = MoldQLQuery::Pattern(PatternQuery {
            shortest: false,
            bindings: vec![
                Binding { name: Some("a".into()), kind: "Function".into() },
                Binding { name: Some("b".into()), kind: "Function".into() },
            ],
            edges: vec![EdgePattern {
                name: None,
                kind: "Calls".into(),
                quantifier: PathQuantifier { max_hops: None, min_hops: 0 },
                direction: EdgeDirection::Outgoing,
            }],
            predicates: vec![],
            projection: PatternProjection::Node { binding: "b".into() },
        });
        let result = lowerer.lower(&ast);
        assert!(result.is_err(), "unbounded should be rejected");
    }

    /// `SHORTEST` sets `projection.shortest = true`.
    #[test]
    fn lower_pattern_profile_shortest() {
        let lowerer = MoldqlAstLowerer::new();
        let ast = MoldQLQuery::Pattern(PatternQuery {
            shortest: true,
            bindings: vec![
                Binding { name: Some("a".into()), kind: "Route".into() },
                Binding { name: Some("b".into()), kind: "Function".into() },
            ],
            edges: vec![EdgePattern {
                name: Some("c".into()),
                kind: "Calls".into(),
                quantifier: PathQuantifier::new(Some(6), 1).unwrap(),
                direction: EdgeDirection::Outgoing,
            }],
            predicates: vec![],
            projection: PatternProjection::Path {
                bindings: vec!["a".into(), "c".into(), "b".into()],
            },
        });
        let result = lowerer.lower(&ast).unwrap();
        if let GraphPlan::Path { projection, .. } = &result {
            assert!(projection.shortest, "shortest should be true");
        }
    }

    /// `PatternQuery` with row projection lowers to `GraphPlan::Cluster`.
    #[test]
    fn lower_pattern_profile_row_cluster() {
        let lowerer = MoldqlAstLowerer::new();
        let ast = MoldQLQuery::Pattern(PatternQuery {
            shortest: false,
            bindings: vec![
                Binding { name: Some("f".into()), kind: "Function".into() },
                Binding { name: Some("g".into()), kind: "Function".into() },
            ],
            edges: vec![EdgePattern {
                name: Some("c".into()),
                kind: "Calls".into(),
                quantifier: PathQuantifier::new(Some(4), 1).unwrap(),
                direction: EdgeDirection::Outgoing,
            }],
            predicates: vec![],
            projection: PatternProjection::Row {
                fields: vec![],
                group_by: vec!["f.module".into()],
                aggregations: vec![super::super::ast::Aggregation::Count {
                    binding: Some("c".into()),
                    alias: "calls".into(),
                }],
                ordering: Some(OrderClause {
                    by: "calls".into(),
                    direction: OrderDirection::Desc,
                }),
                limit: Some(5),
            },
        });
        let result = lowerer.lower(&ast).unwrap();
        assert!(matches!(result, GraphPlan::Cluster { .. }), "expected Cluster variant");
        if let GraphPlan::Cluster { ordering, limit, .. } = &result {
            assert!(ordering.is_some(), "ordering should be set");
            assert_eq!(*limit, Some(5), "limit should be 5");
        }
    }
}
