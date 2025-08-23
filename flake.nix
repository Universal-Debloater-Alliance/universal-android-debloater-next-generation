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
              pkg-config
              android-tools
            ];

            LD_LIBRARY_PATH = "${nixpkgs.lib.makeLibraryPath [
              pkgs.fontconfig
              pkgs.freetype
              pkgs.libglvnd
              pkgs.xorg.libX11
              pkgs.xorg.libXcursor
              pkgs.xorg.libXi
              pkgs.xorg.libXrandr
              pkgs.libxkbcommon
              pkgs.wayland
            ]}";
            LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
          };
        }
      );
    };
}
