# Contributing guidelines

## Code style

- Before committing, run the following checks:

```sh
cargo +nightly fmt -- --check
cargo clippy --all --tests -F precommit-checks -- -D warnings
cargo test --all
```

- Use the structure of `foo.rs`, `foo/submodule.rs`, etc. Do not use `foo/mod.rs`.
- Do not import `bevy::prelude::*`,
  nor to import the re-exports from `bevy::prelude` if a direct import is possible.
- Unit tests should be under a separate tests.rs file
  and included from the parent with `#[cfg(test)] mod tests;`.
- Use `distance_cmp` and `magnitude_cmp` for comparing vector norms.
  Do not use the exact or squared methods for comparisons alone.
- Use `Vec::from([...])` instead of `vec![...]` for Vec literals.
  Large expressions in macros tend to be unfriendly to rust-analyzer.
  - For consistency, use `Vec::new()` instead of `vec![]` for empty Vecs.
- Use the `TryLog` extension trait for getting components from `World` or `Query`
  if absence of the component would be a bug.
  Similarly, use `try_log`/`try_log_return` where suitable.
  The `None` branch should result in termination of processing of the current entity,
  unless the system involves aggregation over all queried entities,
  in which case the aggregation result must not be used
  to avoid propagating errors.
