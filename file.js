for (let i = 0; i < 100; i = i + 1) {
    var obj = new Object()
    obj.x = 0
    print(i)
    for (; obj.x < 10000; obj.x = obj.x + 1) {

    }
}
