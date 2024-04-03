{
  description = "devshell for uad-ng";

  inputs.flake-utils.url = "github:numtide/flake-utils";

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem
      (system:
        let pkgs = nixpkgs.legacyPackages.${system}; in
        {
          devShells.default = pkgs.mkShell rec {
            packages = with pkgs;
            [
              rustc
              cargo
              makeWrapper
              clang
              pkg-config
              mold
              expat
              fontconfig
              freetype
              android-tools
              libxkbcommon
              libGL
              wayland
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
              pkgs.libGL
              pkgs.wayland
            ]}";
            LIBCLANG_PATH="${pkgs.llvmPackages.libclang.lib}/lib";
          };
        }
      );
}
