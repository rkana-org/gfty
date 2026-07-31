{
  lib,
  stdenvNoCC,
  esbuild,
  fetchurl,
}:
let
  # Production UMD builds, vendored so the published site has no runtime
  # CDN dependency. Keep the version in sync with index.html (dev path).
  reactVersion = "18.3.1";
  reactJs = fetchurl {
    url = "https://unpkg.com/react@${reactVersion}/umd/react.production.min.js";
    hash = "sha256-2Unxw2h67a3O2shSYYZfKbF80nOZfn9rK/xTsvnUxN0=";
  };
  reactDomJs = fetchurl {
    url = "https://unpkg.com/react-dom@${reactVersion}/umd/react-dom.production.min.js";
    hash = "sha256-NfT5dPSyvNRNpzljNH+JUuNB+DkJ5EmCJ9Tia5j2bw0=";
  };

  # The site is versioned after the pinned Onshape model version.
  onshapeVersion =
    let
      m = builtins.match ".*const ONSHAPE_VERSION = \"([^\"]+)\".*" (
        builtins.readFile ../designer/JsonPanel.jsx
      );
    in
    lib.throwIf (m == null) "Could not parse ONSHAPE_VERSION from designer/JsonPanel.jsx" (
      builtins.head m
    );
in
stdenvNoCC.mkDerivation {
  pname = "gridfinity-ultimate-designer";
  version = onshapeVersion;

  src = ../designer;

  nativeBuildInputs = [ esbuild ];

  buildPhase = ''
    runHook preBuild

    # Transpile each JSX module in place — the app uses the global-script
    # pattern (window.X, no ES modules), so no bundling is needed.
    for f in *.jsx; do
      esbuild "$f" --outfile="''${f%.jsx}.js"
    done

    mkdir vendor
    cp ${reactJs} vendor/react.production.min.js
    cp ${reactDomJs} vendor/react-dom.production.min.js

    # Rewrite index.html for production: drop in-browser Babel, point at
    # the transpiled modules and the vendored production React.
    sed -i \
      -e 's|<script src="https://unpkg.com/react@[^/]*/umd/react.development.js"[^>]*></script>|<script src="vendor/react.production.min.js"></script>|' \
      -e 's|<script src="https://unpkg.com/react-dom@[^/]*/umd/react-dom.development.js"[^>]*></script>|<script src="vendor/react-dom.production.min.js"></script>|' \
      -e '\|@babel/standalone|d' \
      -e 's|type="text/babel" src="\([^"]*\)\.jsx"|src="\1.js"|' \
      index.html

    # Fail loudly if any dev-only reference survived the rewrite.
    if grep -nE 'text/babel|\.jsx|\.development\.js' index.html; then
      echo "error: index.html still references development-only assets" >&2
      exit 1
    fi

    runHook postBuild
  '';

  installPhase = ''
    runHook preInstall

    mkdir -p $out
    cp index.html styles.css ./*.js $out/
    cp -r vendor $out/

    runHook postInstall
  '';

  meta = {
    description = "Static client-side configurator for the Gridfinity Ultimate Onshape model";
    homepage = "https://github.com/rkana-org/gridfinity-ultimate";
    license = lib.licenses.mit;
  };
}
