# Foundation benchmarks

This report measures the four main operations on one representative mapping of
string keys to integer lists. Parse measures bounded JSON parsing. Validate
uses an already parsed input arena. Construct validates that arena and creates
the typed Rust value. Serialize writes the typed value as JSON.

Run the benchmark with:

```bash
cargo bench -p pydantic_sifr_core --bench foundations -- --noplot
```

Measurement host: Apple M2 Pro, macOS 26.6.1, Rust 1.94.0. Criterion used
30 samples and a three-second measurement window. The measured implementation
commit is `f8ae63a6069186b0bf811c23649a74cdf5955b96`.

| Operation | Median time |
| --- | ---: |
| `parse/representative_json` | 1.537 us |
| `validate/representative_model` | 3.826 us |
| `construct/representative_model` | 6.633 us |
| `serialize/representative_model` | 4.122 us |

These numbers are descriptive measurements, not release budgets. Compare
results only on the same host and toolchain.
