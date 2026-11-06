{ lib, rustPlatform, fetchFromGitHub, makeWrapper, librsvg, ffmpeg, poppler-utils, fd, ripgrep, wl-clipboard, xdg-utils, libarchive, zip, glib }:

rustPlatform.buildRustPackage {
  pname = "file-tui";
  version = "1.0.0";

  src = fetchFromGitHub {
    owner = "m4c4r0n1n";
    repo = "file-tui";
    rev = "71c55c2b532041e25552b315f0e431fa685f53d4";
    hash = "sha256-P1ag9fq+yhC+mD48znMqVdIf6n/gzPCfJon4cFOMLXw=";
  };

  cargoHash = "sha256-ZXB1oJx5llZPx/dcll4xTWV0t0Wl95Sfkt+srsu5nZg=";
  nativeBuildInputs = [ makeWrapper ];
  doCheck = false;

  postInstall = ''
    wrapProgram $out/bin/file-tui --set-default TERMINAL ghostty --prefix PATH : ${lib.makeBinPath [ librsvg ffmpeg poppler-utils fd ripgrep wl-clipboard xdg-utils libarchive zip glib ]}
    install -Dm644 file-tui.desktop $out/share/applications/file-tui.desktop
  '';
}
