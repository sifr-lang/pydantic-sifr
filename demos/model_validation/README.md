# Model validation

This Sifr app demonstrates the public model API.

It covers JSON, string, and structural input. It also covers nested models,
aliases, constraints, defaults, typed errors, dumps, and JSON Schema output.

The app includes `RootModel[T]`, `TypeAdapter[T]`, and a concrete generic model.

Run the app with the certified compiler:

```bash
"$SIFR_BIN" fetch --locked
"$SIFR_BIN" run --locked
```

The app exits successfully only when every assertion passes.
