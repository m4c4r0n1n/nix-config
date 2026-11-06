# Edit this configuration file to define what should be installed on
# your system. Help is available in the configuration.nix(5) man page, on
# https://search.nixos.org/options and in the NixOS manual (`nixos-help`).

{ config, lib, pkgs, ... }:

{
  imports =
    [ # Include the results of the hardware scan.
      ./hardware-configuration.nix
    ];

  # GRUB and the Pen-Pen theme live in penpen-boot.nix.
  boot.loader.timeout = null;
  boot.consoleLogLevel = 3;

  # networking.hostName = "nixos"; # Define your hostname.

  # Configure network connections interactively with nmcli or nmtui.
  networking.networkmanager.enable = true;

  # Set your time zone.
  time.timeZone = "America/New_York";

  # Configure network proxy if necessary
  # networking.proxy.default = "http://user:password@proxy:port/";
  # networking.proxy.noProxy = "127.0.0.1,localhost,internal.domain";

  # Select internationalisation properties.
  # i18n.defaultLocale = "en_US.UTF-8";
  # console = {
  #   font = "Lat2-Terminus16";
  #   keyMap = "us";
  #   useXkbConfig = true; # use xkb.options in tty.
  # };

  # Enable the X11 windowing system.
  # services.xserver.enable = true;


  

  # Configure keymap in X11
  # services.xserver.xkb.layout = "us";
  # services.xserver.xkb.options = "eurosign:e,caps:escape";

  # Enable CUPS to print documents.
  # services.printing.enable = true;

  # Enable sound.
  # services.pulseaudio.enable = true;
  # OR
  # services.pipewire = {
  #   enable = true;
  #   pulse.enable = true;
  # };

  # Enable touchpad support (enabled default in most desktopManager).
  # services.libinput.enable = true;

  # Define a user account. Don't forget to set a password with ‘passwd’.
  # users.users.alice = {
  #   isNormalUser = true;
  #   extraGroups = [ "wheel" ]; # Enable ‘sudo’ for the user.
  #   packages = with pkgs; [
  #     tree
  #   ];
  # };

  # programs.firefox.enable = true;

  # List packages installed in system profile.
  # You can use https://search.nixos.org/ to find more packages (and options).
  # environment.systemPackages = with pkgs; [
  #   vim # Do not forget to add an editor to edit configuration.nix! The Nano editor is also installed by default.
  #   wget
  # ];

  # Some programs need SUID wrappers, can be configured further or are
  # started in user sessions.
  # programs.mtr.enable = true;
  # programs.gnupg.agent = {
  #   enable = true;
  #   enableSSHSupport = true;
  # };

  # List services that you want to enable:

  # Enable the OpenSSH daemon.
  # services.openssh.enable = true;

  # Open ports in the firewall.
  # networking.firewall.allowedTCPPorts = [ ... ];
  # networking.firewall.allowedUDPPorts = [ ... ];
  # Or disable the firewall altogether.
  # networking.firewall.enable = false;

  # Copy the NixOS configuration file and link it from the resulting system
  # (/run/current-system/configuration.nix). This is useful in case you
  # accidentally delete configuration.nix.
  # system.copySystemConfiguration = true;

  # This option defines the first version of NixOS you have installed on this particular machine,
  # and is used to maintain compatibility with application data (e.g. databases) created on older NixOS versions.
  #
  # Most users should NEVER change this value after the initial install, for any reason,
  # even if you've upgraded your system to a new NixOS release.
  #
  # This value does NOT affect the Nixpkgs version your packages and OS are pulled from,
  # so changing it will NOT upgrade your system - see https://nixos.org/manual/nixos/stable/#sec-upgrading for how
  # to actually do that.
  #
  # This value being lower than the current NixOS release does NOT mean your system is
  # out of date, out of support, or vulnerable.
  #
  # Do NOT change this value unless you have manually inspected all the changes it would make to your configuration,
  # and migrated your data accordingly.
  #
  # For more information, see `man configuration.nix` or https://nixos.org/manual/nixos/stable/options#opt-system.stateVersion .
  nix.settings.experimental-features = [ "nix-command" "flakes" ];
  # Keeps /nix small enough for the root partition.
  nix.settings.auto-optimise-store = true;
  nix.settings.min-free = 5368709120;
  nix.settings.max-free = 10737418240;
  nix.gc = {
    automatic = true;
    dates = "weekly";
    options = "--delete-older-than 7d";
  };
  system.stateVersion = "26.05"; # Did you read the comment?

  users.groups.chrome.gid = 1000;

  users.users.chrome = {
    isNormalUser = true;
    uid = 1000;
    group = "chrome";
    extraGroups = [ "wheel" "networkmanager" "video" "render" ];
    createHome = false;
    shell = pkgs.zsh;
  };
  nixpkgs.config.allowUnfree = true;

  hardware.graphics.enable = true;

  services.xserver.videoDrivers = [ "nvidia" ];

  hardware.nvidia.open = true;
  hardware.nvidia.modesetting.enable = true;

  services.displayManager.noctalia-greeter = {
    enable = true;
    passwordless-sync-users = [ "chrome" ];
    settings.appearance.hide_logo = true;
    settings.cursor = {
      theme = "Bibata-Modern-Ice";
      size = 24;
      path = pkgs.bibata-cursors;
    };
  };

  security.sudo.extraConfig = "Defaults insults";
  security.rtkit.enable = true;

  services.pipewire = {
    enable = true;
    alsa.enable = true;
    alsa.support32Bit = true;
    pulse.enable = true;
  };

  environment.systemPackages = with pkgs; [
    (callPackage ./mousiki.nix { })
    (callPackage ./file-tui.nix { })
    linux-wallpaperengine
    vesktop
    koreader
    calibre
    obsidian
    qbittorrent
    obs-studio
    gimp
    libreoffice
    mpv
    spotify
    tmux
    btop
    gh
    fzf
    neovim
    git
    fastfetch
    wl-clipboard
    pavucontrol
    wofi
    grim
    slurp
    satty
    chromium
    _7zz
    drm_info
    ffuf
    thc-hydra
    inter
    jdk21
    localsend
    mgba
    nvtopPackages.nvidia
    exiftool
    python3Packages.pip
    pipx
    python3Packages.wfuzz
    adw-gtk3
  ];
  virtualisation.podman.enable = true;

  programs.starship.enable = true;
  programs.nix-ld.enable = true;
  services.upower.enable = true;
  services.power-profiles-daemon.enable = true;
  systemd.services.power-profiles-daemon.wantedBy = [ "multi-user.target" ];
  services.gnome.gnome-keyring.enable = true;

  programs.steam.enable = true;
  services.lact.enable = true;
  programs.thunar.enable = true;
  # Custom folder icons for Anime, Manga and Projects.
  nixpkgs.overlays = [
    (final: prev: {
      thunar-unwrapped = prev.thunar-unwrapped.overrideAttrs (old: {
        patches = (old.patches or [ ]) ++ [ ./thunar-folder-icons.patch ];
      });
    })
  ];
  services.tumbler.enable = true;
  services.gvfs.enable = true;

    fonts.packages = [ pkgs.nerd-fonts.jetbrains-mono ];

    programs.zsh = {
      enable = true;
      enableCompletion = true;
      autosuggestions.enable = true;
      syntaxHighlighting.enable = true;
      ohMyZsh = {
        enable = true;
        plugins = [ "git" "fzf" "tmux" ];
      };
  };	
}
