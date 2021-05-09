assert = {
    sameValue: function (x, y, msg) {
        if (x !== y) {
            throw msg;
        }
    },
    throws: function (obj, cb, msg) {
        try {
            print("here");
            cb();
        } catch (e) {
            print(e);
            if (e instanceof obj) {
                return true;
            }
        }
        throw msg;
    }
}
function isConstructor(f) {
    try {
        Reflect.construct(function () { }, [], f);
    } catch (e) {
        return false;
    }
    return true;
}
assert.sameValue(
    isConstructor(RegExp.prototype[Symbol.split]),
    false,
    'isConstructor(RegExp.prototype[Symbol.split]) must return false'
);


assert.throws(TypeError, () => {
    let re = new RegExp(''); new re[Symbol.split]();
}, '`let re = new RegExp(\'\'); new re[Symbol.split]()` throws TypeError');

