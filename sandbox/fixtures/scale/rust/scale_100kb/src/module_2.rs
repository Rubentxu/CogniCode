// Module 2 - Auto-generated for scalability testing
use std::collections::HashMap;

/// Data structure 2.1
pub struct Item2 {
    pub id: u64,
    pub name: String,
    pub value: f64,
    pub tags: Vec<String>,
}

impl Item2 {
    /// Process item 2
    pub fn process(&self) -> u64 {
        self.id.wrapping_mul(2)
    }

    /// Calculate value for item 2
    pub fn calculate(&self, multiplier: f64) -> f64 {
        self.value * multiplier + (self.id as f64)
    }

    /// Update tags for item 2
    pub fn update_tags(&mut self, new_tag: String) {
        if !self.tags.contains(&new_tag) {
            self.tags.push(new_tag);
        }
    }
}

/// Data structure 2.2 - Config
pub struct Config2 {
    pub enabled: bool,
    pub threshold: f64,
    pub mappings: HashMap<String, u64>,
}

impl Config2 {
    pub fn new() -> Self {
        Config2 {
            enabled: true,
            threshold: 0.5,
            mappings: HashMap::new(),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.threshold >= 0.0 && self.threshold <= 1.0
    }
}

/// Data structure 2.3 - Stats
pub struct Stats2 {
    pub count: u64,
    pub sum: f64,
    pub min: f64,
    pub max: f64,
}

impl Stats2 {
    pub fn new() -> Self {
        Stats2 {
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

/// Compute function 2.1 - entry point
pub fn compute_2(input: u64) -> u64 {
    let base = input.wrapping_mul(2);
    helper_a_2(base)
}

/// Compute function 2.2 - middle layer
fn helper_a_2(val: u64) -> u64 {
    let processed = val.wrapping_add(2 as u64);
    helper_b_2(processed)
}

/// Compute function 2.3 - leaf
fn helper_b_2(val: u64) -> u64 {
    val.wrapping_mul(3).wrapping_sub(2 as u64)
}

/// Utility function 2.1
pub fn process_batch_2(items: &[u64]) -> Vec<u64> {
    items.iter().map(|&x| compute_2(x)).collect()
}

/// Utility function 2.2
pub fn aggregate_2(values: &[f64]) -> f64 {
    if values.is_empty() { return 0.0; }
    values.iter().sum::<f64>() / (values.len() as f64)
}

/// Trait for 2
pub trait Processor2 {
    fn process(&self) -> String;
    fn validate(&self) -> bool;
}

impl Processor2 for Item2 {
    fn process(&self) -> String {
        format!("item_{}: {}", self.id, self.value)
    }

    fn validate(&self) -> bool {
        self.id > 0 && !self.name.is_empty()
    }
}

/// Enum for 2
pub enum State2 {
    Idle,
    Processing,
    Complete,
    Error(String),
}

impl State2 {
    pub fn is_active(&self) -> bool {
        matches!(self, State2::Processing)
    }
}
