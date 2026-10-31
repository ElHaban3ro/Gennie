import 'dart:io';

// Cada ensayo puede aislar sus artefactos sin sobrescribir capturas anteriores.
File captureOutput(String defaultDirectory, String name) {
  const requested = String.fromEnvironment('UI_CAPTURE_DIR');
  final directory = Directory(requested.isEmpty ? defaultDirectory : requested)
    ..createSync(recursive: true);
  return File('${directory.path}/$name');
}
