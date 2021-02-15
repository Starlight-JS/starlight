# Starlight VM internals

## Memory management

Starlight uses tracing GC for memory management and implements Mark&Sweep algorithm. Read more in [gc/gc.md](gc/gc.md)

## Execution pipeline

Starlight has a multi-tiered execution pipeline. First function code is interpreted in our interpreter called `photon` for fast startup and after a few hunderds loop iteration or function calls it will be JITed to native code for high throughput.
