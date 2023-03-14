# 1.0.1

 - hide the ASCII_ARMOR symbol to work around a linking issue with 32-bit windows builds

# 1.0

 - `pem::parse_many` now returns a `Result<Vec<Pem>>` instead of a `Vec<Pem>` that silently discarded invalid sections.
