function foo(x, y) {
    arguments[0] = y
    print(x)
}
foo(4, 6)