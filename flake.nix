{
  description = "projectionlab-mcp - A Rust project";

  inputs = {
    nixpkgs.url = "nixpkgs";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain
            pkg-config
            openssl
            # Browser automation dependencies
            firefox
            geckodriver
            # Code generation tools
            quicktype
          ];
        };

        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "projectionlab-mcp";
          version = "0.1.0";
          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          nativeBuildInputs = with pkgs; [
            pkg-config
            makeWrapper
          ];

          buildInputs = with pkgs; [
            openssl
          ];

          postInstall = ''
            for bin in $out/bin/*; do
              wrapProgram "$bin" \
                --prefix PATH : ${pkgs.lib.makeBinPath [ pkgs.firefox pkgs.geckodriver ]}
            done
          '';
        };

        # App configuration for 'nix run'
        apps.default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/projectionlab-mcp";
        };
      }
    );
}
