{
  lib,
  rustPlatform,
  makeWrapper,
  chafa,
  dejavu_fonts,
  liberation_ttf,
  fonts ? [
    dejavu_fonts
    liberation_ttf
  ],
}:
rustPlatform.buildRustPackage {
  pname = "gfty-label";
  version = "0.1.0";
  src = lib.cleanSource ./.;

  cargoLock.lockFile = ./Cargo.lock;

  nativeBuildInputs = [ makeWrapper ];
  postFixup = ''
    wrapProgram $out/bin/gfty-label \
      --set GFTY_LABEL_FONT_DIRS ${lib.escapeShellArg (lib.concatStringsSep ":" (map toString fonts))} \
      --prefix PATH : ${lib.makeBinPath [ chafa ]}
  '';

  meta = {
    description = "File-based Gridfinity label composer and Onshape exporter";
    license = lib.licenses.mit;
    mainProgram = "gfty-label";
  };
}
