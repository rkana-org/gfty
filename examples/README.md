# gfty-label flake-parts example

This directory is a standalone flake using
`inputs.gfty-label.flakeModules.default`. Label and plate definitions live in
`labels.nix`; no project marker or generated TOML files are required.

Build one label or plate through the nested package outputs:

```sh
nix build ./examples#labels.module-example
nix build ./examples#plates.module-example
```

Build the combined label output:

```sh
nix build ./examples#labels.all
```

The combined derivation contains one symlink per label, named after its
`gfty-label.labels` attribute:

```text
result/
└── module-example -> /nix/store/…-module-example
```

Each individual label contains `label.svg` and `label.toml`. Each plate contains
`plate.svg`. Geometry JSON is generated in memory only by runtime export apps.

Every individual definition also exposes an explicit runtime export app:

```sh
nix run ./examples#export-label-module-example
nix run ./examples#export-plate-module-example
```

These commands obtain credentials through `gfty` at runtime, send the generated
geometry and pure-Nix Gridfinity configuration in API POST bodies, and download
the STEP to the current directory. Credentials never enter Nix evaluation or the
store.
