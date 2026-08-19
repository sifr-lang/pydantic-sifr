# Field and configuration demo

This dependent Sifr app verifies the M8 declaration surface. It uses
`BaseModel`, `Field`, `ConfigDict`, aliases, constraints, defaults, and schema
annotations.

The app also verifies nested models, sums, recursive models, concrete generic
models, and mapped special values.

Run it with the certified compiler:

```bash
"$SIFR_BIN" fetch --locked
"$SIFR_BIN" run --locked
```

The app exits successfully only when every assertion passes.
