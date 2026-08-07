# Tasks: oss-coverage-expansion

## Phase 1: Fixture Setup (Clone/Pin Repos)

- [x] 1.1 Update `sandbox/scripts/clone_repos.sh` to add `spf13/cobra` pinned at latest stable tag (v1.8.x) → `sandbox/repos/go/cobra/`
- [x] 1.2 Update `clone_repos.sh` to add `charmbracelet/bubbletea` pinned at latest stable tag → `sandbox/repos/go/bubbletea/`
- [x] 1.3 Update `clone_repos.sh` to add `samber/lo` pinned at latest stable tag → `sandbox/repos/go/lo/`
- [x] 1.4 Update `clone_repos.sh` to add `spring-projects/spring-petclinic` pinned at main SHA → `sandbox/repos/java/spring-petclinic/`
- [x] 1.5 Update `clone_repos.sh` to add `expressjs/express` pinned at latest stable tag (v4.x/v5.x) → `sandbox/repos/javascript/express/`
- [x] 1.6 Update `clone_repos.sh` to add `colinhacks/zod` pinned at latest stable tag (v3.x) → `sandbox/repos/typescript/zod/`
- [x] 1.7 Run `clone_repos.sh` to provision all 6 repos

## Phase 2: Manifest Creation Per Repo

- [x] 2.1 Create `sandbox/manifests/go_repos.yaml` with entries for cobra, bubbletea, lo — each containing: language, repo_url, pinned_sha, description, tier
- [x] 2.2 Create `sandbox/manifests/java_repos.yaml` with entry for spring-petclinic
- [x] 2.3 Update `sandbox/manifests/js_repos.yaml` to add expressjs/express entry (do NOT overwrite existing chalk entry)
- [x] 2.4 Update `sandbox/manifests/ts_repos.yaml` to add colinhacks/zod entry (do NOT overwrite existing commander entry)

## Phase 3: Container Configuration (Go & Java)

- [x] 3.1 Verify Go ≥1.21 is available in container for cobra, bubbletea, lo validation stages
- [x] 3.2 Verify Java ≥17 and Maven are available in container for spring-petclinic validation stages
- [x] 3.3 Verify Node.js ≥18 is available for express and zod (npm ci / npm install)
- [x] 3.4 If missing, document required runtime versions in `sandbox/manifests/schema.json` or a `SETUP_REQUIREMENTS.md`

## Phase 4: Validation Stages

- [x] 4.1 Add validation stages to `go_repos.yaml` entries: `go build ./...` (60s), `go vet ./...` (60s), `go test ./...` (180s) for cobra, bubbletea, lo
- [x] 4.2 Add validation stages to `java_repos.yaml` for petclinic: `javac --version` (30s), `mvn compile -q` (300s), `mvn test -q` (300s)
- [x] 4.3 Add validation stages to `js_repos.yaml` for express: `npm ci --frozen-lockfile || npm install` (120s), `node --check index.js` (30s), `npm test` (120s)
- [x] 4.4 Add validation stages to `ts_repos.yaml` for zod: `npm ci --frozen-lockfile || npm install` (120s), `tsc --noEmit` (60s), `jest --passWithNoTests` (120s)
- [x] 4.5 Run each validation stage manually and confirm exit code 0 for all 6 repos

## Phase 5: Smoke Test

- [x] 5.1 Smoke test: verify `sandbox/repos/go/cobra/cobra.go` exists after clone
- [x] 5.2 Smoke test: verify `sandbox/repos/go/bubbletea/go.mod` exists after clone
- [x] 5.3 Smoke test: verify `sandbox/repos/go/lo/lo.go` exists after clone (≥10 .go files)
- [x] 5.4 Smoke test: verify `sandbox/repos/java/spring-petclinic/pom.xml` exists after clone
- [x] 5.5 Smoke test: verify `sandbox/repos/javascript/express/index.js` exists after clone
- [x] 5.6 Smoke test: verify `sandbox/repos/typescript/zod/src/types.ts` exists after clone
- [x] 5.7 Smoke test: verify CogniCode `read_file` tool succeeds on at least one file from each new repo
