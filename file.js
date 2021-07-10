var localesInputs = [null, [NaN], ["i"], ["de_DE"]];
var optionsInputs = [
    { localeMatcher: null },
    { style: "invalid" },
    { style: "currency" },
    { style: "currency", currency: "ßP" },
    { maximumSignificantDigits: -Infinity }
];

for (const locales of localesInputs) {
    var referenceError, error;
    try {
        var format = new Intl.NumberFormat(locales);
    } catch (e) {
        referenceError = e;
    }
    assert.notSameValue(referenceError, undefined, "Internal error: Expected exception was not thrown by Intl.NumberFormat for locales " + locales + ".");

    assert.throws(referenceError.constructor, function () {
        var result = 0n.toLocaleString(locales);
    }, "BigInt.prototype.toLocaleString didn't throw exception for locales " + locales + ".");
}

for (const options of optionsInputs) {
    var referenceError, error;
    try {
        var format = new Intl.NumberFormat([], options);
    } catch (e) {
        referenceError = e;
    }
    assert.notSameValue(referenceError, undefined, "Internal error: Expected exception was not thrown by Intl.NumberFormat for options " + JSON.stringify(options) + ".");

    assert.throws(referenceError.constructor, function () {
        var result = 0n.toLocaleString([], options);
    }, "BigInt.prototype.toLocaleString didn't throw exception for options " + JSON.stringify(options) + ".");
}
