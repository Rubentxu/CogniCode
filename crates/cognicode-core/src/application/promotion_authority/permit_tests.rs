//! Tests for `application::promotion_authority::permit` (e73 WU2
//! placeholder).
//!
//! e73 WU1 only ships a placeholder type in `permit.rs`. The full
//! `PromotionPermit` / `PromotionApply` API lands in e73 WU2. This
//! test file exists so the module layout compiles in WU1.

#[test]
fn permit_placeholder_compiles_and_documents_wu2_reservation() {
    // The placeholder type is `pub` and reachable from outside the
    // module. Future WU2 will replace this with a real permit type;
    // until then, the type's existence is the reservation.
    //
    // We cannot meaningfully test the placeholder beyond asserting the
    // symbol resolves. If this test compiles, the reservation holds.
    fn _compiles() -> crate::application::promotion_authority::permit::PromotionPermitReserved {
        crate::application::promotion_authority::permit::PromotionPermitReserved
    }
    let _ = _compiles;
}
