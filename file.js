let re = new RegExp("javascript", "ig")
let str = 'blah blah JavaScript sucks';
let result;

print(re)
while (result = re.exec(str)) {
    print(result);
    print(result.index)
}