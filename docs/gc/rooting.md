# Rooting

This guide explains the basics of interacting with Starlight's GC as a Starlight API user. Since Starlight has a precise GC, it is very important that it knows about each and every pointer to a GC thing in the system. Starlight's rooting API tries to make this task as simple as possible.

## What is GC thing pointer?

"GC thing" is the term used to refer to memory allocated and managed by the Starlight garbage collector. The main types of GC thing pointer are:

- `JsValue`
- `Rooted<T>`
- `WeakRef<T>`


Note that JsValue can contain pointers internally even though they are not pointer types.

If you use these types directly or create structs or arrays that contain them, you must follow the rules set out in this guide. If you do not your program will not work correctly - if it works at all.

## GC things on the stack

### `GcPointer<T>`,`T:Trace`,`WeakRef<T>`,`JsValue`

All GC thing pointers on the stack (i.e local variables and paramters to functions) must use `Rooted<T>` type or be a reference to `Rooted<T>`. This is a generic structure where the generic parameter is the type GC can trace (i.e `GcPointer<T>`), this means you can have any type that implements `Trace` stored in `Rooted<T>`. For creating new locals `letroot!` macro should be used. 
## GC things on the heap

### `GcPointer<T>`,`WeakRef<T>`

GC thing pointers on the heap must be wrapped in a `GcPointer<T>` or in `WeakRef<T>`. `GcPointer<T>` and `WeakRef<T>` pointers must also continue to be traced in the normal way, which is covered below.
## Conservative roots

Starlight has support for conservative root scanning but if your type boxes some of heap pointers you still have to use `letroot!` but otherwise you do not need to use `letroot!`.
## Tracing

All GC pointers stored on the heap must be traced. For regular runtime `Trace`able objects, this is normally done by storing them in slots, which are automatically traced by the GC

### General structures

For a regular `struct`, tracing must be triggered manually. The usual way is to add tracing code into `fn trace(&self,tracr: &mut dyn Tracer)` in `Trace` impl for your struct. (`GcTrace` derive macro from `starlight_derive` does this automatically )

## Summary

- Use `Rooted<T>` for local variables and function parameters.
- Use `GcPointer<T>` for heap data and `WeakRef<T>` for weak heap references. 
- Do not use `&T` or `&mut T` on the heap.

## Example code
```rust
fn my_function_that_allocates(rt: &mut Runtime) {
    let shadowstack = rt.shadowstack(); // get global shadowstack, returned reference is tied to lifetime of current scope.
    letroot!(my_object = shadowstack, MyObject::new(rt)); // root! macro internally does zero heap allocations, shadowstack stores pinned references to stack values.
    // my_object will be traced.
    rt.gc();
}
```