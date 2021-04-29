RegExp.prototype[Symbol.matchAll] = function matchAll(strArg) {
    "use strict";

    var regExp = this;


    var string = strArg + "";
    var Matcher = RegExp;
    //var Matcher = @speciesConstructor(regExp, @RegExp);

    var flags = regExp.flags + "";
    var matcher = new RegExp(regExp, flags);
    matcher.lastIndex = ___toLength(regExp.lastIndex);

    var global = flags.includes("g");
    var fullUnicode = string.includes("u");

    return new RegExpStringIterator(matcher, string, global, fullUnicode);
}