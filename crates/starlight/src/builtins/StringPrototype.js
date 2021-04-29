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