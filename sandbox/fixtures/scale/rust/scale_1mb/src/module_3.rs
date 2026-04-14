// Module 3 - Auto-generated for scalability testing
use std::collections::HashMap;

/// Data structure 3.1
pub struct Item3 {
    pub id: u64,
    pub name: String,
    pub value: f64,
    pub tags: Vec<String>,
}

impl Item3 {
    /// Process item 3
    pub fn process(&self) -> u64 {
        self.id.wrapping_mul(3)
    }

    /// Calculate value for item 3
    pub fn calculate(&self, multiplier: f64) -> f64 {
        self.value * multiplier + (self.id as f64)
    }

    /// Update tags for item 3
    pub fn update_tags(&mut self, new_tag: String) {
        if !self.tags.contains(&new_tag) {
            self.tags.push(new_tag);
        }
    }
}

/// Data structure 3.2 - Config
pub struct Config3 {
    pub enabled: bool,
    pub threshold: f64,
    pub mappings: HashMap<String, u64>,
}

impl Config3 {
    pub fn new() -> Self {
        Config3 {
            enabled: true,
            threshold: 0.5,
            mappings: HashMap::new(),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.threshold >= 0.0 && self.threshold <= 1.0
    }
}

/// Data structure 3.3 - Stats
pub struct Stats3 {
    pub count: u64,
    pub sum: f64,
    pub min: f64,
    pub max: f64,
}

impl Stats3 {
    pub fn new() -> Self {
        Stats3 {
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

/// Compute function 3.1 - entry point
pub fn compute_3(input: u64) -> u64 {
    let base = input.wrapping_mul(3);
    helper_a_3(base)
}

/// Compute function 3.2 - middle layer
fn helper_a_3(val: u64) -> u64 {
    let processed = val.wrapping_add(3 as u64);
    helper_b_3(processed)
}

/// Compute function 3.3 - leaf
fn helper_b_3(val: u64) -> u64 {
    val.wrapping_mul(3).wrapping_sub(3 as u64)
}

/// Utility function 3.1
pub fn process_batch_3(items: &[u64]) -> Vec<u64> {
    items.iter().map(|&x| compute_3(x)).collect()
}

/// Utility function 3.2
pub fn aggregate_3(values: &[f64]) -> f64 {
    if values.is_empty() { return 0.0; }
    values.iter().sum::<f64>() / (values.len() as f64)
}

/// Trait for 3
pub trait Processor3 {
    fn process(&self) -> String;
    fn validate(&self) -> bool;
}

impl Processor3 for Item3 {
    fn process(&self) -> String {
        format!("item_{}: {}", self.id, self.value)
    }

    fn validate(&self) -> bool {
        self.id > 0 && !self.name.is_empty()
    }
}

/// Enum for 3
pub enum State3 {
    Idle,
    Processing,
    Complete,
    Error(String),
}

impl State3 {
    pub fn is_active(&self) -> bool {
        matches!(self, State3::Processing)
    }
}
