{ lib, rustPlatform, fetchFromGitHub, makeWrapper, librsvg, ffmpeg, poppler-utils, fd, ripgrep, wl-clipboard, xdg-utils, libarchive, zip, glib }:

rustPlatform.buildRustPackage {
  pname = "file-tui";
  version = "1.0.0";

  src = fetchFromGitHub {
    owner = "adelmonte";
    repo = "file-browser-tui";
    rev = "fa9a4710f36670de25e17b7676bbf092d2a6b430";
    hash = "sha256-SCCEY0ZiYs8mCdC1C1sbtSOZoFbJRcnt2K+nirBj74M=";
  };

  # Ghostty text and menus, idle redraw rate, Trash delete.
  patches = [ ./file-tui.patch ];

  postPatch = ''
    substituteInPlace src/ui/sidebar.rs --replace-fail "ICON_ROWS: f32 = 1.65" "ICON_ROWS: f32 = 2.0"
    substituteInPlace src/icons/raster.rs --replace-fail 'cached.with_extension("tmp.png")' 'cached.with_extension(format!("{:?}.tmp.png", std::thread::current().id()))'
    find src -name '*.rs' -exec sed -i 's/file-browser-tui/file-tui/g' {} +
  '';

  cargoHash = "sha256-wtxQEaw23wEdgQq/ST7/AW7a2t/5ol/6c81/XnTSnYU=";
  nativeBuildInputs = [ makeWrapper ];
  doCheck = false;

  postInstall = ''
    mv $out/bin/file-browser-tui $out/bin/file-tui
    wrapProgram $out/bin/file-tui --set-default TERMINAL ghostty --prefix PATH : ${lib.makeBinPath [ librsvg ffmpeg poppler-utils fd ripgrep wl-clipboard xdg-utils libarchive zip glib ]}
    install -Dm644 file-browser-tui.desktop $out/share/applications/file-tui.desktop
    substituteInPlace $out/share/applications/file-tui.desktop \
      --replace-fail "Exec=file-browser-tui --kitty %f" "Exec=ghostty -e file-tui %f" \
      --replace-fail "Name=File Browser TUI" "Name=file-tui" \
      --replace-fail "A Dolphin-like file manager for kitty" "A file manager for the terminal" \
      --replace-fail "StartupWMClass=file-browser-tui" "StartupWMClass=file-tui"
  '';
}
