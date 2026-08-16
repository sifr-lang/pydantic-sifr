# Model validation demo

This dependent Sifr app verifies the public `pydantic_sifr` model-validation
workflow. It covers JSON, strings, and structural input; nested models; field
constraints; aliases and alias paths; defaults; extra-field rejection; and a
stable public validation error.

Run it with the certified compiler:

```bash
"$SIFR_BIN" fetch --locked
"$SIFR_BIN" run --locked
```

The app exits successfully only when every assertion passes.
