/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */


function ___ArrayIterator(array, kind) {
    Object.defineProperty(this, 'IteratedObject', {
        value: array,
        writable: true,
        configurable: false,
        enumerable: false
    });
    Object.defineProperty(this, 'ArrayIteratorNextIndex', {
        value: 0,
        writable: true,
        configurable: false,
        enumerable: false
    });
    Object.defineProperty(this, 'ArrayIteratorKind', {
        value: kind,
        writable: false,
        configurable: false,
        enumerable: false
    });
}

___ArrayIterator.prototype.next = function next() {
    let o = this;
    let a = this.IteratedObject;
    if (a === undefined)
        return {
            value: undefined,
            done: true
        }

    let index = this.ArrayIteratorNextIndex;

    let len = ___toLength(a.length);
    if (index >= len) {
        this.IteratedObject = undefined;
        return {
            value: undefined, done: true
        }
    }

    this.ArrayIteratorNextIndex = index + 1;
    let kind = this.ArrayIteratorKind;
    if (kind === "key")
        return {
            value: index,
            done: false
        }
    else if (kind == "value")
        return {
            value: a[index],
            done: false
        }
    else
        return {
            value: [index, a[index]],
            done: false
        }
}

___ArrayIterator.prototype[Symbol.iterator] = function () {
    return this;
}