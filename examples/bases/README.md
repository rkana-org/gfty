# Base configurations and presets

`2x2-magnetic.toml` is an independent `kind = "base"` configuration. Export it
without generating a bin body in the result:

```sh
gfty export examples/bases/2x2-magnetic.toml \
  --output base.step --image base.png
```

Gridfinity Ultimate still evaluates its unified Part Studio internally, but
gfty resolves the configured `Base` part ID and supplies it through the STEP
translation filter. Generic helper bodies are therefore excluded.

Flake definitions use `perSystem.gfty.bases`; base options do not live inside
bin definitions. The standard connector pin has no authored configuration and
is exported with:

```sh
gfty connector-pin export
```
