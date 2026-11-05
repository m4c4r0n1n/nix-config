# Hides quiet visualizer bars instead of drawing them on the center line.
pkg: pkg.overrideAttrs (o: {
  postPatch = (o.postPatch or "") + ''
    substituteInPlace src/shell/render/programs/audio_spectrum_program.cpp \
      --replace-fail "float crossPixels = std::max(1.0F, std::floor(rawValue * crossAxisLen * crossPixelScale + 0.5F));" \
      "if (rawValue < 0.10F) { continue; } float crossPixels = std::max(1.0F, std::floor(rawValue * crossAxisLen * crossPixelScale + 0.5F));"
  '';
})
