{ pkgs, lib, config, multiverse, ... }:

let
  # The PDF structure checks require exactly this release.
  qpdf = multiverse.qpdf."12.3.2";

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

  # The harness wrappers read qpdf only from target/pdf-harness/qpdf, so link it
  # there, unless tools/pdf-harness/install.sh unpacked the release there.
  enterShell = ''
    harness=${lib.escapeShellArg "${config.devenv.root}/target/pdf-harness"}
    if [[ -L "$harness/qpdf" || ! -e "$harness/qpdf" ]]; then
      mkdir -p "$harness"
      ln -sfn ${qpdf} "$harness/qpdf"
    fi
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

  scripts.pdfium-install = {
    description = "Install the exact-pinned native PDFium library and Rust harness worker (Linux x86_64)";
    exec = ''
      exec bash ${lib.escapeShellArg "${config.devenv.root}/tools/pdf-harness/install.sh"} --pdfium-only
    '';
  };

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
