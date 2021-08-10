
function calcPi() {
    let inside = 0;
    let total = 0;
    for (let i = 0; i < 1000000; i++) {
        let x = Math.random();
        let y = Math.random();
        x *= x;
        y *= y;
        if ((x + y) <= 1) {
            inside += 1;
        }
        total += 1;
    }

    print('PI = ', 4 * inside / total);
}

calcPi();