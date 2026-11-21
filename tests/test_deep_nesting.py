"""Deeply nested documents must not overflow the stack of the thread that handles them."""

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


def nested_any_update(levels: int) -> bytes:
    """A v1 update holding one value nested `levels` arrays deep in a root array."""
    value = bytes([117, 1]) * levels + bytes([126])
    return b"".join(
        [
            write_var_uint(1),
            write_var_uint(1),
            write_var_uint(1),
            write_var_uint(0),
            bytes([8]),
            write_var_uint(1),
            write_var_uint(1),
            b"a",
            write_var_uint(1),
            value,
            write_var_uint(0),
        ]
    )


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


def test_deeply_nested_value_is_refused():
    update = nested_any_update(LEVELS)
    raised = on_small_stack(lambda: Doc().apply_update(update))
    assert isinstance(raised, ValueError)


def test_value_at_the_nesting_limit_is_accepted():
    doc = Doc()
    doc.apply_update(nested_any_update(256))
    value = doc.get("a", type=Array)[0]
    for _ in range(255):
        (value,) = value
    assert value == [None]


def test_value_past_the_nesting_limit_is_refused():
    with pytest.raises(ValueError):
        Doc().apply_update(nested_any_update(257))
