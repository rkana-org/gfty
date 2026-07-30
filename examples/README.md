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

Each individual label contains `label.svg`, `label.json`, and `label.toml`.
Each plate contains `plate.svg` and `plate.json`.
