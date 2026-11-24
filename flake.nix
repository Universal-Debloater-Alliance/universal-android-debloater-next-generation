{
  description = "uad-ng: GUI, CLI and devshell";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  inputs.rust-overlay = {
    url = "github:oxalica/rust-overlay";
    inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs =
    {
      nixpkgs,
      rust-overlay,
      self,
      ...
    }:
    let
      inherit (nixpkgs) lib;

      forSystemList =
        systems: f:
        lib.genAttrs systems (
          system:
          f (
            import nixpkgs {
              inherit system;
              overlays = [ rust-overlay.overlays.default ];
            }
          )
        );
      forSystems = forSystemList lib.systems.flakeExposed;
      # Nix builds target Linux and macOS.
      forPackageSystems = forSystemList (
        lib.intersectLists lib.systems.flakeExposed (lib.platforms.linux ++ lib.platforms.darwin)
      );

      rustToolchainFor =
        pkgs:
        pkgs.rust-bin.stable.latest.minimal.override {
          extensions = [
            "clippy"
            "rustfmt"
            "rust-src"
            "rust-analyzer"
            "llvm-tools-preview"
          ];
        };

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
          appId = "io.github.Universal_Debloater_Alliance.uad_ng";
          rustToolchain = rustToolchainFor pkgs;
          rustPlatform = pkgs.makeRustPlatform {
            cargo = rustToolchain;
            rustc = rustToolchain;
          };
          uad-ng = rustPlatform.buildRustPackage {
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
            ''
            + lib.optionalString pkgs.stdenv.hostPlatform.isLinux ''
              install -Dm644 resources/linux/${appId}.desktop "$out/share/applications/${appId}.desktop"
              install -Dm644 resources/linux/${appId}.metainfo.xml "$out/share/metainfo/${appId}.metainfo.xml"
              install -Dm644 resources/assets/logo-dark.png "$out/share/pixmaps/${appId}.png"
            '';

            meta = {
              description = "Cross-platform GUI to debloat Android devices without root";
              homepage = "https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation";
              license = lib.licenses.gpl3Only;
              mainProgram = "uad-ng";
              platforms = lib.platforms.linux ++ lib.platforms.darwin;
            };
          };

          uad-cli = rustPlatform.buildRustPackage {
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

      devShells = forPackageSystems (
        pkgs:
        let
          inherit (pkgs) stdenv;
          runtimeLibs = runtimeLibsFor pkgs;
          rustToolchain = rustToolchainFor pkgs;
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              rustToolchain
              grcov
              android-tools
              clang
              pkg-config
            ];

            env = {
              RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
              LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
            }
            // lib.optionalAttrs stdenv.hostPlatform.isLinux {
              LD_LIBRARY_PATH = lib.makeLibraryPath runtimeLibs;
            }
            // lib.optionalAttrs stdenv.hostPlatform.isDarwin {
              DYLD_FALLBACK_LIBRARY_PATH = lib.makeLibraryPath runtimeLibs;
            };
          };
        }
      );
    };
}
