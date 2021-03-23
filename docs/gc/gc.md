# Garbage Collection

Starlight implements multiple garbage collection algorithms. Each implements `GarbageCollector` trait. 

## GC types available for use
- MiGC (default): Simple Mark&Sweep GC that uses mimalloc for allocation and has fast parallel marking. 
- MallocGC: Mark&Sweep GC that uses libc's `malloc` and `free` functions to allocate memory. This collector is used mostly for debugging.

## TODO
- Semispace GC
