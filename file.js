function foo() {
    'use strict';
    return typeof (this);
}

function bar() {
    return typeof (this);
}


print(foo())
print(bar())