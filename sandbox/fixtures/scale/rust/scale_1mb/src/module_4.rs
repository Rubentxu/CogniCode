// Module 4 - Auto-generated for scalability testing
use std::collections::HashMap;

/// Data structure 4.1
pub struct Item4 {
    pub id: u64,
    pub name: String,
    pub value: f64,
    pub tags: Vec<String>,
}

impl Item4 {
    /// Process item 4
    pub fn process(&self) -> u64 {
        self.id.wrapping_mul(4)
    }

    /// Calculate value for item 4
    pub fn calculate(&self, multiplier: f64) -> f64 {
        self.value * multiplier + (self.id as f64)
    }

    /// Update tags for item 4
    pub fn update_tags(&mut self, new_tag: String) {
        if !self.tags.contains(&new_tag) {
            self.tags.push(new_tag);
        }
    }
}

/// Data structure 4.2 - Config
pub struct Config4 {
    pub enabled: bool,
    pub threshold: f64,
    pub mappings: HashMap<String, u64>,
}

impl Config4 {
    pub fn new() -> Self {
        Config4 {
            enabled: true,
            threshold: 0.5,
            mappings: HashMap::new(),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.threshold >= 0.0 && self.threshold <= 1.0
    }
}

/// Data structure 4.3 - Stats
pub struct Stats4 {
    pub count: u64,
    pub sum: f64,
    pub min: f64,
    pub max: f64,
}

impl Stats4 {
    pub fn new() -> Self {
        Stats4 {
            count: 0,
            sum: 0.0,
            min: f64::MAX,
            max: f64::MIN,
        }
    }

    pub fn update(&mut self, value: f64) {
        self.count += 1;
        self.sum += value;
        self.min = self.min.min(value);
        self.max = self.max.max(value);
    }

    pub fn average(&self) -> f64 {
        if self.count == 0 { 0.0 } else { self.sum / (self.count as f64) }
    }
}

/// Compute function 4.1 - entry point
pub fn compute_4(input: u64) -> u64 {
    let base = input.wrapping_mul(4);
    helper_a_4(base)
}

/// Compute function 4.2 - middle layer
fn helper_a_4(val: u64) -> u64 {
    let processed = val.wrapping_add(4 as u64);
    helper_b_4(processed)
}

/// Compute function 4.3 - leaf
fn helper_b_4(val: u64) -> u64 {
    val.wrapping_mul(3).wrapping_sub(4 as u64)
}

/// Utility function 4.1
pub fn process_batch_4(items: &[u64]) -> Vec<u64> {
    items.iter().map(|&x| compute_4(x)).collect()
}

/// Utility function 4.2
pub fn aggregate_4(values: &[f64]) -> f64 {
    if values.is_empty() { return 0.0; }
    values.iter().sum::<f64>() / (values.len() as f64)
}

/// Trait for 4
pub trait Processor4 {
    fn process(&self) -> String;
    fn validate(&self) -> bool;
}

impl Processor4 for Item4 {
    fn process(&self) -> String {
        format!("item_{}: {}", self.id, self.value)
    }

    fn validate(&self) -> bool {
        self.id > 0 && !self.name.is_empty()
    }
}

/// Enum for 4
pub enum State4 {
    Idle,
    Processing,
    Complete,
    Error(String),
}

impl State4 {
    pub fn is_active(&self) -> bool {
        matches!(self, State4::Processing)
    }
}
