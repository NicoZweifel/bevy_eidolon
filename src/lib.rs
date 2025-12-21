//! # bevy_eidolon
//!
//! > *"Reality is illusion"*
//!
//! This is a generic instanced material, mostly for foliage/grass and as a high-performance
//! replacement for gizmos when writing debugging tools.
//!
//! I am planning to use this as a base for other instanced materials by allowing shader
//! overwrites, similar to the `Material` and `MaterialExtension` in Bevy.
//!
//! ## What is this for?
//!
//! Drawing a lot of instances (millions) that require GPU-driven rendering with no
//! transparency/alpha masking and that need some variation in scale, color, etc.,
//! but can't be reasonably done with the default material pipeline.
//!
//! Examples include:
//! * Grass
//! * Assemblies for foliage/trees and tools to debug them
//! * Related systems (scattering, height mapping, obstacles)
//!
//! ## Scope & Philosophy
//!
//! **Important: I don't want this to become a monster material that supports everything.**
//!
//! The standard implementation will only ever support simple colors, shapes, and basic features.
//!
//! However, there is an example on how to use the standard PBR lighting, and I don't mind
//! adding specific examples if the API is a bit more mature. I want to focus on composability
//! and declarativity to make it as simple as possible to write new features as custom materials.
//!
//! ## Notes
//!
//! - Uses a custom pipeline for the `VisibilityRange` because of conflicting indices with
//!   the standard pipeline. Might also be replaced by something else in the future.
//! - This is work in progress, I am open to discussions about the API (cover some shapes,
//!   simple cases), for now it's just a proof of concept.

pub mod material;
pub mod resources;

pub mod render;

pub mod components;

pub mod cull;

pub mod prelude {
    pub use crate::{
        components::*, cull::prelude::*, material::*, render::plugin::*, render::prelude::*,
        resources::*,
    };
}
