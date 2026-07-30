{
  linkFarm,
  name,
  entries,
  extra ? { },
}:
let
  combined = linkFarm name (
    builtins.map (entryName: {
      name = entryName;
      path = entries.${entryName};
    }) (builtins.attrNames entries)
  );
in
combined.overrideAttrs (old: {
  passthru = (old.passthru or { }) // entries // extra;
})
