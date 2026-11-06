{ lib, rustPlatform, fetchFromGitHub, makeWrapper, librsvg, ffmpeg, poppler-utils, fd, ripgrep, wl-clipboard, xdg-utils, libarchive, zip, glib, ripdrag }:

rustPlatform.buildRustPackage {
  pname = "file-tui";
  version = "1.0.0";

  src = fetchFromGitHub {
    owner = "m4c4r0n1n";
    repo = "file-tui";
    rev = "37d4627537cb0bdffbe76818fb30f169de788545";
    hash = "sha256-+5rkr3AI/6jNAKLQ3vD+b7oftxtBLCdH5DCMvDyyoaw=";
  };

  cargoHash = "sha256-ZXB1oJx5llZPx/dcll4xTWV0t0Wl95Sfkt+srsu5nZg=";
  nativeBuildInputs = [ makeWrapper ];
  doCheck = false;

  postInstall = ''
    wrapProgram $out/bin/file-tui --set-default TERMINAL ghostty --prefix PATH : ${lib.makeBinPath [ librsvg ffmpeg poppler-utils fd ripgrep wl-clipboard xdg-utils libarchive zip glib ripdrag ]}
    install -Dm644 file-tui.desktop $out/share/applications/file-tui.desktop
  '';
}
