
{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";

  inputs.noctalia-greeter = {
    url = "github:noctalia-dev/noctalia-greeter";
    inputs.nixpkgs.follows = "nixpkgs";
  };

  inputs.evangelion = {
    url = "github:Aleph1-9012/Evangelion";
    flake = false;
  };

  inputs.nixpkgs-unstable.url = "github:NixOS/nixpkgs/nixos-unstable";
  
  inputs.umbriel.url = "github:noctalia-dev/umbriel/cachix";

  inputs.noctalia.url = "github:noctalia-dev/noctalia/cachix";

  inputs.zen-browser.url = "github:0xc000022070/zen-browser-flake";

  inputs.claude-code.url = "github:sadjow/claude-code-nix";

  inputs.claude-desktop.url = "github:aaddrick/claude-desktop-debian";

  outputs = { self, nixpkgs, nixpkgs-unstable, umbriel, zen-browser, noctalia, evangelion, noctalia-greeter, claude-code, claude-desktop, ... }: {
    nixosConfigurations.nixos = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        ./configuration.nix
        (import ./penpen-boot.nix evangelion)
        noctalia-greeter.nixosModules.default
	umbriel.nixosModules.default
	noctalia.nixosModules.default
	({ ... }: {
	 programs.umbriel.enable = true;
         programs.noctalia.enable = true;
  programs.noctalia.package = import ./noctalia-bars.nix noctalia.packages."x86_64-linux".default;
	 
	 environment.systemPackages = [
	   zen-browser.packages."x86_64-linux".twilight
	   claude-code.packages."x86_64-linux".default
	   claude-desktop.packages."x86_64-linux".claude-desktop-fhs
	   (nixpkgs-unstable.legacyPackages."x86_64-linux".netwatch.overrideAttrs (o: { checkFlags = (o.checkFlags or [ ]) ++ [ "--skip=sandbox::linux::tests::parser_layer_leaves_fast_open_and_autobind_open" ]; }))
	   (nixpkgs-unstable.legacyPackages."x86_64-linux".ghostty.overrideAttrs (o: { patches = (o.patches or [ ]) ++ [ ./ghostty-drag-out.patch ]; }))
	 ];
	 nix.settings.extra-substituters = [
	   "https://umbriel.cachix.org"
	   "https://claude-code.cachix.org"
	 ];

	 nix.settings.extra-trusted-public-keys = [
	   "umbriel.cachix.org-1:JfNq/2yg2S6D6z4Z2dVSZrZlDPQTKtexB6GAVLD98nw="
	   "claude-code.cachix.org-1:YeXf2aNu7UTX8Vwrze0za1WEDS+4DuI2kVeWEE4fsRk="
	 ];
       })
     ];
   };
 };
}
