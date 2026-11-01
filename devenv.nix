{ pkgs, lib, config, multiverse, ... }:

let
  # The PDF structure checks require exactly this release.
  qpdf = multiverse.qpdf."12.3.2";

  # The upstream pdfium-binaries build 7947 the PDFium viewer profile names,
  # unpacked as published: a derivation's fixup would strip and rewrite the
  # library. Linux x86_64 is the exact library the profile was qualified with;
  # every host's build is held to the retained raster bytes by the harness.
  pdfium =
    let
      build =
        {
          x86_64-linux = {
            name = "linux-x64";
            hash = "sha256-HLAxMMJJHt+HsVdt+9hpJsodzNBEQEz1t0NiYEAAa+g=";
          };
          aarch64-linux = {
            name = "linux-arm64";
            hash = "sha256-OZIqRs/rk4xvnf27I+4VaRC1zTMmuSdms0+ob5kC6rA=";
          };
          x86_64-darwin = {
            name = "mac-x64";
            hash = "sha256-z2PURImFtyEsO96bQLoHtuDfhb4Z9NR3G4M8M/dwsMA=";
          };
          aarch64-darwin = {
            name = "mac-arm64";
            hash = "sha256-GIsFCk6WxW4Jo+Y/Xfyx7dH5xkdlVGXpiIscV0/vYZ4=";
          };
        }
        .${pkgs.stdenv.hostPlatform.system};
    in
    pkgs.fetchzip {
      name = "pdfium-binaries-7947-${build.name}";
      url = "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F7947/pdfium-${build.name}.tgz";
      inherit (build) hash;
      stripRoot = false;
    };

  # The WASM test runner must match the wasm-bindgen crate exactly.
  lock = builtins.fromTOML (builtins.readFile ./Cargo.lock);
  wasmBindgen = lib.findFirst (p: p.name == "wasm-bindgen") null lock.package;
  wasm-bindgen-cli = multiverse.wasm-bindgen-cli.${wasmBindgen.version};
in
{
  dotenv.enable = true;

  dagger.enable = true;
  env.DAGGER_X_RELEASE = "v1.0.0-beta.14";

  # Required by arborium
  env.CC_wasm32_unknown_unknown = "${pkgs.llvmPackages.clang-unwrapped}/bin/clang";

  # xtask reads qpdf from QPDF before target/pdf-harness/qpdf.
  env.QPDF = "${qpdf}/bin/qpdf";

  # Browser WASM runs in Firefox, the browser nixpkgs builds for every host
  # (ADR-0024). The renderer makes no browser calls, so the build is not pinned.
  env.CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER = "wasm-bindgen-test-runner";
  env.GECKODRIVER = "${pkgs.geckodriver}/bin/geckodriver";
  # The qualification corpus runs for minutes in one test.
  env.WASM_BINDGEN_TEST_TIMEOUT = "1800";
  # geckodriver otherwise starts whichever Firefox it finds first.
  env.WASM_BINDGEN_TEST_WEBDRIVER_JSON = toString (
    pkgs.writeText "webdriver.json" (
      builtins.toJSON {
        "moz:firefoxOptions".binary =
          if pkgs.stdenv.isDarwin then
            "${pkgs.firefox}/Applications/Firefox.app/Contents/MacOS/firefox"
          else
            "${pkgs.firefox}/bin/firefox";
      }
    )
  );

  # The PDF harness reads qpdf and PDFium only from target/pdf-harness, so link
  # them there, replacing a release an earlier installer unpacked in their place.
  enterShell = ''
    harness=${lib.escapeShellArg "${config.devenv.root}/target/pdf-harness"}
    mkdir -p "$harness"
    link_harness_tool() {
      [[ -L "$2" || ! -e "$2" ]] || rm -rf "$2"
      ln -sfn "$1" "$2"
    }
    link_harness_tool ${qpdf} "$harness/qpdf"
    link_harness_tool ${pdfium} "$harness/pdfium"
  '';

  packages =
    with pkgs;
    [
      just

      lld
      python3
      qpdf
      wasm-bindgen-cli
      firefox
      geckodriver
      cargo-audit
      cargo-deny
      cargo-dist
      cargo-nextest
      cargo-release
      cargo-watch
      wasm-pack
      curl
      unzip
      gnutar
      gzip
    ]
    ++ lib.optionals pkgs.stdenv.isLinux [
      pkgs.chromium
      pkgs.chromedriver
    ];

  languages = {
    rust = {
      enable = true;
      channel = "stable";
      # The toolchain the gates build with (ADR-0024); the evidence operations'
      # Rust image in `dagger.toml` matches it. A newer Clippy adds lints.
      # A default, so `just msrv` can run on another one with
      # `devenv --option languages.rust.version:string 1.92.0`.
      version = lib.mkDefault "1.97.1";
      targets = [ "wasm32-unknown-unknown" ];
    };
    javascript = {
      enable = true;
      npm.enable = true;
    };
  };
}
