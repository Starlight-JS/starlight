# Garbage Collection

Starlight has tracing garbage collector that is used for almost every single runtime object. We implement simple Mark&Sweep GC with segregated allocation scheme. 


# Algorithm overview

- GC starts either when allocated bytes exceed threshold or user requests GC.
- All `LocalContext`s and `PersistentContext` is scanned for roots.
- Marking constraints is executed for obtaining root objects.
- Marking cycle is performed.
- Small arenas blocks are sweeped.
- Precise allocations are sweeped.
- GC is done and execution continues.

## Finalization

GC invokes `drop` on all GC allocated cells when they're dead but note that finalization order is not guaranteed in any way, other GC objects should not be accessed in finalizer.

## Large object space

For large objects (>4KB) runtime has special heap space which uses system malloc and free functions for allocation and deallocation. Usually allocation and sweeping for this space is slower than the same operations for small arena space so try not to allocate large objects too often.