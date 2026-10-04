{ pkgs, lib, config, ... }:

{
  dotenv.enable = true;

  dagger.enable = true;
  env.DAGGER_X_RELEASE = "v1.0.0-beta.14";

  # Required by arborium
  env.CC_wasm32_unknown_unknown = "${pkgs.llvmPackages.clang-unwrapped}/bin/clang";

  packages =
    with pkgs;
    [
      lld
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
      # The toolchain `dagger.toml` pins for the gates, so local lints and
      # tests match CI. A newer Clippy adds lints the pinned gates do not run.
      version = "1.97.1";
      # targets = [ "wasm32-unknown-unknown" ];
    };
    javascript = {
      enable = true;
      npm.enable = true;
    };
  };
}
