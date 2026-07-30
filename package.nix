{
  lib,
  rustPlatform,
  makeWrapper,
  callPackage,
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
  pname = "gfty-label";
  version = "0.1.0";
  src = lib.cleanSource ./.;

  cargoLock.lockFile = ./Cargo.lock;

  nativeBuildInputs = [ makeWrapper ];
  postFixup = ''
    wrapProgram $out/bin/gfty-label \
      --prefix GFTY_LABEL_FONT_DIRS : ${lib.escapeShellArg (lib.concatStringsSep ":" (map toString fonts))}
  '';

  passthru = {
    mkLabel =
      args:
      callPackage ./nix/mk-label.nix {
        gftyLabel = finalAttrs.finalPackage;
        inherit args;
      };
    mkPlate =
      args:
      callPackage ./nix/mk-plate.nix {
        gftyLabel = finalAttrs.finalPackage;
        inherit args;
      };
    mkOutputSet = args: callPackage ./nix/mk-output-set.nix args;
  };

  meta = {
    description = "File-based Gridfinity label composer and Onshape exporter";
    license = lib.licenses.mit;
    mainProgram = "gfty-label";
  };
})
