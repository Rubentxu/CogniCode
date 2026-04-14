# Fixture Specification: java-petclinic (spring-projects/spring-petclinic)

## Purpose

Replace or supplement the minimal `java-sample` fixture with a real-world
Java Spring Boot application. spring-petclinic is a well-known reference app
with layered Maven/Gradle build structure, JPA entities, REST controllers, and service
classes — exercising Java analysis at production scale. The implementation uses
Gradle (`./gradlew`) as the primary build tool.

## Fixture Metadata

| Field       | Value                                            |
|-------------|--------------------------------------------------|
| Repo        | spring-projects/spring-petclinic                 |
| Language    | Java                                             |
| Tier        | B (nightly, not smoke lane)                      |
| Pin target  | Latest stable tag or main SHA                    |
| Path        | `sandbox/repos/java/spring-petclinic/`           |
| Manifest    | `sandbox/manifests/java_repos.yaml`              |
| Size budget | ≤ 10 MB on disk (excluding `.m2` cache)          |

## Requirements

### Requirement: Repo Snapshot Presence

The fixture MUST contain a pinned snapshot of spring-petclinic at a tagged SHA.
At minimum these MUST be present: `pom.xml`, `src/main/java/`, and at least
one entity class (e.g. `Owner.java`).

#### Scenario: Core build structure is present after setup

- GIVEN the sandbox setup script has run for java-petclinic
- WHEN `sandbox/repos/java/spring-petclinic/` is inspected
- THEN `pom.xml` MUST exist (Maven build file)
- AND `./gradlew` MUST exist (Gradle wrapper)
- AND `src/main/java/org/springframework/samples/petclinic/` MUST exist
- AND at least 5 `.java` files MUST be present

#### Scenario: Pinned SHA is reproducible

- GIVEN the manifest entry specifies a SHA
- WHEN the setup script clones or restores the fixture
- THEN the resulting HEAD commit MUST match the pinned SHA exactly

---

### Requirement: Manifest Entry

A valid manifest entry MUST exist for java-petclinic in `java_repos.yaml`,
containing: `language`, `repo_url`, `pinned_sha`, `description`, and `tier`.

#### Scenario: Manifest entry validates against schema

- GIVEN `sandbox/manifests/java_repos.yaml` exists and contains the java-petclinic entry
- WHEN the manifest schema validator runs
- THEN validation MUST pass with zero errors

---

### Requirement: Validation Stages

The java-petclinic fixture MUST define these validation stages:

| Stage   | Command                          | Timeout |
|---------|----------------------------------|---------|
| syntax  | `javac --version`                | 30s     |
| build   | `./gradlew compileJava -q`       | 300s    |
| test    | `./gradlew test -q`              | 300s    |

#### Scenario: Gradle build succeeds

- GIVEN the fixture is present and Java ≥17 and Gradle are available
- WHEN `./gradlew compileJava -q` is run in `sandbox/repos/java/spring-petclinic/`
- THEN exit code MUST be 0

#### Scenario: Gradle test passes

- GIVEN the fixture is present and Gradle is available
- WHEN `./gradlew test -q` is run
- THEN exit code MUST be 0 and at least 1 test MUST execute

---

### Requirement: CogniCode Java Analysis Compatibility

CogniCode's pipeline MUST run without errors on petclinic's Java source.
Scenarios MUST cover: read_file, search_content, and extract_symbols on
entity, controller, and service classes.

#### Scenario: read_file on a Java entity class

- GIVEN java-petclinic fixture is present
- WHEN `read_file(path="src/main/java/.../Owner.java", mode="raw")` is called
- THEN result MUST contain Java source text with `pass` outcome

#### Scenario: search_content finds Spring annotation

- GIVEN java-petclinic fixture is present
- WHEN `search_content(query="@Entity")` is called
- THEN at least one match MUST be returned

#### Scenario: extract_symbols returns class-level declarations

- GIVEN java-petclinic fixture is present
- WHEN `extract_symbols` is called on `Owner.java`
- THEN result MUST be `pass` or `capability_missing` — NEVER `error`
- AND if `pass`, result MUST include at least the class name `Owner`

#### Scenario: Annotation-heavy files do not cause parser panic

- GIVEN java-petclinic has files with multiple stacked annotations
- WHEN any analysis tool processes a controller or entity file
- THEN the pipeline MUST not crash or return `error`

---

## Correctness Metrics

| Metric                            | Target    |
|-----------------------------------|-----------|
| Manifest schema validation        | 100% pass |
| `./gradlew compileJava` success   | 100%      |
| Analysis pipeline errors          | 0         |
| Class symbol extraction (if supp.)| ≥ 1 per file  |
| Fixture size on disk              | ≤ 10 MB   |
