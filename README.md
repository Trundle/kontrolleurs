# kontrolleurs

A readline-like history search for fish. kontrolleurs is a port of
[kontrolleur](https://github.com/Trundle/kontrolleur) to Rust. Mostly only
exists because the `curtsies` package was broken in nixpkgs for macOS.

## Why? fish has a built-in history search

True! This wasn't the case though when
[kontrolleur](https://github.com/Trundle/kontrolleur) was first created. Plus,
I personally prefer a different kind of search, if only because of muscle
memory.


## Installation

### Nix

Add kontrolleurs as input to your flake:

```nix
inputs.kontrolleurs = {
  url = "github:Trundle/kontrolleurs";
  inputs.nixpkgs.follows = "nixpkgs";
};
```

To make `kontrolleurs` and `kontrolleurs-fish` available in `pkgs`, you also
need to add kontrolleur's `default` overlay to your `nixpkgs`.

#### NixOS

Untested, so please let me know if the following doesn't work: Either use
`wrapFish` or add `kontrolleurs-fish` to your system packages and enable
`programs.fish.enable`, `programs.fish.vendor.config.enable` and
`programs.fish.vendor.functions.enable`.

#### home-manager

home-manager uses a structure for fish plugins that differs from nixpkgs's
`buildFishPlugin`, so adding `kontrolleurs-fish` to `programs.fish.plugins`
won't work. You can work around with `programs.fish.interactiveShellInit`
though:

```nix
programs.fish = {
  enable = true;
  interactiveShellInit = ''
    source ${pkgs.kontrolleurs-fish}/share/fish/vendor_conf.d/*.fish
    set fish_function_path $fish_function_path[1] ${pkgs.kontrolleurs-fish}/share/fish/vendor_functions.d $fish_function_path[2..-1]
  '';
};
```

The above snippet assumes you added kontrolleur's default overlay.


## License

kontrolleurs is released under the Apache License, Version 2.0. See `LICENSE`
or http://www.apache.org/licenses/LICENSE-2.0.html for details.
