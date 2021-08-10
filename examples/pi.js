
function calcPi() {
    let inside = 0;
    let total = 0;
    for (let i = 0; i < 1000000; i++) {
        let x = Math.pow(Math.random(), 2);
        let y = Math.pow(Math.random(), 2);
        if ((x + y) <= 1) {
            inside += 1;
        }
        total += 1;
    }

    print('PI = ', 4 * inside / total);
}

calcPi();