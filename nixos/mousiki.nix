# Mousiki Plus, built from its release source.
{ lib, stdenv, fetchFromGitHub, cmake, pkg-config, makeWrapper, alsa-lib
, libpulseaudio, SDL2, ffmpeg, yt-dlp, python3, curl, wl-clipboard }:

stdenv.mkDerivation rec {
  pname = "mousiki-plus";
  version = "4.9.0";

  src = fetchFromGitHub {
    owner = "StSchwerdtfeger";
    repo = "mousiki-plus";
    rev = "v${version}";
    hash = "sha256-/JE0q2I3Hck+bORASpVtRoxySkxOUpcdM5BLQpSm2tk=";
  };

  # Gradients use the wallpaper palette, and YouTube streams keep their thumbnail.
  patches = [ ./mousiki-palette.patch ./mousiki-thumbnails.patch ];

  # The source misses this include, which newer GCC rejects.
  env.CXXFLAGS = "-include cstdint";

  nativeBuildInputs = [ cmake pkg-config makeWrapper ];
  buildInputs = [ alsa-lib libpulseaudio SDL2 ];

  # It looks for scripts/ and its data files beside the real binary.
  installPhase = ''
    runHook preInstall
    mkdir -p $out/lib/mousiki $out/bin
    cp mousiki $out/lib/mousiki/
    cp -r scripts $out/lib/mousiki/
    cp $src/stations.txt $src/radio_config.txt $out/lib/mousiki/
    makeWrapper $out/lib/mousiki/mousiki $out/bin/mousiki \
      --prefix PATH : ${lib.makeBinPath [ ffmpeg yt-dlp python3 curl wl-clipboard ]} \
      --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath [ alsa-lib libpulseaudio SDL2 ]}
    runHook postInstall
  '';
}
