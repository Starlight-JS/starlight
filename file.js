
let RegExpCtor = RegExp;
let strIncludes = String.prototype.includes;
let strIndexOf = String.prototype.indexOf;
let strCharCodeAt = String.prototype.charCodeAt;
let strSubstring = String.prototype.substring;
let arrayPush = Array.prototype.push;
let regexExec = RegExp.prototype.exec;
let splitFast = RegExp.___splitFast;
let match = RegExp.prototype[Symbol.match];
RegExp.prototype[Symbol.split] = function split(string, limit) {
    "use strict";

    let regexp = ___toObject(this, "RegExp.prototype.@@split requires that |this| be an Object");

    let str = string + "";
    let ctor = regexp.constructor;
    if (ctor === RegExpCtor && !hasObservableSideEffectsForSplit(regexp)) {
        return splitFast(regexp, str, limit);
    }
    let flags = regexp.flags;

    let unicodeMatching = strIncludes.call(flags, "u");
    let newFlags = strIncludes.call(flags, "y") ? flags : flags + "y";
    let splitter = new ctor(regexp, newFlags);
    if (!hasObservableSideEffectsForSplit(splitter))
        return splitFast(splitter, str, limit);

    let result = [];
    limit = (limit === undefined) ? 0xffffffff : limit >>> 0;
    if (!limit)
        return result;

    let size = str.length;
    if (!size) {
        var z = regexExec.call(splitter, str);
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
        var matches = regexExec.call(splitter, str);
        // c. If z is null, let q be AdvanceStringIndex(S, q, unicodeMatching).
        if (matches === null)
            matchPosition = ___advanceStringIndex(str, matchPosition, unicodeMatching);
        // d. Else z is not null,
        else {
            // i. Let e be ? ToLength(? Get(splitter, "lastIndex")).
            var endPosition = ___toLength(splitter.lastIndex);
            // ii. Let e be min(e, size).
            endPosition = (endPosition <= size) ? endPosition : size;
            // iii. If e = p, let q be AdvanceStringIndex(S, q, unicodeMatching).
            if (endPosition === position)
                matchPosition = ___advanceStringIndex(str, matchPosition, unicodeMatching);
            // iv. Else e != p,
            else {
                // 1. Let T be a String value equal to the substring of S consisting of the elements at indices p (inclusive) through q (exclusive).
                var subStr = strSubstring.call(str, position, matchPosition)
                // 2. Perform ! CreateDataProperty(A, ! ToString(lengthA), T).
                // 3. Let lengthA be lengthA + 1.
                arrayPush.call(result, subStr);
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
                    arrayPush.call(result, nextCapture);
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
    var remainingStr = strSubstring.call(str, position, size)
    // 21. Perform ! CreateDataProperty(A, ! ToString(lengthA), T).
    arrayPush.call(result, remainingStr);
    // 22. Return A.
    return result;


}
function isConstructor(f) {
    try {
        Reflect.construct(function () { }, [], f);
    } catch (e) {
        return false;
    }
    return true;
}

isConstructor(RegExp.prototype[Symbol.split]);


let re = new RegExp(''); new re[Symbol.split]();