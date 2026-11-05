{
  description = "uad-ng: GUI, CLI and devshell";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

  outputs =
    { nixpkgs, self, ... }:
    let
      inherit (nixpkgs) lib;

      forSystemList = systems: f: lib.genAttrs systems (system: f (import nixpkgs { inherit system; }));
      forSystems = forSystemList lib.systems.flakeExposed;
      # The GUI only targets desktop Linux and macOS.
      forPackageSystems = forSystemList (
        lib.intersectLists lib.systems.flakeExposed (lib.platforms.linux ++ lib.platforms.darwin)
      );

      runtimeLibsFor =
        pkgs:
        with pkgs;
        [
          fontconfig
          freetype
        ]
        ++ lib.optionals stdenv.hostPlatform.isLinux [
          libglvnd
          libx11
          libxcursor
          libxi
          libxkbcommon
          libxrandr
          # wgpu loads Vulkan dynamically and uses the system's GPU drivers.
          vulkan-loader
          wayland
        ];
    in
    {
      formatter = forSystems (pkgs: pkgs.nixfmt-tree);

      packages = forPackageSystems (
        pkgs:
        let
          runtimeLibs = runtimeLibsFor pkgs;
          uad-ng = pkgs.rustPlatform.buildRustPackage {
            pname = "uad-ng";
            version = (lib.importTOML ./Cargo.toml).workspace.package.version;

            # Include the Rust workspace and embedded resources.
            src = lib.fileset.toSource {
              root = ./.;
              fileset = lib.fileset.unions [
                ./Cargo.toml
                ./Cargo.lock
                ./clippy.toml
                ./crates
                ./resources
              ];
            };

            cargoLock.lockFile = ./Cargo.lock;

            cargoBuildFlags = [ "--package=uad-gui" ];
            cargoTestFlags = [
              "--package=uad-gui"
              "--package=uad-core"
            ];

            # Application updates are managed by Nix.
            buildNoDefaultFeatures = true;
            buildFeatures = [
              "wgpu"
              "img"
            ];

            nativeBuildInputs = with pkgs; [
              makeWrapper
              pkg-config
            ];

            buildInputs = with pkgs; [
              fontconfig
              freetype
            ];

            nativeCheckInputs = with pkgs; [ writableTmpDirAsHomeHook ];

            # Make adb and graphics libraries available at runtime.
            postInstall = ''
              wrapProgram $out/bin/uad-ng \
                --suffix PATH : ${lib.makeBinPath [ pkgs.android-tools ]} \
                ${lib.optionalString pkgs.stdenv.hostPlatform.isLinux "--prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath runtimeLibs}"}
            '';

            meta = {
              description = "Cross-platform GUI to debloat Android devices without root";
              homepage = "https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation";
              license = lib.licenses.gpl3Only;
              mainProgram = "uad-ng";
              platforms = lib.platforms.linux ++ lib.platforms.darwin;
            };
          };

          uad-cli = pkgs.rustPlatform.buildRustPackage {
            pname = "uad-cli";
            inherit (uad-ng) version src cargoDeps;

            cargoBuildFlags = [ "--package=uad-cli" ];
            cargoTestFlags = [
              "--package=uad-cli"
              "--package=uad-core"
            ];

            nativeBuildInputs = with pkgs; [
              installShellFiles
              makeWrapper
            ];

            nativeCheckInputs = with pkgs; [ writableTmpDirAsHomeHook ];

            postInstall = ''
              wrapProgram $out/bin/uad --suffix PATH : ${lib.makeBinPath [ pkgs.android-tools ]}
            ''
            + lib.optionalString (pkgs.stdenv.buildPlatform.canExecute pkgs.stdenv.hostPlatform) ''
              for shell in bash fish zsh; do
                $out/bin/uad completions "$shell" > "uad.$shell"
                installShellCompletion --cmd uad --"$shell" "uad.$shell"
              done
            '';

            nativeInstallCheckInputs = with pkgs; [ versionCheckHook ];
            doInstallCheck = true;

            meta = {
              inherit (uad-ng.meta) homepage license platforms;
              description = "Command-line interface for Universal Android Debloater";
              mainProgram = "uad";
            };
          };
        in
        {
          inherit uad-ng uad-cli;
          default = uad-ng;
        }
      );

      apps = forPackageSystems (
        pkgs:
        let
          packages = self.packages.${pkgs.stdenv.hostPlatform.system};
          mkApp = package: {
            type = "app";
            program = lib.getExe package;
            meta.description = package.meta.description;
          };
        in
        {
          default = mkApp packages.uad-ng;
          uad-ng = mkApp packages.uad-ng;
          uad-cli = mkApp packages.uad-cli;
        }
      );

      devShells = forSystems (
        pkgs:
        let
          inherit (pkgs) stdenv;
          runtimeLibs = runtimeLibsFor pkgs;
        in
        {
          default = pkgs.mkShell (
            {
              packages = with pkgs; [
                rustc
                cargo
                rustfmt
                rust-analyzer
                clippy
                android-tools
                clang
                pkg-config
              ];

              RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}/lib/rustlib/src/rust";
              LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
            }
            // lib.optionalAttrs stdenv.hostPlatform.isLinux {
              LD_LIBRARY_PATH = lib.makeLibraryPath runtimeLibs;
            }
            // lib.optionalAttrs stdenv.hostPlatform.isDarwin {
              DYLD_FALLBACK_LIBRARY_PATH = lib.makeLibraryPath runtimeLibs;
            }
          );
        }
      );
    };
}
