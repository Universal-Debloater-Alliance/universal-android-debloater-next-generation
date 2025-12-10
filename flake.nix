{
  description = "devshell for uad-ng";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

  outputs =
    { nixpkgs, ... }:
    let
      forSystems =
        f:
        nixpkgs.lib.genAttrs nixpkgs.lib.systems.flakeExposed (
          system: f (import nixpkgs { inherit system; })
        );
    in
    {
      formatter = forSystems (pkgs: pkgs.nixfmt-tree);

      devShells = forSystems (
        pkgs:
        let
          inherit (pkgs) lib stdenv;
          runtimeLibs = [
            pkgs.fontconfig
            pkgs.freetype
          ]
          ++ lib.optionals stdenv.isLinux (
            with pkgs;
            [
              libglvnd
              xorg.libX11
              xorg.libXcursor
              xorg.libXi
              xorg.libXrandr
              libxkbcommon
              wayland
            ]
          );
        in
        {
          default = pkgs.mkShell (
            {
              nativeBuildInputs = with pkgs; [
                clang
                pkg-config
              ];
              buildInputs =
                with pkgs;
                [
                  rustc
                  cargo
                  rustfmt
                  rust-analyzer
                  clippy
                  android-tools
                ]
                ++ runtimeLibs;

              RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}/lib/rustlib/src/rust";
              LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
            }
            // lib.optionalAttrs stdenv.isLinux { LD_LIBRARY_PATH = lib.makeLibraryPath runtimeLibs; }
            // lib.optionalAttrs stdenv.isDarwin {
              DYLD_FALLBACK_LIBRARY_PATH = lib.makeLibraryPath runtimeLibs;
            }
          );
        }
      );
    };
}
