function main(foo) {
    function bar() {
        let array = [];
        for (let i = 0; i < 10000; i++)
            array[i] = [];
        array = 42;
    }
    bar();
}

main();

gc();
while (true) { }