var _gamma = 1; // learning constant
var _scale = 1; // scaling of the parameter

// activation function:
function _act(x) {
    return 1.0 / (1.0 + Math.exp(-x));
}

// activation function's derivative:
function _actDer(x) {
    var y = 1.0 + Math.exp(-x);
    return Math.exp(-x) / (y * y);
}

// end of defaults

function Neuron(options) {
    if (!options) {
        options = {};
    }
    this.act = options.act ? options.act : _act;
    this.actDer = options.actDer ? options.actDer : _actDer;
    this.gamma = options.gamma ? options.gamma : _gamma;
    this.scaleVal = options.scale ? options.scale : _scale;

    this.output = 0;
    this.inputs = [];
    this.bias = Math.random() - 0.5;
    this.isStale = false;
}

Neuron.prototype.scale = function (x) {
    return x / this.scaleVal;
}

Neuron.prototype.addInput = function (_neuron, weight) {
    if (!weight) {
        weight = Math.random() - 0.5;
    }
    this.inputs.push({
        n: _neuron,
        w: weight
    });
    this.resetOutput();
};

Neuron.prototype.getOutput = function () {
    if (!this.isStale) {
        return this.output;
    }

    var res = this.bias;
    for (var i = 0; i < this.inputs.length; i++) {
        res += this.inputs[i].n.getOutput() * this.inputs[i].w;
    }

    this.output = this.act(this.scale(res));
    return this.output;
};

Neuron.prototype.setOutput = function (val) {
    this.output = val;
    this.isStale = false;
}

Neuron.prototype.resetOutput = function (val) {
    this.output = 0;
    this.isStale = true;
}

// updating weights in current Neuron
Neuron.prototype.updateWeights = function (error) {
    var res = this.gamma * error * this.actDer(this.scale(this.getOutput()));
    for (var i = 0; i < this.inputs.length; i++) {
        this.inputs[i].w += this.inputs[i].n.getOutput() * res;
    }
    this.bias += parseFloat(this.gamma * error);
    this.resetOutput();
}
function Network(options) {
    this.nodeOptions = options ? options : {};
    this.sensors = [];
    this.nodes = [];
    this.output = new Neuron(this.nodeOptions);
    this.numberSensors = 0;
    this.numberNodes = 0;
}

Network.prototype.init = function (numberSensors, numberNodes) {
    for (var i = 0; i < numberSensors; i++) {
        var s = new Neuron;
        this.sensors.push(s);
    }
    for (var i = 0; i < numberNodes; i++) {
        var n = new Neuron(this.nodeOptions);
        this.nodes.push(n);
    }
    this.numberSensors = numberSensors;
    this.numberNodes = numberNodes;
    this.setConnections();
}

Network.prototype.setConnections = function () {
    for (var i = 0; i < this.nodes.length; i++) {
        var n = this.nodes[i];
        for (var j = 0; j < this.sensors.length; j++) {
            var s = this.sensors[j];
            n.addInput(s);
        }
        this.output.addInput(n);
    }
}

Network.prototype.setSensors = function (inputs) {
    if (this.sensors.length !== inputs.length) {
        throw "Number of inputs does nor coinside with number of sensors";
    }
    for (var i = 0; i < inputs.length; i++) {
        this.sensors[i].setOutput(inputs[i]);
    }
    for (var i = 0; i < this.nodes.length; i++) {
        this.nodes[i].resetOutput();
    }
    this.output.resetOutput();
}

// trainig via backpropagation
Network.prototype.train = function (inputs, answer) {
    this.setSensors(inputs);
    var output = this.output.getOutput();
    var error = parseFloat(answer - output);

    this.output.updateWeights(error);
    for (var i = 0; i < this.nodes.length; i++) {
        this.nodes[i].updateWeights(error * this.output.inputs[i].w);
    }
}

Network.prototype.test = function (inputs) {
    this.setSensors(inputs);
    return (this.output.getOutput());
}

var net = new Network({
    scale: 1, // scaling
    gamma: 1  // learning constant
    // act: (x) => x // custom activation function
    // actDer: (x) => 1 // derivative of custom activation function
});

net.init(2, 3);

function test() {
    return net.test([0, 0]) + ' ' +
        net.test([0, 1]) + ' ' +
        net.test([1, 0]) + ' ' +
        net.test([1, 1]);
}


print(test());

for (var i = 0; i < 10000; i++) {
    net.train([0, 0], 0);
    net.train([0, 1], 1);
    net.train([1, 0], 1);
    net.train([1, 1], 0);
}
print(test());
