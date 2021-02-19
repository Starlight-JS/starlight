var source = [3, 4, 5];
var target;

var callCount = 0;

new function () {
    assert.sameValue(arguments.length, 5);
    assert.sameValue(arguments[0], 1);
    assert.sameValue(arguments[1], 2);
    assert.sameValue(arguments[2], 3);
    assert.sameValue(arguments[3], 4);
    assert.sameValue(arguments[4], 5);
    assert.sameValue(target, source);
    callCount += 1;
}(1, 2, ...target = source);

assert.sameValue(callCount, 1);
