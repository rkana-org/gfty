# gfty-label flake-parts example

This directory is a standalone flake using
`inputs.gfty-label.flakeModules.default`. Named bin, label, and plate definitions
live in `labels.nix`; no project marker or checked-in generated files are
required.

Build one bin, label, or plate through the nested package outputs:

```sh
nix build ./examples#bins.module-example
nix build ./examples#labels.module-example
nix build ./examples#plates.module-example
```

Build the combined bin or label outputs:

```sh
nix build ./examples#bins.all
nix build ./examples#labels.all
```

The combined derivation contains one symlink per label, named after its
`gfty-label.labels` attribute:

```text
result/
└── module-example -> /nix/store/…-module-example
```

Each bin contains `bin.toml`, each label contains `label.svg` and `label.toml`,
and each plate contains `plate.svg`. Geometry JSON is generated in memory only by runtime export apps.

Every individual definition also exposes an explicit runtime export app:

```sh
nix run ./examples#export-bin-module-example
nix run ./examples#export-label-module-example
nix run ./examples#export-plate-module-example
```

These commands obtain credentials through `gfty` at runtime, send the generated
geometry and the typed named-bin configuration in API POST bodies, and download
the STEP to the current directory. Credentials never enter Nix evaluation or the
store.
