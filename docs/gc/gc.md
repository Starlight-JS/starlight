# Garbage Collection

As of [1790d15a](https://github.com/Starlight-JS/starlight/commit/1790d15a01ed358d9bf8dc93b5ffac4899b13a10) Starlight uses a new garbage collector. This new GC utilizes a segregated free list for small allocations and uses mimalloc for large allocations.


## Algorithm

- *Marking*: The collector marks objects as it finds references to them. Objects not marked are deleted. Most of the collector’s time is spent visiting objects to find references to other objects.
- *Constraints*: The collector allows the runtime to supply additional constraints on when objects should be marked, to support custom object lifetime rules.
- *Conservatism*: The collector scans the stack and registers conservatively, that is, checking each word to see if it is in the bounds of some object and then marking it if it is. This means that all of the Rust, assembly, and just-in-time (JIT) compiler-generated code in our system can store heap pointers in local variables without any hassles.
- *Efficiency*: This is our always-on garbage collector. It has to be fast.

## Simple Segregated Storage
- Small and medium-sized objects are allocated from segregated free lists. Given a desired object size, we perform a table lookup to find the appropriate free list and then pop the first object from this list. The lookup table is usually constant-folded by the compiler.
- Memory is divided into 16KB [*blocks*](https://github.com/Starlight-JS/starlight/blob/dev/crates/starlight/src/gc/block.rs). Each block contains [*cells*](https://github.com/Starlight-JS/starlight/blob/dev/crates/starlight/src/gc/cell.rs). All cells in a block have the same cell size, called the block’s size class. The GC literature would typically use *object* to refer to what our code would call a *cell*. Since this post is not really concerned with JavaScript types, we’ll use the term object to mean any cell in our heap.
- At any time, the active free list for a size class contains only objects from a single block. When we run out of objects in a free list, we go to next available block or allocate new one.


Large objects (larger than about 8KB) are allocated using malloc.

## Limited heap size

Heap size for segregated storage is limited and by default, this limit is set to 2GB. This allows users of Starlight to set some limit of memory their JS program can use.

## Conservative Roots
Garbage collection begins by looking at local variables and some global state to figure out the initial set of marked objects. Introspecting the values of local variables is tricky. Starlight uses Rust local variables for pointers to the garbage collector’s heap, but C-like languages provide no facility for precisely introspecting the values of specific variables of arbitrary stack frames. Starlight solves this problem by marking objects conservatively when scanning roots. Since our heap size is limited we can utilize simple bitmap to check if pointer on stack is heap allocated. 
We view this as an important optimization. Without conservative root scanning, Rust code would have to use some API to notify the collector about what objects it points to. Conservative root scanning means not having to do any of that work.