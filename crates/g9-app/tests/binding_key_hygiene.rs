#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! core-11 (2026-11-10 review): `approvals.decide` cloned the installation's binding key on every
//! approval, and the copy was dropped without being wiped. The handler now borrows the key held
//! by the services, `g9_policy::BindingKey` wipes its bytes when it is dropped, and the type is
//! not `Clone` (2026-11-17 rework): no code in any crate or host can copy the key it holds, which
//! a text scan of one crate's sources could not guarantee (a copy through another binding, or in
//! `apps/cli` / `apps/desktop`, passed it).

use g9_policy::BindingKey;

/// Compiles only while `T` is not `Clone`: with a `Clone` impl the call below is ambiguous
/// (error E0283), so the gate fails at build time.
trait AmbiguousIfClone<A> {
    fn checked() {}
}
impl<T: ?Sized> AmbiguousIfClone<()> for T {}
struct IsClone;
impl<T: Clone> AmbiguousIfClone<IsClone> for T {}

#[test]
fn the_binding_key_cannot_be_copied_and_is_wiped_on_drop() {
    <BindingKey as AmbiguousIfClone<_>>::checked();
    <g9_app::keyfile::ProvisionedKey as AmbiguousIfClone<_>>::checked();
    assert!(
        std::mem::needs_drop::<BindingKey>(),
        "BindingKey implements Drop (wipes its bytes)"
    );
}
