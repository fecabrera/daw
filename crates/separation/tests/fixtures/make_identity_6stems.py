"""Build a tiny ONNX identity model for six-stem adapter tests. No weights."""
from pathlib import Path


def varint(value):
    result = bytearray()
    while value >= 128:
        result.append((value & 127) | 128)
        value >>= 7
    return bytes(result + bytes([value]))


def integer(field, value):
    return varint(field << 3) + varint(value)


def message(field, value):
    if isinstance(value, str):
        value = value.encode()
    return varint(field << 3 | 2) + varint(len(value)) + value


def value(name, dims):
    shape = b"".join(message(1, integer(1, d) if isinstance(d, int)
                             else message(2, d)) for d in dims)
    tensor = integer(1, 1) + message(2, shape)  # FLOAT
    return message(1, name) + message(2, message(1, tensor))


def node(inputs, output, op, attributes=b""):
    return (b"".join(message(1, name) for name in inputs)
            + message(2, output) + message(4, op) + attributes)


axes = integer(1, 1) + integer(2, 7) + message(7, b"\x01") + message(8, "axes")
axis = message(5, message(1, "axis") + integer(3, 1) + integer(20, 2))
graph = (message(1, node(["mix", "axes"], "x", "Unsqueeze"))
         + message(1, node(["x"] * 6, "stems", "Concat", axis))
         + message(2, "identity_6stems") + message(5, axes)
         + message(11, value("mix", [1, 2, "n"]))
         + message(12, value("stems", [1, 6, 2, "n"])))
model = integer(1, 8) + message(7, graph) + message(8, integer(2, 17))
Path(__file__).with_name("identity_6stems.onnx").write_bytes(model)
