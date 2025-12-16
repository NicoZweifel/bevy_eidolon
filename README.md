# bevy_eidolon

> *"Reality is illusion"*

This is a generic instanced material, mostly as a high-performance replacement for gizmos.
I am planning to use this as a base for other instanced materials by allowing shader overwrites, similar to the `Material` and `MaterialExtension` in bevy.

## Who is this for? 

This won't have complex lighting support or anything like that. It's mostly about the infrastructure for instanced rendering.

The standard implementations will only ever support simple colors and shapes.

## Notes

- Completely ignores bevy's `Transform` currently. Might change that soon, but it's not a priority.
- Uses a custom pipeline for the `VisibilityRange` because of conflicting indices with the standard pipeline. Might also be replaced by something else in the future.
- I am open to discussions about the API (cover some shapes, simple cases), for now it's just a proof of concept.
- This is a WIP. Eventually a nice API will exist and probably a way to overwrite/customize shaders.
