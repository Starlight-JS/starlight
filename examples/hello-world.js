print('Hello, World!');

function equalOne(e) {
  return e === 1;
}

equalOne.call = null;

print('Function prototype call: ', equalOne.call);

const a = [1, 2, 3];
print(a.some(equalOne));

const b = '1,2,3';
print(b.split(','));

function foo() {
  try {
    if (1) {
      return 3;
    }
    return 1;
  } finally {
    return 2;
  }
}

print(foo());
