# Validator demo

This dependent Sifr app verifies the M9 validator declaration surface. It uses
typed field validators in before, after, and plain modes. It also uses typed
model validators in before and after modes.

The demo also verifies multiple field targets, typed mutable context, checked
callback errors, and equal validator behavior for JSON and structural input.
The statically typed before-handler contract is an adapted Pydantic behavior.
Wildcard targets and wrap mode are not part of this surface.

Run it with the certified compiler:

```bash
"$SIFR_BIN" fetch --locked
"$SIFR_BIN" run --locked
```
