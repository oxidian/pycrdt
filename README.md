[![Build Status](https://github.com/y-crdt/pycrdt/actions/workflows/test.yml/badge.svg?query=branch%3Amain++)](https://github.com/y-crdt/pycrdt/actions/workflows/test.yml/badge.svg?query=branch%3Amain++)
[![Code Coverage](https://img.shields.io/badge/coverage-100%25-green)](https://img.shields.io/badge/coverage-100%25-green)

# pycrdt

This is Oxidian's fork. Its `oxidian/*` branches build pycrdt against
[oxidian/y-crdt](https://github.com/oxidian/y-crdt), whose yrs deletes, garbage-collects and
decodes deeply nested documents without recursing on the native stack. A tag
`v<version>-oxidian.<n>` attaches the wheels to its GitHub release and lists them in the flat
package index at https://oxidian.github.io/pycrdt/. Nothing is published to PyPI.

CRDTs based on [Yrs](https://github.com/y-crdt/y-crdt/tree/main/yrs).

Documentation is available [here](https://y-crdt.github.io/pycrdt).
