import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/src/chat_viewport.dart';

void main() {
  testWidgets(
    'El lector conserva el desvanecido y los últimos 12 px transparentes',
    (tester) async {
      tester.view.physicalSize = const Size(200, 200);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      await tester.pumpWidget(
        const MaterialApp(
          home: RepaintBoundary(
            key: ValueKey('capture-fade'),
            child: ChatViewport(child: ColoredBox(color: Colors.white)),
          ),
        ),
      );
      await tester.pumpAndSettle();
      final boundary = tester.renderObject<RenderRepaintBoundary>(
        find.byKey(const ValueKey('capture-fade')),
      );
      final alphas = await tester.runAsync(() async {
        final image = await boundary.toImage();
        final bytes = (await image.toByteData(
          format: ui.ImageByteFormat.rawStraightRgba,
        ))!;
        final result = [
          for (final y in [120, 170, 190])
            bytes.getUint8((y * image.width + 100) * 4 + 3),
        ];
        image.dispose();
        return result;
      });
      expect(alphas![0], 255);
      expect(alphas[1], inInclusiveRange(110, 140));
      expect(alphas[2], 0);
    },
  );
}
