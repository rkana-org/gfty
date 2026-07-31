# Gridfinity Ultimate

This is the Onshape model support and browser designer bundled with the `gfty`
repository. Its original Git history is preserved through the repository's
history merge.

A fully parametric [Gridfinity](https://gridfinity.xyz) model for
[Onshape](https://www.onshape.com): base-plates with magnets and connectors, bins
with custom divider layouts, easy-grab scoops, swappable color-coded rims and
swappable 3D-printed labels — all generated from a single JSON configuration.

## Usage

1. **Open the [Designer](https://rkana-org.github.io/gfty/)** —
   configure your bin visually and watch the JSON update live.
2. Click **Open in Onshape** (or copy the JSON into the model's `Config`
   parameter yourself) to generate the parts in the
   **[Onshape CAD model](https://cad.onshape.com/documents/044aa38d921c6673acd89aef/v/793cbd4a9bdd57cb44baa08a/e/47f09ccd9b344504691f98d4)**.
3. Export the generated parts and print.

Build the static designer from the repository root with `nix build .#designer`,
or run `designer-dev` in `nix develop` for live reload.

The designer runs entirely in your browser — nothing is uploaded anywhere. The
same `Config` text parameter also supports configured API exports: the JSON and
configuration encoding can be sent in POST bodies while targeting the immutable
model version, without modifying a workspace.

## License

[MIT](LICENSE)
