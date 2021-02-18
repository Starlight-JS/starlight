# Rooting

This guide explains the basics of interacting with Starlight's GC as a Starlight API user. Since Starlight has a moving GC, it is very important that it knows about each and every pointer to a GC thing in the system. Starlight's rooting API tries to make this task as simple as possible.

## What is GC thing pointer?

"GC thing" is the term used to refer to memory allocated and managed by the Starlight garbage collector. The main types of GC thing pointer are:

- `JsValue`
- `Gc<T>`
- `Handle<T>`
Note that JsValue can contain pointers internallly even though they are not pointer types.

If you use these types directly, or create structs or arrays that contain them, you must follow the rules set out in this guide. If you do not your program will not work correctly - if it works at all.

## GC things on the stack

### `Gc<T>`,`T:Trace`

All GC thing pointers on the stack (i.e local variables and paramters to functions) must use `Handle<T>` type. This is a generic reference counted structure where the generic parameter is the type GC can trace (i.e `Gc<T>`), this means you can have any type that implements `Trace` allocated in `Handle<T>`. For creating new locals `Gc::root` or `Handle::new` should be used. 
## GC things on the heap

### `Gc<T>`

GC thing pointers on the heap must be wrapped in a `Gc<T>`. `Gc<T>` pointers must also continue to be traced in the normal way, which is covered below.

## Tracing

All GC pointers stored on the heap must be traced. For regular runtime `Trace`able objects, this is normally done by storing them in slots, which are automatically traced by the GC

### General structures

For a regular `struct`, tracing must be triggered manually. The usual way is to add tracing code into `fn trace(&self,tracr: &mut dyn Tracer)` in `Trace` impl for your struct. (`Trace` derive macro from `starlight_derive` does this automatically )

## Summary

- Use `Handle<T>` for local variables and function parameters.
- Use `Gc<T>` for heap data. **Note: `Gc<T>` are not rooted: they must be traced!**
- Do not use `&T` or `&mut T` on the heap.
