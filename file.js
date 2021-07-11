let bar = [];
let foo = new WeakRef(bar);
bar = 42;
gc()

print(foo.deref());