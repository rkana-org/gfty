{
  lib,
  rustPlatform,
  makeWrapper,
  callPackage,
  writeShellScript,
  dejavu_fonts,
  liberation_ttf,
  jetbrains-mono,
  fonts ? [
    dejavu_fonts
    liberation_ttf
    jetbrains-mono
  ],
}:
rustPlatform.buildRustPackage (finalAttrs: {
  pname = "gfty";
  version = "0.1.0";
  src = lib.cleanSource ./.;

  cargoLock.lockFile = ./Cargo.lock;

  nativeBuildInputs = [ makeWrapper ];
  postFixup = ''
    wrapProgram $out/bin/gfty \
      --prefix GFTY_FONT_DIRS : ${lib.escapeShellArg (lib.concatStringsSep ":" (map toString fonts))}
  '';

  passthru = {
    mkLabel =
      args:
      callPackage ./nix/mk-label.nix {
        gfty = finalAttrs.finalPackage;
        inherit args;
      };
    mkPlate =
      args:
      callPackage ./nix/mk-plate.nix {
        gfty = finalAttrs.finalPackage;
        inherit args;
      };
    mkBin = args: callPackage ./nix/mk-bin.nix { inherit args; };
    mkBase = args: callPackage ./nix/mk-base.nix { inherit args; };
    mkRim = args: callPackage ./nix/mk-rim.nix { inherit args; };
    mkSwappableLabel = args: callPackage ./nix/mk-swappable-label.nix { inherit args; };
    mkBinSet = args: callPackage ./nix/mk-bin-set.nix { inherit args; };
    mkOutputSet = args: callPackage ./nix/mk-output-set.nix args;
    writeExportScript = name: text: writeShellScript name text;
  };

  meta = {
    description = "Reproducible Gridfinity bin and label authoring with Onshape export";
    license = lib.licenses.mit;
    mainProgram = "gfty";
  };
})
