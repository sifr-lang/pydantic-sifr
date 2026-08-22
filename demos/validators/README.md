# Validators

This Sifr app demonstrates typed field validators in before, after, and plain
modes. It also demonstrates model validators in before and after modes.

The app covers multiple field targets, typed mutable context, checked callback
errors, and equal behavior for JSON and structural input. The statically typed
before-handler contract is an adapted Pydantic behavior.
Wildcard targets and wrap mode are not part of this surface.

Run it with the certified compiler:

```bash
"$SIFR_BIN" fetch --locked
"$SIFR_BIN" run --locked
```

The app exits successfully only when every assertion passes.
