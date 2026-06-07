# Versioned Object Identity For Persisted Artifacts

CogniCode Explorer Phase 1A may use MVP object IDs like `symbol:{file}:{name}:{line}` for direct UI/API calls, but persisted exploration paths and JSON replay decision artifacts must store versioned `ObjectIdentity` records. `ObjectIdentity` contains `id`, `object_type`, `version`, `natural_key`, `fingerprints`, `first_seen`, `last_seen`, and `supersedes`. This prevents saved paths and artifacts from breaking when files move, line numbers change, symbols are renamed, or stronger indexing identities are introduced later.
