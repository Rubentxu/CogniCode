//! Análisis incremental fiable (B3.W4).
//!
//! BLAKE3 sobre bytes reemplaza la invalidación pura por mtime + size
//! en el path caliente del cache de aplicación. El primitivo
//! `ContentHash` y el almacen por-workspace `CacheStore` viven aquí;
//! los adapters de análisis y de sesión los consumen sin introducir
//! estado global (cada `WorkspaceSession` carga su propio `CacheStore`).
//!
//! Reliable incremental analysis (B3.W4).
//!
//! BLAKE3 over file bytes replaces mtime+size-only invalidation on the
//! hot path of the application cache. The `ContentHash` primitive and
//! per-workspace `CacheStore` live here; analysis + session adapters
//! consume them without introducing global state (each
//! `WorkspaceSession` carries its own `CacheStore`).

#[cfg(feature = "persistence")]
pub mod content_hash;
