# starlight

Starlight is a JS engine in Rust which focuses on performance rather than ensuring 100% safety of JS runtime.


# Features
- Bytecode interpreter
- Mostly-precise GC without overhead for tracking GC objects
- Polymorphic inline caches for objects and variable lookups
- Small memory footprint, only around ~10KB of memory is used for a single runtime instance. 
    **NOTE**: memory consumption will be larger in the future when more of JS builtins will be implemented but it should not exceed 100KB I believe.

- Startup snapshots

# Why?

I was developing my own JS-like language but then I realized that there's no point in it and decided to try to create optimized JS engine with JIT,inline caches, fast GC and other fun stuff that is interesting me.

# Starlight VS Boa
Starlight is much faster than Boa in a lot of ways one of which is object property accesses but Starlight is designed more like JS engine that can be potentionally used in web browsers like V8 and JSC and not something small and embedabble (we have 1.6MB binary through!).

# Startup snapshots
The ES specification includes lots and lots of builtin objects. Every new single `Runtime` type instance has to set-up and initialize these builtins at the time runtime is created. It takes quite some time to do this from scratch.


To solve this problems V8-like startup snapshots were implemented. Startup snapshots just deserialize previously serialized heap state which might reduce load times singnificantly (not much difference right now since not that many builtins is implemented). Custom JS functions and objects could be added to global state and then serialized into binary file and deserialized later which removes overhead of initializing objects, parsing source code and compiling it to bytecode to just simple deserialization of raw bytes. Think of it like AOT compilation but simpler and smaller.


# TODO
- Complete support for full ES5.1 and some part of ES6 (we already support spread,const and let).
- Document startup snapshots
- Parallel GC marking
- Bytecode compiler
