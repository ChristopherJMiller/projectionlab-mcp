# This file provides compatibility with the legacy nix tooling
# It exposes the devShell from flake.nix for use with VSCode Nix env selector
let
  flake = builtins.getFlake (toString ./.);
  system = builtins.currentSystem;
in
  flake.devShells.${system}.default
