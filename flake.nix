{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };
  outputs = { self, nixpkgs, rust-overlay, ... }:
    let
      namespace = "dynqueue";
      overlays = [
        rust-overlay.overlays.default
      ];
      forEachSystem = systems: f: nixpkgs.lib.genAttrs systems f;
      forAllSystems = function: nixpkgs.lib.genAttrs nixpkgs.lib.systems.flakeExposed (
        system: function (import nixpkgs { inherit system overlays; })
      );
    in
    {
      packages = forAllSystems (pkgs: (self.overlays.default pkgs pkgs).${namespace});
      cross = forAllSystems (pkgs: (forEachSystem (nixpkgs.lib.filter (sys: sys != pkgs.system) nixpkgs.lib.systems.flakeExposed) (crossSystem:
        let
          crossPkgs = import nixpkgs { localSystem = pkgs.system; inherit crossSystem; };
        in
        (self.overlays.default crossPkgs crossPkgs).${namespace}
      )));
      devShells = forAllSystems (pkgs: (self.overlays.default pkgs pkgs).devShells);
      formatter = forAllSystems (pkgs: pkgs.nixpkgs-fmt);
      overlays.default = final: prev:
        let pkgs = final; in {
          devShells.default = pkgs.callPackage ./shell.nix { };
          ${namespace} = {
            dynqueue = pkgs.callPackage ./default.nix { };
            default = pkgs.callPackage ./default.nix { };
          };
        };
    };
}
