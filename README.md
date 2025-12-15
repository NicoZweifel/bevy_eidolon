# bevy_eidolon

This is a generic instanced material, mostly as a high-performance replacement for gizmos,
but I am planning to use this as a base for other instanced materials by allowing shader overwrites, similar to the `MaterialExtension` in bevy.

## What this won't do

This won't have complex lighting support or anything like that. It's mostly about the infrastructure for instanced rendering.

The standard implementations will only ever support simple colors and shapes.