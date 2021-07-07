let assert = { sameValue: function () { } }

Array.print = print;
var r = /a/g;
Object.defineProperty(r, 'global', { writable: true });

r.lastIndex = 0;
r.global = undefined;
assert.sameValue(r[Symbol.replace]('aa', 'b'), 'ba', 'value: undefined');

r.lastIndex = 0;
r.global = null;
assert.sameValue(r[Symbol.replace]('aa', 'b'), 'ba', 'value: null');

r.lastIndex = 0;
r.global = false;
assert.sameValue(r[Symbol.replace]('aa', 'b'), 'ba', 'value: false');

r.lastIndex = 0;
r.global = NaN;
assert.sameValue(r[Symbol.replace]('aa', 'b'), 'ba', 'value: NaN');

r.lastIndex = 0;
r.global = 0;
assert.sameValue(r[Symbol.replace]('aa', 'b'), 'ba', 'value: global');

r.lastIndex = 0;
r.global = '';
assert.sameValue(r[Symbol.replace]('aa', 'b'), 'ba', 'value: ""');

var execCount = 0;
r = /a/;
Object.defineProperty(r, 'global', { writable: true });

r.exec = function (...args) {
    print(args);
    execCount += 1;
    if (execCount === 1) {
        return ['a'];
    }
    return null;
};

execCount = 0;
r.global = true; print('here')
r[Symbol.replace]('aa', 'b');
assert.sameValue(execCount, 2, 'value: true');
;
execCount = 0;
r.global = 86;
r[Symbol.replace]('aa', 'b');
assert.sameValue(execCount, 2, 'value: 86');

execCount = 0;
r.global = Symbol.replace;
r[Symbol.replace]('aa', 'b');
assert.sameValue(execCount, 2, 'value: Symbol.replace');

execCount = 0;
r.global = {};
r[Symbol.replace]('aa', 'b');
assert.sameValue(execCount, 2, 'value: {}');
