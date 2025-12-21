# bevy_eidolon ![crates.io](https://img.shields.io/crates/v/bevy_eidolon.svg)

> *"Reality is illusion"*

This is a generic instanced material, mostly for foliage/grass and as a high-performance replacement for gizmos when writing debugging tools.

I am planning to use this as a base for other instanced materials, similar to the `Material` and `MaterialExtension` in Bevy.

> [!CAUTION]
> This package is in early development.

## What is this for? 

Drawing a lot of instances (millions) that require GPU-driven rendering with no transparency/alpha masking and that need some variation in scale, color, etc.,
but can't be reasonably done with the default material pipeline. 

Examples include grass, assemblies for foliage/trees and tools to debug them, as well as related systems (scattering, height mapping, obstacles).

## Scope & Philosophy

The standard implementation only supports simple colors, shapes, and basic features.

> [!IMPORTANT]
> **I don't want this to become a monster material that supports everything.** 

However, there is an example on how to use the standard PBR lighting, and I don't mind adding specific examples if the API is a bit more mature.

I want to focus on composability and declarativity to make it as simple as possible to write new features as custom materials.

## Notes

- Uses a custom pipeline for the `VisibilityRange` because of conflicting indices with the standard pipeline. Might also be replaced by something else in the future.
- This is work in progress, I am open to discussions about the API (cover some shapes, simple cases), for now it's just a proof of concept.
