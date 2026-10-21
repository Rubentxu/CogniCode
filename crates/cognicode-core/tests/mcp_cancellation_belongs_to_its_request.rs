//! A cancellation belongs to the request that asked for it.
//!
//! The registry is where that belonging is kept: the id of the request and
//! a flag of its own. These tests pin the three properties that make it
//! true —
//!
//!   * the cancellation is filed under the id of the request that asked
//!     for it, and reaches nobody else;
//!   * the entry leaves the registry when the request ends, so a late
//!     notification finds nobody to mark;
//!   * a request that nobody cancelled starts uncancelled.
//!
//! The same properties are observed from the outside, over the real
//! transport, in the MCP crate. These ones are the inside of the
//! mechanism: they say which structure carries the guarantee, so a later
//! change that keeps the behaviour but moves the structure would have to
//! come back and say so.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use cognicode_core::interface::mcp::cancellation::CancellationRegistry;
use cognicode_core::interface::mcp::handlers::{
    handle_build_graph, BuildGraphInput, HandlerContext,
};
use rmcp::model::RequestId;

fn request_id(n: i64) -> RequestId {
    RequestId::Number(n)
}

fn flag() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}

/// The cancellation is filed under the id of its own request.
#[test]
fn la_cancelacion_se_registra_con_el_identificador_de_su_peticion() {
    let registry = CancellationRegistry::new();

    let first = flag();
    let second = flag();
    let _first_registration = registry.register(request_id(1), first.clone());
    let _second_registration = registry.register(request_id(2), second.clone());

    assert!(
        registry.is_registered(&request_id(1)),
        "the request has to be in the registry while it is in flight"
    );
    assert_eq!(registry.registered_count(), 2);

    assert!(
        registry.cancel(&request_id(1)),
        "cancelling a live request has to find it"
    );

    assert!(
        first.load(Ordering::SeqCst),
        "the cancelled request has to observe its own cancellation"
    );
    assert!(
        !second.load(Ordering::SeqCst),
        "the cancellation of one request must not reach another request"
    );
    assert!(
        registry.is_registered(&request_id(2)),
        "cancelling a request does not end it"
    );
}

/// The entry leaves the registry when the request ends.
#[test]
fn al_terminar_la_peticion_su_registro_desaparece() {
    let registry = CancellationRegistry::new();

    let flag = flag();
    {
        let _registration = registry.register(request_id(1), flag.clone());
        assert!(registry.is_registered(&request_id(1)));
    }

    assert!(
        !registry.is_registered(&request_id(1)),
        "a request that ended cannot still be in the registry"
    );
    assert_eq!(registry.registered_count(), 0);

    assert!(
        !registry.cancel(&request_id(1)),
        "a late notification finds nobody to mark"
    );
}

/// A request that nobody cancelled starts uncancelled.
#[test]
fn cancelar_una_operacion_deja_sin_cancelar_la_siguiente() {
    let registry = CancellationRegistry::new();

    let first = flag();
    {
        let _first_registration = registry.register(request_id(1), first.clone());
        registry.cancel(&request_id(1));
    }

    let second = flag();
    let _second_registration = registry.register(request_id(2), second.clone());

    assert!(
        !second.load(Ordering::SeqCst),
        "the request that came after a cancelled one has to start uncancelled"
    );
    assert!(
        registry.is_registered(&request_id(2)),
        "the new request is live, and only its own cancellation can mark it"
    );
    assert!(
        !registry.cancel(&request_id(1)),
        "the notification for the request that already ended finds nobody"
    );
    assert!(
        !second.load(Ordering::SeqCst),
        "a late notification for the old request must not mark the new one"
    );
}

/// Each request carries a flag of its own, and the session keeps none.
#[tokio::test]
async fn cada_peticion_tiene_su_propia_bandera_de_cancelacion() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let ctx = HandlerContext::builder()
        .with_working_dir(workspace.path().to_path_buf())
        .build();

    let (first, first_flag) = ctx.scoped_for_request();
    let (second, second_flag) = ctx.scoped_for_request();

    first_flag.store(true, Ordering::SeqCst);

    assert!(first.is_cancelled(), "the first request is cancelled");
    assert!(
        !second.is_cancelled(),
        "the second request was not cancelled"
    );
    assert!(
        !ctx.is_cancelled(),
        "a request cannot cancel the session it belongs to"
    );

    // Two flags, not one flag wearing two names.
    assert!(
        !Arc::ptr_eq(&first_flag, &second_flag),
        "each request has to carry its own flag"
    );
    second_flag.store(true, Ordering::SeqCst);
    assert!(second.is_cancelled(), "the second request is cancelled now");
}

/// The flag the context carries is the one the handler reads.
#[tokio::test]
async fn el_manejador_observa_la_cancelacion_de_su_peticion() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let ctx = HandlerContext::builder()
        .with_working_dir(workspace.path().to_path_buf())
        .build();

    let (cancelled, flag) = ctx.scoped_for_request();
    flag.store(true, Ordering::SeqCst);

    let error = handle_build_graph(&cancelled, BuildGraphInput { directory: None })
        .await
        .expect_err("a cancelled request must not run the handler");
    assert!(
        error.to_string().contains("Cancel"),
        "the handler has to say it was cancelled, said: {error}"
    );

    let (alive, _alive_flag) = ctx.scoped_for_request();
    let result = handle_build_graph(&alive, BuildGraphInput { directory: None }).await;
    assert!(
        !result.is_err_and(|e| e.to_string().contains("Cancel")),
        "a request that was not cancelled must not be reported as cancelled"
    );
}
