#!/usr/bin/env bash

set -e

echo "Generating SchemeMut from Scheme"
sed 's/trait Scheme/trait SchemeMut/' scheme.rs \
| sed 's/\&self/\&mut self/g' \
> scheme_mut.rs

echo "Generating SchemeBlock from Scheme"
sed 's/trait Scheme/trait SchemeBlock/' scheme.rs \
| sed 's/fn handle(\&self, packet: \&mut Packet)/fn handle(\&self, packet: \&Packet) -> Option<usize>/' \
| sed 's/packet.a = Error::mux(res);/res.transpose().map(Error::mux)/' \
| sed 's/Result<usize>/Result<Option<usize>>/g' \
> scheme_block.rs

echo "Generating SchemeBlockMut from SchemeBlock"
sed 's/trait SchemeBlock/trait SchemeBlockMut/' scheme_block.rs \
| sed 's/\&self/\&mut self/g' \
> scheme_block_mut.rs
