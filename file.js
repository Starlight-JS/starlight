let re = new RegExp("[0-9]+", "g")


const str = '2016-01-02|2019-03-07';
const result = re[Symbol.matchAll](str);
Array.from(result, (x) => print(x))