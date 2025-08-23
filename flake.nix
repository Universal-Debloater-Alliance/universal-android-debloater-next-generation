{
  description = "devshell for uad-ng";

  inputs.nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";

  outputs =
    { self, nixpkgs, ... }:
    let
      forSystems =
        f:
        nixpkgs.lib.genAttrs nixpkgs.lib.systems.flakeExposed (
          system: f { pkgs = import nixpkgs { inherit system; }; }
        );
    in
    {
      formatter = forSystems ({ pkgs }: pkgs.nixfmt-tree);

      devShells = forSystems (
        { pkgs }:
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              rustc
              cargo
              clang

              rustfmt
              rust-analyzer
              clippy

              pkg-config
              android-tools
            ];

            RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}/lib/rustlib/src/rust";
            LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";

            LD_LIBRARY_PATH = "${pkgs.lib.makeLibraryPath (
              with pkgs;
              [
                fontconfig
                freetype
                libglvnd
                xorg.libX11
                xorg.libXcursor
                xorg.libXi
                xorg.libXrandr
                libxkbcommon
                wayland
              ]
            )}";
          };
        }
      );
    };
}
