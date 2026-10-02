{
  description = "Pinned build tools for rake-lang.org";

  # Match the compiler's development toolchain without sharing build outputs.
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/418468ac9527e799809c900eda37cbff999199b6";

  outputs = { nixpkgs, ... }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
    in {
      devShells = nixpkgs.lib.genAttrs systems (system:
        let pkgs = nixpkgs.legacyPackages.${system}; in {
          default = pkgs.mkShell {
            packages = with pkgs; [
              cargo rustc rustfmt clippy rust-analyzer stdenv.cc
              nodejs tree-sitter emscripten
            ];
            shellHook = ''
              if [ -n "''${KAI_CARGO_TARGET_ROOT:-}" ]; then
                export CARGO_TARGET_DIR="$KAI_CARGO_TARGET_ROOT/rake-lang-site"
              else
                export CARGO_TARGET_DIR="''${XDG_CACHE_HOME:-$HOME/.cache}/cargo-targets/rake-lang-site"
                echo "rake-lang.org: host-managed Cargo storage unavailable; using $CARGO_TARGET_DIR" >&2
              fi
            '';
          };
        });
    };
}
