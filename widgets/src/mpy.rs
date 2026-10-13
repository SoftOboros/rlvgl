//! Canonical MPY descriptor catalog assembled from actor-local declarations.

use rlvgl_core::actor::TypeDescriptor;

/// Built-in MPY v1 proof-actor catalog in stable TypeId order.
pub static CATALOG: [TypeDescriptor; 5] = [
    crate::container::MPY_DESCRIPTOR,
    crate::label::MPY_DESCRIPTOR,
    crate::button::MPY_DESCRIPTOR,
    crate::slider::MPY_DESCRIPTOR,
    crate::list::MPY_DESCRIPTOR,
];
