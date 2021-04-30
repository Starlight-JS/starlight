"use strict";
var obj = { bar: true, some: 1, foo: "a" };

var count = 0;

for (var property in obj) count++;

if (count !== 3) {
    $ERROR('#1: obj = {bar:true, some:1, foo:"a"}; count=0; for (property in obj) count++; count === 3. Actual: ' + (count));
}
//
////////////////////////////////////////////////////////

///////////////////////////////////////////////////////
// CHECK#2
var obj_ = { bar: true };
obj_.some = 1;
obj_.foo = "a";

count = 0;

for (property in obj_) count++;

if (count !== 3) {
    $ERROR('#2: obj_ = {bar:true}; obj_.some = 1; obj_.foo = "a"; count=0; for (property in obj_) count++; count === 3. Actual: ' + (count));
}
//
////////////////////////////////////////////////////////

///////////////////////////////////////////////////////
// CHECK#3
var obj__ = new Object();
obj__.bar = true;
obj__.some = 1;
obj__.foo = "a";

count = 0;

for (property in obj__) count++;

if (count !== 3) {
    $ERROR('#3: obj__ = new Object(); obj__.bar = true; obj__.some = 1; obj__.foo = "a"; for (property in obj__)  count++; count === 3. Actual: ' + (count));
}