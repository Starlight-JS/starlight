function Node(left, right) {
    this.left = left;
    this.right = right;
}

function makeTree(depth) {
    if (depth <= 0) {
        return new Node(undefined, undefined);
    }

    return new Node(makeTree(depth - 1), makeTree(depth - 1));
}

makeTree(14);