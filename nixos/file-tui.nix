{ lib, rustPlatform, fetchFromGitHub, makeWrapper, librsvg, ffmpeg, poppler-utils, fd, ripgrep, wl-clipboard, xdg-utils, libarchive, zip, glib }:

rustPlatform.buildRustPackage {
  pname = "file-tui";
  version = "1.0.0";

  src = fetchFromGitHub {
    owner = "m4c4r0n1n";
    repo = "file-tui";
    rev = "cfae734c62f99b1e66f43043f965b8ae21c89194";
    hash = "sha256-FxVLAKkQs5wk6SoIDumwEKo+2F+Bo9Alc+BZLosmPyI=";
  };

  cargoHash = "sha256-ZXB1oJx5llZPx/dcll4xTWV0t0Wl95Sfkt+srsu5nZg=";
  nativeBuildInputs = [ makeWrapper ];
  doCheck = false;

  postInstall = ''
    wrapProgram $out/bin/file-tui --set-default TERMINAL ghostty --prefix PATH : ${lib.makeBinPath [ librsvg ffmpeg poppler-utils fd ripgrep wl-clipboard xdg-utils libarchive zip glib ]}
    install -Dm644 file-tui.desktop $out/share/applications/file-tui.desktop
  '';
}
