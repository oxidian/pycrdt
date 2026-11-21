"""Deeply nested documents must not overflow the stack of the thread that handles them."""

import random
import threading

import pytest
from pycrdt import Array, Doc, Map, XmlFragment, write_var_uint

# A thread this small overflows within a few thousand levels of recursion.
SMALL_STACK = 256 * 1024
LEVELS = 100_000

ITEM_TYPE = 7
HAS_PARENT_SUB = 0x20
TYPE_CONTENT = {
    "array": bytes([0]),
    "map": bytes([1]),
    "text": bytes([2]),
    "xml": bytes([3]) + write_var_uint(1) + b"p",
}
ROOT_TYPE = {"array": Array, "map": Map, "text": Array, "xml": XmlFragment}


def nested_update(kind: str, levels: int, *, deleted: bool) -> bytes:
    """A v1 update nesting `levels` shared types of `kind`, each the only child of the last."""
    key = b"k" if kind == "map" else None
    parts = [write_var_uint(1), write_var_uint(levels), write_var_uint(1), write_var_uint(0)]
    for clock in range(levels):
        info = ITEM_TYPE | (HAS_PARENT_SUB if key is not None else 0)
        if clock == 0:
            parent = write_var_uint(1) + write_var_uint(1) + b"a"
        else:
            parent = write_var_uint(0) + write_var_uint(1) + write_var_uint(clock - 1)
        if key is not None:
            parent += write_var_uint(len(key)) + key
        parts.append(bytes([info]) + parent + TYPE_CONTENT[kind])
    if deleted:
        parts += [write_var_uint(1)] * 3 + [write_var_uint(0), write_var_uint(levels)]
    else:
        parts.append(write_var_uint(0))
    return b"".join(parts)


ANY_CONTENT = 8
NESTED_VALUE = {"arrays": bytes([117, 1]), "maps": bytes([118, 1, 1]) + b"k"}


def nested_value_update(shape: str, levels: int) -> bytes:
    """A v1 update holding, in a root array, a null nested `levels` arrays or maps deep."""
    value = NESTED_VALUE[shape] * levels + bytes([126])
    return b"".join(
        [
            write_var_uint(1),
            write_var_uint(1),
            write_var_uint(1),
            write_var_uint(0),
            bytes([ANY_CONTENT]),
            write_var_uint(1),
            write_var_uint(1),
            b"a",
            write_var_uint(1),
            value,
            write_var_uint(0),
        ]
    )


def nesting_of(value) -> int:
    """How many lists or dicts, each holding only the next, a value read from
    `nested_value_update` nests its null inside."""
    levels = 0
    while isinstance(value, (list, dict)) and len(value) == 1:
        value = value[0] if isinstance(value, list) else value["k"]
        levels += 1
    assert value is None
    return levels


def on_small_stack(function):
    """Runs `function` on a thread with a small stack and returns what it raised, if anything."""
    raised: list[BaseException] = []

    def target() -> None:
        try:
            function()
        except BaseException as error:
            raised.append(error)

    previous = threading.stack_size(SMALL_STACK)
    try:
        thread = threading.Thread(target=target)
        thread.start()
    finally:
        threading.stack_size(previous)
    thread.join()
    return raised[0] if raised else None


@pytest.mark.parametrize("kind", sorted(TYPE_CONTENT))
@pytest.mark.parametrize("skip_gc", [False, True])
def test_applying_a_deleted_deep_nesting(kind, skip_gc):
    update = nested_update(kind, LEVELS, deleted=True)
    assert on_small_stack(lambda: Doc(skip_gc=skip_gc).apply_update(update)) is None


@pytest.mark.parametrize("kind", sorted(TYPE_CONTENT))
def test_deleting_the_top_of_a_deep_nesting(kind):
    update = nested_update(kind, LEVELS, deleted=False)

    def delete_top() -> None:
        doc = Doc()
        doc.apply_update(update)
        shared = doc.get("a", type=ROOT_TYPE[kind])
        with doc.transaction():
            if kind == "map":
                del shared["k"]
            elif kind == "xml":
                del shared.children[0]
            else:
                del shared[0]

    assert on_small_stack(delete_top) is None


@pytest.mark.parametrize("shape", sorted(NESTED_VALUE))
def test_reading_encoding_and_deleting_a_deeply_nested_value(shape):
    update = nested_value_update(shape, LEVELS)
    read: list[int] = []

    def read_encode_and_delete() -> None:
        doc = Doc()
        doc.apply_update(update)
        read.append(nesting_of(doc.get("a", type=Array)[0]))
        again = Doc()
        again.apply_update(doc.get_update())
        read.append(nesting_of(again.get("a", type=Array)[0]))
        del doc.get("a", type=Array)[0]

    assert on_small_stack(read_encode_and_delete) is None
    assert read == [LEVELS, LEVELS]


def random_value(rng: random.Random, depth: int):
    kinds = ["none", "bool", "int", "big int", "float", "str", "bytes"]
    if depth < 5:
        kinds += ["list", "tuple", "dict"] * 2
    kind = rng.choice(kinds)
    if kind == "none":
        return None
    if kind == "bool":
        return rng.random() < 0.5
    if kind == "int":
        return rng.randint(-(2**53) + 1, 2**53 - 1)
    if kind == "big int":
        # Odd past 2**53, so no float holds it exactly and it is encoded as a bigint.
        return rng.choice([-1, 1]) * (2**53 + 2 * rng.randrange(2**51) + 1)
    if kind == "float":
        return rng.uniform(-1e9, 1e9)
    if kind == "str":
        return "".join(rng.choice("ab\u00e9\U0001f600") for _ in range(rng.randint(0, 3)))
    if kind == "bytes":
        return bytes(rng.randrange(256) for _ in range(rng.randint(0, 3)))
    items = [random_value(rng, depth + 1) for _ in range(rng.randint(0, 3))]
    if kind == "list":
        return items
    if kind == "tuple":
        return tuple(items)
    return {f"k{index}": item for index, item in enumerate(items)}


def read_back(value):
    """`value` as pycrdt reads it back once written: an integer a float holds exactly as a float,
    bytes as a bytearray and a tuple as a list."""
    if isinstance(value, bool) or value is None or isinstance(value, (str, float)):
        return value
    if isinstance(value, int):
        return float(value) if -(2**53) < value < 2**53 else value
    if isinstance(value, bytes):
        return bytearray(value)
    if isinstance(value, dict):
        return {key: read_back(item) for key, item in value.items()}
    return [read_back(item) for item in value]


def same(left, right) -> bool:
    if type(left) is not type(right):
        return False
    if isinstance(left, list):
        return len(left) == len(right) and all(same(a, b) for a, b in zip(left, right))
    if isinstance(left, dict):
        return left.keys() == right.keys() and all(same(left[key], right[key]) for key in left)
    return left == right


def test_a_value_reads_back_as_written():
    rng = random.Random(14)
    for _ in range(500):
        value = random_value(rng, 0)
        doc = Doc()
        doc.get("m", type=Map)["k"] = value
        again = Doc()
        again.apply_update(doc.get_update())
        for read in (doc.get("m", type=Map)["k"], again.get("m", type=Map)["k"]):
            assert same(read, read_back(value)), (value, read)
