{
  description = "Readline-like ctrl-r for fish";

  inputs.git-hooks = {
    url = "github:cachix/git-hooks.nix";
    inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, git-hooks }:
    let
      defaultSystems = [
        "aarch64-linux"
        "aarch64-darwin"
        "x86_64-darwin"
        "x86_64-linux"
      ];
      eachDefaultSystem = f:
        let
          op = outputAttrs: system:
            let systemAttrs = f system;
            in builtins.foldl'
              (attrs: name: attrs // {
                ${name} = (attrs.${name} or { }) // { ${system} = systemAttrs.${name}; };
              })
              outputAttrs
              (builtins.attrNames systemAttrs);
        in
        builtins.foldl' op { } defaultSystems;
    in
    eachDefaultSystem
      (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          lib = pkgs.lib;
          version = "0.1.0";
        in
        {
          packages.kontrolleurs = pkgs.rustPlatform.buildRustPackage
            {
              inherit version;
              pname = "kontrolleurs";

              src = with lib; cleanSourceWith {
                src = lib.fileset.toSource {
                  root = ./.;
                  fileset = lib.fileset.unions
                    [
                      ./Cargo.toml
                      ./Cargo.lock
                      ./src
                    ];
                };
              };
              cargoLock = {
                lockFile = ./Cargo.lock;
                outputHashes = {
                  "filedescriptor-0.8.3" = "sha256-8j7044lN0w/uVQOvqq/GlDGATmI3zAk/GTndJEyb3Ws=";
                };
              };

              buildInputs = lib.optionals pkgs.stdenv.isDarwin [
                pkgs.libiconv
              ];

              nativeBuildInputs = [ pkgs.clippy ];

              preInstallPhases = [ "clippy" ];
              clippy = ''
                cargo clippy --release --offline --all-features --tests -- -D warnings -D clippy::pedantic
              '';
            };

          packages.kontrolleurs-fish = pkgs.fishPlugins.buildFishPlugin {
            inherit version;
            pname = "kontrolleurs-fish";

            src = "${self}/fish";

            buildInputs = [ self.packages.${system}.kontrolleurs ];

            patchPhase = ''
              substituteInPlace functions/_kontrolleurs_ctrl_r.fish \
                --replace-fail '| kontrolleurs |' '| ${self.packages.${system}.kontrolleurs}/bin/kontrolleurs |'
            '';
          };

          packages.default = self.packages.${system}.kontrolleurs-fish;

          devShells.default = pkgs.mkShell {
            name = "kontrolleurs-dev-shell";

            buildInputs = with pkgs; [
              rustc
              rustfmt
              cargo
              clippy
            ];
          };

          checks.actionlint = git-hooks.lib.${pkgs.system}.run {
            src = lib.sourceFilesBySuffices self [ ".yml" ".yaml" ];
            hooks = {
              actionlint.enable = true;
            };
          };

          checks.nix = git-hooks.lib.${pkgs.system}.run {
            src = lib.sourceFilesBySuffices self [ ".nix" ];
            hooks = {
              nixpkgs-fmt.enable = true;
              deadnix.enable = true;
            };
          };

          checks.rustfmt = git-hooks.lib.${pkgs.system}.run {
            inherit (self.packages.${pkgs.system}.kontrolleurs) src;
            hooks.rustfmt.enable = true;
          };
        }) // {
      overlays.default = final: _prev: {
        kontrolleurs = self.packages.${final.system}.kontrolleurs;
        kontrolleurs-fish = self.packages.${final.system}.kontrolleurs-fish;
      };
    };
}
