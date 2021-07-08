/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
let RegExpCtor = RegExp;
let strIncludes = String.prototype.includes;
let strIndexOf = String.prototype.indexOf;
let strCharCodeAt = String.prototype.charCodeAt;
let strSubstring = String.prototype.substring;
let arrayPush = Array.prototype.push;
let regexExec = RegExp.prototype.exec;
let regexBuiltinExec = regexExec;
let splitFast = RegExp.___splitFast;
let match = RegExp.prototype[Symbol.match];

let regExpExec = function (regexp, str) {
    "use strict";
    var exec = regexp.exec;
    var builtinExec = regexBuiltinExec;
    if (exec != builtinExec && ___isCallable(exec)) {
        var result = exec.___call(regexp, str);
        if (result !== null && !___isObject(result))
            throw new TypeError("The result of a RegExp exec must be null or an object");
        return result;
    }

    return builtinExec.___call(regexp, str);
}
RegExp.prototype[Symbol.matchAll] = function matchAll(strArg) {
    "use strict";

    var regExp = this;


    var string = strArg + "";
    var Matcher = RegExp;
    //var Matcher = @speciesConstructor(regExp, @RegExp);

    var flags = regExp.flags + "";
    var matcher = new RegExpCtor(regExp, flags);
    matcher.lastIndex = ___toLength(regExp.lastIndex);

    var global = strIncludes.___call(flags, "g");
    var fullUnicode = strIncludes.___call(flags, "u");//string.includes("u");

    return new RegExpStringIterator(matcher, string, global, fullUnicode);
}

