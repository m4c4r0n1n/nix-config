# Pen-Pen GRUB menu with NixOS and Windows icons.
evangelion:
{ pkgs, ... }:
let
  theme = pkgs.runCommand "penpen-grub-theme"
    { nativeBuildInputs = [ pkgs.imagemagick ]; } ''
      cp -r ${evangelion}/themes/penpen/1440p $out
      chmod -R u+w $out
      sed -i \
        -e 's/icon_width = 0/icon_width = 48/' \
        -e 's/icon_height = 0/icon_height = 48/' \
        -e 's/item_icon_space = 0/item_icon_space = 16/' \
        $out/theme.txt
      mkdir $out/icons
      cp ${pkgs.nixos-icons}/share/icons/hicolor/48x48/apps/nix-snowflake.png \
        $out/icons/nixos.png
      magick -size 48x48 xc:none -fill '#0078D4' -draw \
        'rectangle 2,2 22,22 rectangle 26,2 46,22 rectangle 2,26 22,46 rectangle 26,26 46,46' \
        PNG32:$out/icons/windows.png
    '';
in
{
  boot.loader.efi.canTouchEfiVariables = false;
  boot.loader.grub = {
    enable = true;
    device = "nodev";
    efiSupport = true;
    # The firmware starts \EFI\BOOT\BOOTX64.EFI first, so GRUB goes there.
    efiInstallAsRemovable = true;
    configurationLimit = 1;
    inherit theme;
    gfxmodeEfi = "3440x1440,2560x1440,auto";
    gfxpayloadEfi = "keep";
    # Show boot messages instead of a frozen menu after picking an entry.
    extraPerEntryConfig = "terminal_output console";
    extraEntries = ''
      menuentry "Windows" --class windows {
        insmod part_gpt
        insmod fat
        search --no-floppy --fs-uuid --set=root 52EE-3A46
        chainloader /EFI/Microsoft/Boot/bootmgfw.efi
      }
    '';
    # GRUB always adds this submenu; drop it so NixOS is listed once.
    extraInstallCommands = ''
      ${pkgs.gnused}/bin/sed -i \
        '/^submenu "NixOS - All configurations"/,/^}/d' /boot/grub/grub.cfg
    '';
  };
}
