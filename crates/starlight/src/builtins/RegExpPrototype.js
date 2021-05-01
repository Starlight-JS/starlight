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

let getSubstitution = function getSubstitution(matched, str, position, captures, namedCaptures, replacement) {
    "use strict";

    var matchLength = matched.length;
    var stringLength = str.length;
    var tailPos = position + matchLength;
    var m = captures.length;
    var replacementLength = replacement.length;
    var result = "";
    var lastStart = 0;

    for (var start = 0; start = replacement.indexOf("$", lastStart), start !== -1; lastStart = start) {
        if (start - lastStart > 0)
            result = result + replacement.substring(lastStart, start)
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
                        result = result + str.substring(0, position)//@stringSubstringInternal.@call(str, 0, position);
                    start++;
                    break;
                case "'":
                    if (tailPos < stringLength)
                        result = result + str.substring(tailPos)//@stringSubstringInternal.@call(str, tailPos);
                    start++;
                    break;
                case "<":
                    if (namedCaptures !== undefined) {
                        var groupNameStartIndex = start + 1;
                        var groupNameEndIndex = replacement.indexOf(">", groupNameStartIndex)//@stringIndexOfInternal.@call(replacement, ">", groupNameStartIndex);
                        if (groupNameEndIndex !== -1) {
                            var groupName = replacement.substring(groupNameStartIndex, groupNameEndIndex) //@stringSubstringInternal.@call(replacement, groupNameStartIndex, groupNameEndIndex);
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
                    var chCode = ch.charCodeAt(0);
                    if (chCode >= 0x30 && chCode <= 0x39) {
                        var originalStart = start - 1;
                        start++;

                        var n = chCode - 0x30;
                        if (n > m) {
                            result = result + replacement.substring(originalStart, start);//@stringSubstringInternal.@call(replacement, originalStart, start);
                            break;
                        }

                        if (start < replacementLength) {
                            var nextChCode = replacement.charCodeAt(start);
                            if (nextChCode >= 0x30 && nextChCode <= 0x39) {
                                var nn = 10 * n + nextChCode - 0x30;
                                if (nn <= m) {
                                    n = nn;
                                    start++;
                                }
                            }
                        }

                        if (n == 0) {
                            result = result + replacement.substring(originalStart, start);//@stringSubstringInternal.@call(replacement, originalStart, start);
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

    return result + replacement.substring(lastStart);
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
        result = regexp.exec(str);
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
                    regexp.lastIndex = ___advanceStringIndex(str, thisIndex, unicode);
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
            captures.push(capN);// @arrayPush(captures, capN);
        }

        var replacement;
        var namedCaptures = result.groups;

        if (functionalReplace) {
            var replacerArgs = [matched];
            for (var j = 0; j < captures.length; j++)
                replacerArgs.push(captures[j]);
            // @arrayPush(replacerArgs, captures[j]);

            replacerArgs.push(position);
            replacerArgs.push(str);


            if (namedCaptures !== undefined)
                replacerArgs.push(namedCaptures)//@arrayPush(replacerArgs, namedCaptures);

            var replValue = replace.apply(undefined, replacerArgs);
            replacement = replValue + "";
        } else {
            if (namedCaptures !== undefined)
                namedCaptures = ___toObject(namedCaptures, "RegExp.prototype[Symbol.replace] requires 'groups' property of a match not be null");

            replacement = getSubstitution(matched, str, position, captures, namedCaptures, replace);
        }

        if (position >= nextSourcePosition) {
            accumulatedResult = accumulatedResult + str.substring(nextSourcePosition, position) + replacement//@stringSubstringInternal.@call(str, nextSourcePosition, position) + replacement;
            nextSourcePosition = position + matchLength;
        }
    }

    if (nextSourcePosition >= stringLength)
        return accumulatedResult;

    return accumulatedResult + str.substring(nextSourcePosition)// @stringSubstringInternal.@call(str, nextSourcePosition);
}

