# Base presets

Gridfinity Ultimate baseplates are configured as the `[base]` section of a bin,
so these Nix files are reusable base presets consumed by `../module.nix`.
`magnetic.nix` demonstrates magnets, connector cutouts, and the separate
connector pin; `plain.nix` demonstrates the low-profile non-magnetic base.

The complete TOML equivalent is shown in
`../bins/with-magnetic-base.toml`. A bin without any base is shown in
`../bins/bin-only.toml`.

A standalone base export is not advertised yet. The pinned model currently
leaves an unnamed helper body when the bin is disabled, and `gfty` rejects that
manifest rather than shipping an ambiguous `Part N`. Complete bin exports still
contain the correctly named `Base` and optional `ConnectorPin` products.
