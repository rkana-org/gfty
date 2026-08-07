# Gridfinity Ultimate designer

A static, client-side configurator for the Gridfinity Ultimate Onshape model. It
covers baseplates, bins, divider layouts, easy-grab scoops, swappable rims, and
swappable labels while producing the canonical JSON accepted by the model's
`Config` parameter.

## Use

1. Open the hosted [designer](https://rkana-org.github.io/gfty/).
2. Configure the model and copy its JSON, typed TOML files, or flake-parts module.
3. Choose **Open in Onshape** to use the pinned immutable model version.

The browser workflow uploads nothing. Generated TOML can be exported through the
authenticated `gfty` CLI.

## Develop

Run these commands from the repository root:

```sh
nix develop
designer-dev       # source with live reload on :8080
designer-preview   # production build on :8081
nix build .#designer
```

`logic.js` contains the framework-free configuration and serialization logic.
The JSX files use global browser objects rather than ES modules so development
can use Babel directly and the Nix build can transpile each file independently.

Designer deployment uses `.github/workflows/designer-pages.yml` and tags such as
`designer-v42`. The tag version must match `ONSHAPE_VERSION` in `JsonPanel.jsx`.
