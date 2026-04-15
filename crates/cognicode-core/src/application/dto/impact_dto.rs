//! Impact DTO - Data Transfer Objects for impact analysis

use crate::domain::aggregates::call_graph::SymbolId;
use serde::{Deserialize, Serialize};

/// DTO for impact analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactDto {
    /// Symbol that was analyzed
    pub symbol_id: String,
    /// Number of symbols that would be impacted
    pub impacted_count: usize,
    /// List of impacted symbol IDs
    pub impacted_symbols: Vec<String>,
    /// Impact score (0-10)
    pub impact_score: u8,
    /// Risk level description
    pub risk_level: String,
}

impl ImpactDto {
    /// Creates a new ImpactDto
    pub fn new(symbol_id: impl Into<String>, impacted: usize, score: u8) -> Self {
        let risk_level = match score {
            0..=3 => "low",
            4..=6 => "medium",
            7..=9 => "high",
            _ => "critical",
        };

        Self {
            symbol_id: symbol_id.into(),
            impacted_count: impacted,
            impacted_symbols: Vec::new(),
            impact_score: score,
            risk_level: risk_level.to_string(),
        }
    }

    /// Sets the impacted symbols
    pub fn with_impacted_symbols(mut self, symbols: Vec<SymbolId>) -> Self {
        self.impacted_symbols = symbols.iter().map(|s| s.as_str().to_string()).collect();
        self
    }
}

/// DTO for cycle detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CycleDto {
    /// Whether cycles were detected
    pub has_cycles: bool,
    /// Number of cycles detected
    pub cycle_count: usize,
    /// List of cycles (each cycle is a list of symbol IDs)
    pub cycles: Vec<Vec<String>>,
}

impl CycleDto {
    /// Creates a new CycleDto
    pub fn new(has_cycles: bool, cycle_count: usize) -> Self {
        Self {
            has_cycles,
            cycle_count,
            cycles: Vec::new(),
        }
    }

    /// Sets the cycles
    pub fn with_cycles(mut self, cycles: Vec<Vec<SymbolId>>) -> Self {
        self.cycles = cycles
            .into_iter()
            .map(|c| c.into_iter().map(|s| s.as_str().to_string()).collect())
            .collect();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::aggregates::call_graph::SymbolId;

    #[test]
    fn test_impact_dto_new_low_risk() {
        let dto = ImpactDto::new("symbol_1", 2, 2);
        assert_eq!(dto.symbol_id, "symbol_1");
        assert_eq!(dto.impacted_count, 2);
        assert_eq!(dto.impact_score, 2);
        assert_eq!(dto.risk_level, "low");
        assert!(dto.impacted_symbols.is_empty());
    }

    #[test]
    fn test_impact_dto_new_medium_risk() {
        let dto = ImpactDto::new("func_a", 5, 5);
        assert_eq!(dto.risk_level, "medium");
    }

    #[test]
    fn test_impact_dto_new_high_risk() {
        let dto = ImpactDto::new("core", 20, 8);
        assert_eq!(dto.risk_level, "high");
    }

    #[test]
    fn test_impact_dto_critical_risk() {
        let dto = ImpactDto::new("critical", 100, 10);
        assert_eq!(dto.risk_level, "critical");
    }

    #[test]
    fn test_impact_dto_with_impacted_symbols() {
        let symbols = vec![
            SymbolId::new("sym_1"),
            SymbolId::new("sym_2"),
            SymbolId::new("sym_3"),
        ];
        let dto = ImpactDto::new("target", 3, 5).with_impacted_symbols(symbols);
        assert_eq!(dto.impacted_symbols.len(), 3);
        assert!(dto.impacted_symbols.contains(&"sym_1".to_string()));
    }

    #[test]
    fn test_cycle_dto_new_no_cycles() {
        let dto = CycleDto::new(false, 0);
        assert!(!dto.has_cycles);
        assert_eq!(dto.cycle_count, 0);
        assert!(dto.cycles.is_empty());
    }

    #[test]
    fn test_cycle_dto_new_with_cycles() {
        let dto = CycleDto::new(true, 2);
        assert!(dto.has_cycles);
        assert_eq!(dto.cycle_count, 2);
    }

    #[test]
    fn test_cycle_dto_with_cycles() {
        let cycles = vec![
            vec![SymbolId::new("a"), SymbolId::new("b"), SymbolId::new("a")],
            vec![
                SymbolId::new("x"),
                SymbolId::new("y"),
                SymbolId::new("z"),
                SymbolId::new("x"),
            ],
        ];
        let dto = CycleDto::new(true, 2).with_cycles(cycles);
        assert_eq!(dto.cycles.len(), 2);
        assert_eq!(dto.cycles[0], vec!["a", "b", "a"]);
        assert_eq!(dto.cycles[1], vec!["x", "y", "z", "x"]);
    }
}
