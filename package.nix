{
  lib,
  rustPlatform,
}:
rustPlatform.buildRustPackage {
  pname = "gfty-label";
  version = "0.1.0";
  src = lib.cleanSource ./.;

  cargoLock.lockFile = ./Cargo.lock;

  meta = {
    description = "File-based Gridfinity label composer and Onshape exporter";
    license = lib.licenses.mit;
    mainProgram = "gfty-label";
  };
}
