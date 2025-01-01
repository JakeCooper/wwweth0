let
  pkgs = import <nixpkgs> {};
in
pkgs.mkShell {
  buildInputs = with pkgs; [
    # Rust toolchain
    rustc
    cargo
    clippy      # Added clippy
    rust-analyzer
    rustfmt

    # WASM tools
    wasm-pack
    wasm-bindgen-cli

    # Build tools
    pkg-config
    libiconv    # Add libiconv here

    # Development tools
    nodePackages.npm
    nodejs

    # Shell and git
    zsh
    git

    # Build dependencies
    openssl
  ];

  # Environment variables and shell configuration
  shellHook = ''
    # Set up Rust environment
    export RUST_SRC_PATH="${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}"
    export RUST_BACKTRACE=1

    # Ensure clippy is available to cargo
    export CARGO_HOME=$PWD/.cargo

    # Return to zsh
    if [ -f "$HOME/.zshrc" ]; then
      exec zsh -i
    else
      exec zsh
    fi
  '';
}
