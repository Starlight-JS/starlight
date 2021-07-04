function Foo() { }

Foo.prototype.a = 42;
let f = new Foo();
f.a = 43;

for (let i = 0; i < 10; i++) {
    f.a;
}