{ lib, rustPlatform, fetchFromGitHub, makeWrapper, librsvg, ffmpeg, poppler-utils, fd, ripgrep, wl-clipboard, xdg-utils, libarchive, zip, glib }:

rustPlatform.buildRustPackage {
  pname = "file-tui";
  version = "1.0.0";

  src = fetchFromGitHub {
    owner = "m4c4r0n1n";
    repo = "file-tui";
    rev = "0c313616b0c651e1dbd324d16e48919cad735167";
    hash = "sha256-pMmMCkEDEiCG3OF2ewADMScRxu+rpq+PMKedHlRHsvM=";
  };

  cargoHash = "sha256-ZXB1oJx5llZPx/dcll4xTWV0t0Wl95Sfkt+srsu5nZg=";
  nativeBuildInputs = [ makeWrapper ];
  doCheck = false;

  postInstall = ''
    wrapProgram $out/bin/file-tui --set-default TERMINAL ghostty --prefix PATH : ${lib.makeBinPath [ librsvg ffmpeg poppler-utils fd ripgrep wl-clipboard xdg-utils libarchive zip glib ]}
    install -Dm644 file-tui.desktop $out/share/applications/file-tui.desktop
  '';
}