let getSubstitution = function getSubstitution(matched, str, position, captures, namedCaptures, replacement) {
    "use strict";

    var matchLength = matched.length;
    var stringLength = str.length;
    var tailPos = position + matchLength;
    var m = captures.length;
    var replacementLength = replacement.length;
    var result = "";
    var lastStart = 0;
    if (strIncludes.___call(replacement, '$'))
        throw "TODO"
    for (var start = 0; start = strIndexOf.___call(replacement, "$", lastStart) /*replacement.indexOf("$", lastStart)*/, start !== -1; lastStart = start) {

        if (start - lastStart > 0)
            result = result + strSubstring.___call(replacement, lastStart, start) //replacement.substring(lastStart, start)
        start++;
        if (start >= replacementLength)
            result = result + "$";
        else {
            var ch = replacement[start];
            switch (ch) {
                case "$":
                    result = result + "$";
                    start++;
                    break;
                case "&":
                    result = result + matched;
                    start++;
                    break;
                case "`":
                    if (position > 0)
                        result = result + strSubstring.___call(str, 0, position) //str.substring(0, position)//@stringSubstringInternal.@call(str, 0, position);
                    start++;
                    break;
                case "'":
                    if (tailPos < stringLength)
                        result = result + strSubstring.___call(str, tailPos) //str.substring(tailPos)//@stringSubstringInternal.@call(str, tailPos);
                    start++;
                    break;
                case "<":
                    if (namedCaptures !== undefined) {
                        var groupNameStartIndex = start + 1;
                        var groupNameEndIndex = strIndexOf.___call(replacement, ">", groupNameStartIndex) //@stringIndexOfInternal.@call(replacement, ">", groupNameStartIndex);
                        if (groupNameEndIndex !== -1) {
                            var groupName = strSubstring.___call(replacement, groupNameStartIndex, groupNameEndIndex) //@stringSubstringInternal.@call(replacement, groupNameStartIndex, groupNameEndIndex);
                            var capture = namedCaptures[groupName];
                            if (capture !== undefined)
                                result = result + capture;

                            start = groupNameEndIndex + 1;
                            break;
                        }
                    }

                    result = result + "$<";
                    start++;
                    break;
                default:
                    var chCode = strCharCodeAt.___call(ch, 0) //ch.charCodeAt(0);
                    if (chCode >= 0x30 && chCode <= 0x39) {
                        var originalStart = start - 1;
                        start++;

                        var n = chCode - 0x30;
                        if (n > m) {
                            result = result + strSubstring.___call(replacement, originalStart, start);//@stringSubstringInternal.@call(replacement, originalStart, start);
                            break;
                        }

                        if (start < replacementLength) {
                            var nextChCode = strCharCodeAt.___call(replacement, start) //replacement.charCodeAt(start);
                            if (nextChCode >= 0x30 && nextChCode <= 0x39) {
                                var nn = 10 * n + nextChCode - 0x30;
                                if (nn <= m) {
                                    n = nn;
                                    start++;
                                }
                            }
                        }

                        if (n == 0) {
                            result = result + strSubstring.___call(replacement, originalStart, start);//@stringSubstringInternal.@call(replacement, originalStart, start);
                            break;
                        }

                        var capture = captures[n - 1];
                        if (capture !== undefined)
                            result = result + capture;
                    } else
                        result = result + "$";
                    break;
            }
        }
    }

    return result + strSubstring.___call(replacement, lastStart) //replacement.substring(lastStart);
}
RegExp.prototype[Symbol.replace] = function (strArg, replace) {
    "use strict";

    var regexp = this;
    var str = strArg + "";
    var stringLength = str.length;
    var functionalReplace = ___isCallable(replace);
    if (!functionalReplace)
        replace = replace + "";

    var global = regexp.global;
    var unicode = false;
    if (global) {
        unicode = regexp.unicode;
        regexp.lastIndex = 0;
    }

    var resultList = [];
    var result;
    var done = false;

    while (!done) {

        result = regExpExec(regexp, str) // regexp.exec(str);

        if (result === null)
            done = true;
        else {
            resultList.push(result);
            if (!global)
                done = true;
            else {
                var matchStr = result[0] + "";
                if (!matchStr.length) {
                    var thisIndex = ___toLength(regexp.lastIndex);
                    regexp.lastIndex = __advanceStringIndex__(str, thisIndex, unicode);
                }
            }
        }
    }
    var accumulatedResult = "";
    var nextSourcePosition = 0;

    for (var i = 0, resultListLength = resultList.length; i < resultListLength; ++i) {

        var result = resultList[i];
        var nCaptures = result.length - 1;
        if (nCaptures < 0)
            nCaptures = 0;
        var matched = result[0] + "";
        var matchLength = matched.length;
        var position = ___toIntegerOrInfinity(result.index);
        position = (position > stringLength) ? stringLength : position;
        position = (position < 0) ? 0 : position;

        var captures = [];
        for (var n = 1; n <= nCaptures; n++) {

            var capN = result[n];
            if (capN !== undefined)
                capN = capN + "";
            arrayPush.___call(captures, capN) //captures.push(capN);// @arrayPush(captures, capN);
        }

        var replacement;
        var namedCaptures = result.groups;

        if (functionalReplace) {
            var replacerArgs = [matched];
            for (var j = 0; j < captures.length; j++)
                arrayPush.___call(replacerArgs, captures[j]);

            // @arrayPush(replacerArgs, captures[j]);
            arrayPush.___call(replacerArgs, position);
            arrayPush.___call(replacerArgs, str);
            //replacerArgs.push(position);
            //replacerArgs.push(str);


            if (namedCaptures !== undefined)
                arrayPush.___call(replacerArgs, namedCaptures);
            //replacerArgs.push(namedCaptures)//@arrayPush(replacerArgs, namedCaptures);

            var replValue = replace.apply(undefined, replacerArgs);
            replacement = replValue + "";
        } else {
            if (namedCaptures !== undefined)
                namedCaptures = ___toObject(namedCaptures, "RegExp.prototype[Symbol.replace] requires 'groups' property of a match not be null");

            replacement = getSubstitution(matched, str, position, captures, namedCaptures, replace);
        }

        if (position >= nextSourcePosition) {
            accumulatedResult = accumulatedResult + strSubstring.___call(str, nextSourcePosition, position) + replacement//@stringSubstringInternal.@call(str, nextSourcePosition, position) + replacement;
            nextSourcePosition = position + matchLength;
        }
    }

    if (nextSourcePosition >= stringLength)
        return accumulatedResult;

    return accumulatedResult + strSubstring.___call(str, nextSourcePosition)// @stringSubstringInternal.@call(str, nextSourcePosition);
}


