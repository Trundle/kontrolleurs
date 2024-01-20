{
  description = "Readline-like ctrl-r for fish";

  inputs.crane = {
    url = "github:ipetkov/crane";
    inputs.nixpkgs.follows = "nixpkgs";
  };
  inputs.pre-commit-hooks = {
    url = "github:cachix/pre-commit-hooks.nix";
    inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, crane, pre-commit-hooks }:
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
          craneLib = crane.lib.${system};
          pkgs = nixpkgs.legacyPackages.${system};
          lib = pkgs.lib;
          version = "0.1.0";
        in
        {
          packages.kontrolleurs = craneLib.buildPackage {
            inherit version;
            pname = "kontrolleurs";

            src = with lib; cleanSourceWith {
              src = craneLib.cleanCargoSource self;
            };

            buildInputs = lib.optionals pkgs.stdenv.isDarwin [
              pkgs.libiconv
            ];
          };

          packages.kontrolleurs-fish = pkgs.fishPlugins.buildFishPlugin {
            inherit version;
            pname = "kontrolleurs-fish";

            src = "${self}/fish";

            buildInputs = [ self.packages.${system}.kontrolleurs ];

            patchPhase = ''
              substituteInPlace functions/_kontrolleurs_ctrl_r.fish \
                --replace '| kontrolleurs |' '| ${self.packages.${system}.kontrolleurs}/bin/kontrolleurs |'
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

          checks.actionlint = pre-commit-hooks.lib.${pkgs.system}.run {
            src = lib.sourceFilesBySuffices self [ ".yml" ".yaml" ];
            hooks = {
              actionlint.enable = true;
            };
          };

          checks.clippy = craneLib.cargoClippy {
            inherit (self.packages.${pkgs.system}.kontrolleurs) pname src buildInputs;
            cargoClippyExtraArgs = "--all-features --tests -- -D warnings -D clippy::pedantic";
            cargoArtifacts = null;
            doInstallCargoArtifacts = false;
          };

          checks.nix = pre-commit-hooks.lib.${pkgs.system}.run {
            src = lib.sourceFilesBySuffices self [ ".nix" ];
            hooks = {
              nixpkgs-fmt.enable = true;
              deadnix.enable = true;
            };
          };

          checks.rustfmt = craneLib.cargoFmt {
            inherit (self.packages.${pkgs.system}.kontrolleurs) pname src;
          };
        }) // {
      overlays.default = final: _prev: {
        kontrolleurs = self.packages.${final.system}.kontrolleurs;
        kontrolleurs-fish = self.packages.${final.system}.kontrolleurs-fish;
      };
    };
}
