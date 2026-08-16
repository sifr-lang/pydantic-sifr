# Sum validation demo

This dependent Sifr app verifies sum validation through the public
`pydantic_sifr` API. It covers literals, payload-free enums, smart unions,
field-discriminated tagged unions, recursive models, and labelled branch
errors.

Run it with the certified compiler:

```bash
"$SIFR_BIN" fetch --locked
"$SIFR_BIN" run --locked
```

The app exits successfully only when every assertion passes.
