function Foo(x, y, outer) {
    this.outer = outer;
}

const BAR = new Foo(0, 0, undefined);

let far = new Foo(1, 2, BAR);

print(far, far == BAR);