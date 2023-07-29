# Atomo

Atomo is a common-interface database wrapper which provides high-throughput consistent-view using an
in-memory snapshot functionality.

See the examples directory for API uses.

# Why

Although any complex persistent engine is most likely already equipped with snapshot functionality,
our use specific use case of having execution engines purely in Rust compelled us to have our optimized
implementation that doesn't rely on correctness of the snapshot functionality of the underlying persistence
layer and therefore act as an additional layer of correctness.

# Work in progress

1. Writing more tests and fuzz test.
2. Provide a generic for binding with arbitrary back ends.
