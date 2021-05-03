/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
let RegExpCtor = RegExp;
let RegExpReplace = RegExp.prototype[Symbol.replace];
let symReplace = Symbol.replace;
let __replace = String.___replace;
String.prototype.match = function match(regexp) {
    "use strict";
    if (this === null | this === undefined) {
        throw new TypeError("String.prototype.match requires that |this| not be null or undefined")
    }

    if (regexp) {
        var matcher = regexp[Symbol.match];
        if (matcher) {
            return matcher.call(regexp, this);
        }
    }

    var thisString = this + "";
    var createdRegExp = new RegExp(regexp, undefined);
    return createdRegExp[Symbol.match](thisString);
}

String.prototype.matchAll = function matchAll(arg) {
    "use strict";
    if (this === null | this === undefined) {
        throw new TypeError("String.prototype.matchAll requires |this| not to be null nor undefined")
    }

    if (arg) {
        if (arg instanceof RegExp && arg.flags.includes("g"))
            throw new TypeError("String.prototype.matchAll argument must not be a non-global regular expression")

        var matcher = arg[Symbol.matchAll];
        if (!matcher) {
            return matcher.call(arg, this);
        }
    }

    var string = arg + "";
    var regExp = new RegExp(arg, "g");
    return regExp[Symbol.matchAll](string);
}

let hasObservableSideEffectsForStringReplace = function (regexp, replacer) {
    "use strict";
    if (!(regexp instanceof RegExpCtor)) {
        return true;
    }
    if (replacer !== RegExpReplace) {
        return true;
    }

    return typeof regexp.lastIndex !== "number";
}
String.prototype.replace = function replace(search, replace) {
    "use strict";

    if (this == undefined | this == null)
        throw new TypeError("String.prototype.replace requires that |this| not be null or undefined");

    if (search !== undefined & search !== null) {
        var replacer = search[symReplace];
        if (replacer) {
            return replacer.call(search, this, replace);
        }
    }

    return __replace.call(this, search, replace);
}
let split_sym = Symbol.split;
let fastSplit = String.prototype.___splitFast;

String.prototype.split = function (separator, limit) {
    "use strict";
    if (this === undefined | this === null)
        throw new TypeError("String.prototype.split requires that |this| not be null or undefined")

    if (separator !== undefined & separator !== null) {
        var splitter = separator[split_sym];
        if (splitter !== undefined & splitter !== null) {
            return splitter.call(separator, this, limit);
        }
    }
    return fastSplit.call(this, separator, limit);
}