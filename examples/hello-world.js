print("Hello, World!")

function equalOne(e){
    return e===1;
}

// const c = Function.prototype.call;

equalOne.call=null

print("Function prototype call: ",equalOne.call)

const a=[1,2,3]
print(a.some(equalOne))

const b = "1,2,3"
print(b.split(','))