#!/bin/bash
# Generate Graduated Workspace Fixtures for Scalability Testing
# Creates Rust projects of ~1KB, ~10KB, ~100KB, ~1MB

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUTPUT_DIR="$(dirname "$SCRIPT_DIR")/fixtures/scale/rust"

# Clean output directory
rm -rf "${OUTPUT_DIR}"
mkdir -p "${OUTPUT_DIR}"

# Target sizes (in bytes, approximate)
SCALE_1KB=1024
SCALE_10KB=10240
SCALE_100KB=102400
SCALE_1MB=1048576

# Calculate number of modules needed for target size
# Each module ~80 lines * ~50 chars = ~4KB
calculate_modules() {
    local target=$1
    local bytes_per_module=4000
    local num=$((target / bytes_per_module))
    [ $num -lt 1 ] && num=1
    [ $num -gt 2000 ] && num=2000
    echo $num
}

# Generate a single module
write_module() {
    local idx=$1
    local output=$2
    
    cat > "$output" << ENDOFMODULE
// Module ${idx} - Auto-generated for scalability testing
use std::collections::HashMap;

/// Data structure ${idx}.1
pub struct Item${idx} {
    pub id: u64,
    pub name: String,
    pub value: f64,
    pub tags: Vec<String>,
}

impl Item${idx} {
    /// Process item ${idx}
    pub fn process(&self) -> u64 {
        self.id.wrapping_mul(${idx})
    }

    /// Calculate value for item ${idx}
    pub fn calculate(&self, multiplier: f64) -> f64 {
        self.value * multiplier + (self.id as f64)
    }

    /// Update tags for item ${idx}
    pub fn update_tags(&mut self, new_tag: String) {
        if !self.tags.contains(&new_tag) {
            self.tags.push(new_tag);
        }
    }
}

/// Data structure ${idx}.2 - Config
pub struct Config${idx} {
    pub enabled: bool,
    pub threshold: f64,
    pub mappings: HashMap<String, u64>,
}

impl Config${idx} {
    pub fn new() -> Self {
        Config${idx} {
            enabled: true,
            threshold: 0.5,
            mappings: HashMap::new(),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.threshold >= 0.0 && self.threshold <= 1.0
    }
}

/// Data structure ${idx}.3 - Stats
pub struct Stats${idx} {
    pub count: u64,
    pub sum: f64,
    pub min: f64,
    pub max: f64,
}

impl Stats${idx} {
    pub fn new() -> Self {
        Stats${idx} {
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

/// Compute function ${idx}.1 - entry point
pub fn compute_${idx}(input: u64) -> u64 {
    let base = input.wrapping_mul(${idx});
    helper_a_${idx}(base)
}

/// Compute function ${idx}.2 - middle layer
fn helper_a_${idx}(val: u64) -> u64 {
    let processed = val.wrapping_add(${idx} as u64);
    helper_b_${idx}(processed)
}

/// Compute function ${idx}.3 - leaf
fn helper_b_${idx}(val: u64) -> u64 {
    val.wrapping_mul(3).wrapping_sub(${idx} as u64)
}

/// Utility function ${idx}.1
pub fn process_batch_${idx}(items: &[u64]) -> Vec<u64> {
    items.iter().map(|&x| compute_${idx}(x)).collect()
}

/// Utility function ${idx}.2
pub fn aggregate_${idx}(values: &[f64]) -> f64 {
    if values.is_empty() { return 0.0; }
    values.iter().sum::<f64>() / (values.len() as f64)
}

/// Trait for ${idx}
pub trait Processor${idx} {
    fn process(&self) -> String;
    fn validate(&self) -> bool;
}

impl Processor${idx} for Item${idx} {
    fn process(&self) -> String {
        format!("item_{}: {}", self.id, self.value)
    }

    fn validate(&self) -> bool {
        self.id > 0 && !self.name.is_empty()
    }
}

/// Enum for ${idx}
pub enum State${idx} {
    Idle,
    Processing,
    Complete,
    Error(String),
}

impl State${idx} {
    pub fn is_active(&self) -> bool {
        matches!(self, State${idx}::Processing)
    }
}
ENDOFMODULE
}

# Generate lib.rs with module declarations
write_lib_rs() {
    local num_modules=$1
    local output=$2
    
    {
        echo "//! Scale fixture - auto-generated Rust library"
        echo "//! Size: ${num_modules} modules"
        echo ""
        
        # Module declarations
        for i in $(seq 1 $num_modules); do
            echo "pub mod module_${i};"
        done
        
        echo ""
        echo "/// Main entry point - computes across all modules"
        echo "pub fn compute_all(input: u64) -> u64 {"
        echo "    let mut result = input;"
        for i in $(seq 1 $num_modules); do
            echo "    result = result.wrapping_add(module_${i}::compute_${i}(result));"
        done
        echo "    result"
        echo "}"
        
    } > "$output"
}

# Generate Cargo.toml
write_cargo_toml() {
    local name=$1
    local output=$2
    
    cat > "$output" << ENDOFCARGO
[package]
name = "${name}"
version = "0.1.0"
edition = "2021"

[lib]
name = "${name}"
path = "src/lib.rs"

[dependencies]
ENDOFCARGO
}

# Create a fixture
create_fixture() {
    local name=$1
    local target_size=$2
    local output_dir="${OUTPUT_DIR}/${name}"
    
    echo "Creating ${name} (target: ~${target_size} bytes)..."
    
    mkdir -p "${output_dir}/src"
    
    local num_modules=$(calculate_modules $target_size)
    echo "  Using ${num_modules} modules..."
    
    # Write Cargo.toml
    write_cargo_toml "$name" "${output_dir}/Cargo.toml"
    
    # Write lib.rs
    write_lib_rs $num_modules "${output_dir}/src/lib.rs"
    
    # Generate individual modules
    for i in $(seq 1 $num_modules); do
        write_module $i "${output_dir}/src/module_${i}.rs"
    done
    
    # Calculate actual size
    local total_size=$(find "${output_dir}/src" -name "*.rs" -exec cat {} \; 2>/dev/null | wc -c)
    local file_count=$(find "${output_dir}/src" -name "*.rs" | wc -l)
    echo "  Total source: ${total_size} bytes in ${file_count} files"
}

# Main execution
echo "=== Graduated Workspace Fixture Generator ==="
echo "Output directory: ${OUTPUT_DIR}"
echo ""

create_fixture "scale_1kb" $SCALE_1KB
create_fixture "scale_10kb" $SCALE_10KB
create_fixture "scale_100kb" $SCALE_100KB
create_fixture "scale_1mb" $SCALE_1MB

echo ""
echo "=== Generation Complete ==="

# List created fixtures
echo ""
echo "Created fixtures:"
for dir in "${OUTPUT_DIR}"/*; do
    if [ -d "$dir" ]; then
        name=$(basename "$dir")
        size=$(find "$dir/src" -name "*.rs" -exec cat {} \; 2>/dev/null | wc -c || echo "0")
        files=$(find "$dir/src" -name "*.rs" 2>/dev/null | wc -l || echo "0")
        echo "  ${name}: ${size} bytes in ${files} files"
    fi
done
