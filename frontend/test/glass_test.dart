import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/src/glass.dart';

void main() {
  testWidgets('El canvas macOS deja pasar el material nativo de la ventana', (
    tester,
  ) async {
    final key = GlobalKey();
    await tester.pumpWidget(
      MaterialApp(
        home: RepaintBoundary(
          key: key,
          child: const GlassCanvas(
            child: Glass(canvas: true, radius: 0, child: SizedBox.expand()),
          ),
        ),
      ),
    );
    final boundary =
        key.currentContext!.findRenderObject()! as RenderRepaintBoundary;
    await tester.runAsync(() async {
      final image = await boundary.toImage();
      final pixels = await image.toByteData(format: ui.ImageByteFormat.rawRgba);
      final offset = ((image.height ~/ 2) * image.width + image.width ~/ 2) * 4;
      final alpha = pixels!.getUint8(offset + 3);
      image.dispose();
      // Un fondo opaco, aunque tenga gradiente o blur, oculta el escritorio.
      expect(alpha, lessThan(100));
    });
  }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));
}
