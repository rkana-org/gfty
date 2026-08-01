# Base presets

Gridfinity Ultimate baseplates are configured as the `[base]` section of a bin,
so these Nix files are reusable base presets consumed by `../module.nix`.
`magnetic.nix` demonstrates magnets, connector cutouts, and the separate
connector pin; `plain.nix` demonstrates the low-profile non-magnetic base.

The complete TOML equivalent is shown in
`../bins/with-magnetic-base.toml`. A bin without any base is shown in
`../bins/bin-only.toml`.

A standalone base export is not advertised yet. The pinned model leaves an
unnamed helper body when the bin is disabled, and the current whole-Part-Studio
export correctly rejects that manifest. Live API testing has since shown that
configured part discovery plus translation `partIds` can export only the named
`Base` or `ConnectorPin` and omit the helper. Complete bin exports continue to
contain the correctly named products while that selection path is added to the
CLI.
