
fixedbitset
===========

A simple bitset container for Rust

Please read the `API documentation here`__

__ https://docs.rs/fixedbitset/

|build_status|_ |crates|_

.. |build_status| image:: https://travis-ci.org/bluss/fixedbitset.svg?branch=master
.. _build_status: https://travis-ci.org/bluss/fixedbitset

.. |crates| image:: http://meritbadge.herokuapp.com/fixedbitset
.. _crates: https://crates.io/crates/fixedbitset

Recent Changes
--------------

- 0.1.9

  + Add intersection, union, difference iterators by @jrraymond
  + Add intersection: ``&`` and union: ``|`` operator implementations by @jrraymond
  + Add Extend and FromIterator implementations (from sequences of bit indices)
    by @jrraymond

- 0.1.8

  + Add missing ``#[inline]`` on the ones iterator
  + Fix docs for ``insert_range, set_range``

- 0.1.7

  + Add fast methods ``.insert_range``, ``.set_range`` by @kennytm

- 0.1.6

  + Add iterator ``.ones()`` by @mneumann
  + Fix bug with ``.count_ones()`` where it would erronously have an
    out-of-bounds panic for even block endpoints

- 0.1.5

  + Add method ``.count_ones(range)``.

- 0.1.4

  + Remove an assertion in ``.copy_bit(from, to)`` so that it is in line
    with the documentation. The ``from`` bit does not need to be in bounds.
  + Improve ``.grow()`` to use ``Vec::resize`` internally.

- 0.1.3

  + Add method ``.put()`` to enable a bit and return previous value

- 0.1.2

  + Add method ``.copy_bit()`` (by fuine)
  + impl Default

- 0.1.1

  + Update documentation URL

- 0.1.0

  + Add method ``.grow()``

License
-------

Dual-licensed to be compatible with the Rust project.

Licensed under the Apache License, Version 2.0
http://www.apache.org/licenses/LICENSE-2.0 or the MIT license
http://opensource.org/licenses/MIT, at your
option. This file may not be copied, modified, or distributed
except according to those terms.