let hasObservableSideEffectsForSplit = function (regexp) {
    if (regexp[Symbol.match] !== match)
        return true;
    if (regexp.exec !== regexBuiltinExec)
        return true
    return typeof regexp.lastIndex !== "number";
}

RegExp.prototype[Symbol.split] = function (string, limit) {
    "use strict";

    let regexp = ___toObject(this, "RegExp.prototype.@@split requires that |this| be an Object");

    let str = string + "";
    let ctor = regexp.constructor;
    if (ctor === RegExpCtor && !hasObservableSideEffectsForSplit(regexp)) {
        return splitFast(regexp, str, limit);
    }
    let flags = regexp.flags;

    let unicodeMatching = strIncludes.___call(flags, "u");
    let newFlags = strIncludes.___call(flags, "y") ? flags : flags + "y";
    let splitter = new ctor(regexp, newFlags);
    if (!hasObservableSideEffectsForSplit(splitter))
        return splitFast(splitter, str, limit);

    let result = [];
    limit = (limit === undefined) ? 0xffffffff : limit >>> 0;
    if (!limit)
        return result;

    let size = str.length;
    if (!size) {
        var z = regExpExec(splitter, str);
        if (z !== null)
            return result;
        result[0] = str;
        return result;
    }
    // 15. [Defered from above] Let p be 0.
    var position = 0;
    // 18. Let q be p.
    var matchPosition = 0;

    // 19. Repeat, while q < size
    while (matchPosition < size) {
        // a. Perform ? Set(splitter, "lastIndex", q, true).
        splitter.lastIndex = matchPosition;
        // b. Let z be ? RegExpExec(splitter, S).
        var matches = regExpExec(splitter, str);
        // c. If z is null, let q be AdvanceStringIndex(S, q, unicodeMatching).
        if (matches === null)
            matchPosition = __advanceStringIndex__(str, matchPosition, unicodeMatching);
        // d. Else z is not null,
        else {
            // i. Let e be ? ToLength(? Get(splitter, "lastIndex")).
            var endPosition = ___toLength(splitter.lastIndex);
            // ii. Let e be min(e, size).
            endPosition = (endPosition <= size) ? endPosition : size;
            // iii. If e = p, let q be AdvanceStringIndex(S, q, unicodeMatching).
            if (endPosition === position)
                matchPosition = __advanceStringIndex__(str, matchPosition, unicodeMatching);
            // iv. Else e != p,
            else {
                // 1. Let T be a String value equal to the substring of S consisting of the elements at indices p (inclusive) through q (exclusive).
                var subStr = strSubstring.___call(str, position, matchPosition)
                // 2. Perform ! CreateDataProperty(A, ! ToString(lengthA), T).
                // 3. Let lengthA be lengthA + 1.
                arrayPush.___call(result, subStr);
                // 4. If lengthA = lim, return A.
                if (result.length == limit)
                    return result;

                // 5. Let p be e.
                position = endPosition;
                // 6. Let numberOfCaptures be ? ToLength(? Get(z, "length")).
                // 7. Let numberOfCaptures be max(numberOfCaptures-1, 0).
                var numberOfCaptures = matches.length > 1 ? matches.length - 1 : 0;

                // 8. Let i be 1.
                var i = 1;
                // 9. Repeat, while i <= numberOfCaptures,
                while (i <= numberOfCaptures) {
                    // a. Let nextCapture be ? Get(z, ! ToString(i)).
                    var nextCapture = matches[i];
                    // b. Perform ! CreateDataProperty(A, ! ToString(lengthA), nextCapture).
                    // d. Let lengthA be lengthA + 1.
                    arrayPush.___call(result, nextCapture);
                    // e. If lengthA = lim, return A.
                    if (result.length == limit)
                        return result;
                    // c. Let i be i + 1.
                    i++;
                }
                // 10. Let q be p.
                matchPosition = position;
            }
        }
    }
    // 20. Let T be a String value equal to the substring of S consisting of the elements at indices p (inclusive) through size (exclusive).
    var remainingStr = strSubstring.___call(str, position, size)
    // 21. Perform ! CreateDataProperty(A, ! ToString(lengthA), T).
    arrayPush.___call(result, remainingStr);
    // 22. Return A.
    return result;


}