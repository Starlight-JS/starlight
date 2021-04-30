let re = new RegExp("[0-9]+", "g");

let str = "2016-01-04";

print(Array.from(re[Symbol.matchAll](str)))