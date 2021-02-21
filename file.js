function newArray(...rest) {
    let arr = [];
    arr.push(...rest);
    return arr;
}

print(newArray(1, 2, 3, 4));