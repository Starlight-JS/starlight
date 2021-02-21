function bar() {
    print(this)
}

let x = bar.bind(42)

x()