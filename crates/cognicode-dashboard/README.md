# CogniCode Dashboard

Web UI for CogniCode code quality analysis, built with Leptos 0.7.

## Overview

The CogniCode Dashboard provides a visual interface for:

- **Dashboard**: Overview of project quality with ratings, metrics, and recent issues
- **Issues**: Browse and filter code quality issues found during analysis
- **Metrics**: Detailed metrics including technical debt, coverage, and trends
- **Quality Gate**: View and manage quality gate conditions and status
- **Configuration**: Configure analysis settings and project preferences

## Tech Stack

- **Leptos 0.7**: Rust frontend framework with fine-grained reactivity
- **Leptos Router**: Client-side routing
- **cognicode-quality**: In-process code quality analysis (embedded, no external server)

## Setup

### Prerequisites

- Rust toolchain (1.70+)
- `trunk` - WebAssembly bundler
- `wasm-bindgen-cli` - For building WASM targets

### Installation

```bash
# Install trunk (WebAssembly bundler)
cargo install trunk

# Install wasm-bindgen-cli
cargo install wasm-bindgen-cli
```

## Development

```bash
# Start the development server
trunk serve

# Open in browser
open http://localhost:8080
```

The dashboard will auto-reload when you modify source files.

## Building

```bash
# Build for release (optimized WASM)
trunk build --release

# The output will be in the `dist/` directory
```

## Project Structure

```
src/
├── api/              # Server functions for backend communication
│   ├── analysis.rs   # Analysis requests and results
│   ├── issues.rs     # Issue listing and filtering
│   ├── quality_gate.rs # Quality gate evaluation
│   └── configuration.rs # Configuration management
├── components/       # Reusable UI components
│   ├── shell.rs      # Main layout with sidebar navigation
│   ├── rating_card.rs # Letter rating display (A-E)
│   ├── metric_card.rs # Metrics with trends
│   ├── gate_status_bar.rs # Quality gate status
│   ├── issue_table.rs # Issues table
│   └── ...
├── pages/            # Page components
│   ├── dashboard.rs  # Main dashboard
│   ├── issues.rs     # Issues browser
│   ├── metrics.rs    # Detailed metrics
│   ├── quality_gate.rs # Quality gate view
│   └── configuration.rs # Settings page
├── state.rs          # Types and reactive state
├── lib.rs            # Library root with exports
└── main.rs           # Application entry point
```

## Features

### Responsive Design

The dashboard adapts to different screen sizes:

- **Desktop (>768px)**: Full sidebar navigation always visible
- **Mobile (<768px)**: Hamburger menu with slide-out sidebar

### Error Handling

Components use an error boundary pattern to gracefully handle runtime errors, showing user-friendly fallback UI instead of crashing.

### Mock Data

Currently uses mock data for demonstration. Integration with `cognicode-quality` is planned.

## Browser Support

- Chrome/Edge 90+
- Firefox 90+
- Safari 15+

## License

MIT
