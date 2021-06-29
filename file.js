let i = 0;
function Node(l, r) {
    this.left = l;
    this.right = r;
    //this.counter = i++;
}

function makeTree(depth) {
    if (depth <= 0) {
        return new Node();
    }

    return new Node(makeTree(depth - 1), makeTree(depth - 1))
}

let n = makeTree(17);

print(n.counter);