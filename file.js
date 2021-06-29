let arr = new Array(1000000);
for (let i = 0; i < 100000; i++) {
    arr[i] = {};
}
gc();
