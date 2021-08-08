let arr = []

for (let i = 0; i < 10000; i++) {
    arr[i] = [];
}

arr = null;
gc();