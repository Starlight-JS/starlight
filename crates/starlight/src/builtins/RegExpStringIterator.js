/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
let defineProperty = Object.defineProperty;
let charCodeAt = String.charCodeAt;
let regexExec = RegExp.prototype.exec;
function RegExpStringIterator(regexp, string, global, fullUnicode) {
    "use strict";
    defineProperty(this, "regExpStringIteratorRegExp", {
        value: regexp,
        writable: false,
        configurable: false,
        enumerable: false
    });
    defineProperty(this, "regExpStringIteratorString", {
        value: string,
        writable: false,
        configurable: false,
        enumerable: false
    });
    defineProperty(this, "regExpStringIteratorGlobal", {
        value: global,
        writable: false,
        configurable: false,
        enumerable: false
    });
    defineProperty(this, "regExpStringIteratorUnicode", {
        value: fullUnicode,
        writable: false,
        configurable: false,
        enumerable: false
    });

    defineProperty(this, "regExpStringIteratorDone", {
        value: false,
        writable: true,
        configurable: false,
        enumerable: false
    });
}

function ___advanceStringIndex(string, index, unicode) {
    // This function implements AdvanceStringIndex described in ES6 21.2.5.2.3.
    "use strict";

    if (!unicode)
        return index + 1;

    if (index + 1 >= string.length)
        return index + 1;

    var first = charCodeAt.call(string, index);
    if (first < 0xD800 || first > 0xDBFF)
        return index + 1;

    var second = charCodeAt.call(string, index + 1); //string.charCodeAt(index + 1);
    if (second < 0xDC00 || second > 0xDFFF)
        return index + 1;

    return index + 2;
}

RegExpStringIterator.prototype.next = function next() {
    "use strict";

    var done = this.regExpStringIteratorDone;
    if (done === undefined)
        throw new TypeError("%RegExpStringIteratorPrototype%.next requires |this| to be an RegExp String Iterator instance");


    if (done)
        return {
            value: undefined,
            done: true
        }


    var regExp = this.regExpStringIteratorRegExp;
    var string = this.regExpStringIteratorString;
    var global = this.regExpStringIteratorGlobal;
    var fullUnicode = this.regExpStringIteratorUnicode
    var match = regexExec.call(regExp, string); //regExp.exec(string);
    if (match === null) {
        this.regExpStringIteratorDone = true;
        return { value: undefined, done: true }
    }

    if (global) {
        var matchStr = match[0] + "";
        if (matchStr === "") {
            var thisIndex = ___toLength(regExp.lastIndex);
            regExp.lastIndex = ___advanceStringIndex(string, thisIndex, fullUnicode);
        }
    } else {
        this.regExpStringIteratorDone = true;
    }
    return {
        value: match, done: false
    }
}

Object.defineProperty(RegExpStringIterator.prototype, Symbol.iterator, {
    get: function () {
        return this
    }
})