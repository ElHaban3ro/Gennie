import 'dart:math' as math;
import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/src/backend.dart';
import 'package:gennie/src/chat_background.dart';
import 'package:gennie/src/chat_message.dart';

void main() {
  testWidgets('El blur borra el patrón con un canvas translúcido', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(600, 300);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    Future<double> texture({required bool bubble}) async {
      await tester.pumpWidget(
        MaterialApp(
          theme: ThemeData(brightness: Brightness.dark),
          home: RepaintBoundary(
            key: const ValueKey('capture-blur'),
            child: Stack(
              fit: StackFit.expand,
              children: [
                ColoredBox(
                  color: const Color(0xff26282c).withValues(alpha: .68),
                ),
                const ChatBackground(),
                if (bubble)
                  Positioned(
                    top: 40,
                    left: 40,
                    right: 40,
                    child: ChatMessage(
                      message: Message(
                        speaker: 'user',
                        text: '\n\n\n\n',
                        createdAt: '',
                      ),
                    ),
                  ),
              ],
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      final boundary = tester.renderObject<RenderRepaintBoundary>(
        find.byKey(const ValueKey('capture-blur')),
      );
      return tester
          .runAsync(() async {
            final image = await boundary.toImage();
            final bytes = (await image.toByteData(
              format: ui.ImageByteFormat.rawStraightRgba,
            ))!;
            final values = <double>[];
            // Interior sin texto ni bordes; simular el escritorio nativo bajo el alfa.
            for (var y = 65; y < 140; y++) {
              for (var x = 380; x < 530; x++) {
                final i = (y * image.width + x) * 4;
                final a = bytes.getUint8(i + 3) / 255;
                values.add(bytes.getUint8(i) * a + 80 * (1 - a));
              }
            }
            image.dispose();
            final mean = values.reduce((a, b) => a + b) / values.length;
            return values
                    .map((v) => math.pow(v - mean, 2))
                    .reduce((a, b) => a + b) /
                values.length;
          })
          .then((v) => v!);
    }

    final without = await texture(bubble: false);
    final withBlur = await texture(bubble: true);
    // Debe eliminar al menos el 90 % de la variación del dibujo.
    expect(
      withBlur / without,
      lessThan(.1),
      reason: 'Varianza patrón: $without; dentro del globo: $withBlur',
    );
  });
}
