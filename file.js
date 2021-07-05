let buffer = new ArrayBuffer(128);

let view = new DataView(buffer, 0, 8);
view.setUint8(0, 42);
print(view.getUint16(0));