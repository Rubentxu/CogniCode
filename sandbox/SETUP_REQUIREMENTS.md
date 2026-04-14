# CogniCode Sandbox - Runtime Requirements

## Overview
This document lists the runtime requirements for validating repos in the CogniCode sandbox.

## Runtime Versions

| Language | Tool | Minimum Version | Required For |
|----------|------|-----------------|--------------|
| Rust | cargo | 1.70+ | serde, ripgrep, anyhow |
| Python | python | 3.9+ | click, urllib3, requests |
| Go | go | 1.21+ | cobra, bubbletea, lo |
| Java | java | 17+ | spring-petclinic |
| Java | maven | 3.8+ | spring-petclinic |
| JavaScript | node | 18+ | chalk, express |
| TypeScript | node | 18+ | commander, zod |
| npm | npm | 9+ | JS/TS repos |

## Phase 3 Coverage Expansion - New Repos

### Go Repos (sandbox/repos/go/)
- **cobra** (v1.8.1): CLI framework - `go build ./...`, `go vet ./...`, `go test ./...`
- **bubbletea** (v1.3.9): TUI framework - `go build ./...`, `go vet ./...`, `go test ./...`
- **lo** (v1.43.0): Utility library - `go build ./...`, `go vet ./...`, `go test ./...`

### Java Repos (sandbox/repos/java/)
- **spring-petclinic** (main): Spring Boot demo app - `mvn compile`, `mvn test`
- **java-sample** (fixture): Minimal Maven project for capability probes

### JavaScript Repos (sandbox/repos/javascript/)
- **chalk** (v5.1.0): CLI colors - `npm ci`, `node --check`, `npm test`
- **express** (4.21.0): Web framework - `npm ci`, `node --check`, `npm test`

### TypeScript Repos (sandbox/repos/typescript/)
- **commander** (v11.0.0): CLI framework - `npm ci`, `tsc --noEmit`, `npm test`
- **zod** (v3.24.1): Schema validation - `npm ci`, `tsc --noEmit`, `npm test`

## Current Container Status

- Go 1.22.2 ✅ (≥1.21 required) - AVAILABLE
- Java 17 ✅ (≥17 required) - AVAILABLE
- Maven ❌ (≥3.8 required) - **MISSING** - needs installation
- Node.js v25.2.1 ✅ (≥18 required) - AVAILABLE
- npm 11.6.2 ✅ (≥9 required) - AVAILABLE

## Installation Notes

### Maven Installation (if missing)
```bash
# Ubuntu/Debian
apt-get install maven

# macOS
brew install maven

# Manual install
 wget https://dlcdn.apache.org/maven/maven-3/3.9.6/binaries/apache-maven-3.9.6-bin.tar.gz
 tar -xf apache-maven-3.9.6-bin.tar.gz -C /opt/
 export PATH=/opt/apache-maven-3.9.6/bin:$PATH
```

## Validation Commands by Language

### Go
```bash
cd sandbox/repos/go/{repo}
go build ./...
go vet ./...
go test ./...
```

### Java
```bash
cd sandbox/repos/java/spring-petclinic
mvn compile
mvn test
```

### JavaScript
```bash
cd sandbox/repos/javascript/{repo}
npm ci --frozen-lockfile || npm install
node --check index.js
npm test
```

### TypeScript
```bash
cd sandbox/repos/typescript/{repo}
npm ci --frozen-lockfile || npm install
npx tsc --noEmit
npm test
```
