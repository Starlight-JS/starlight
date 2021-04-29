var argObj = (function (a, b, c) {
    return arguments;
})(1, 2, 3);
var accessed = false;

Object.defineProperty(argObj, 0, {
    get: function () {
        print("getter");
        accessed = true;
        return 12;
    }
});
print(argObj[0]);
print(accessed);