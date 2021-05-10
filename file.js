function* foo() {
    return "foo";
}

print(typeof foo);
let gen = foo();

gen.next();
gen = 42;
gc();


print(gen);
