import 'dart:io';
import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/src/sidebar.dart';
import 'chats_preview_test.dart' show loadFonts;
import 'tool_evidence_test.dart' show measureContrast;

void main() {
  for (final brightness in Brightness.values) {
    for (final scale in [1.0, 1.5]) {
      testWidgets('estados vacíos legibles ${brightness.name} $scale', (
        t,
      ) async {
        await loadFonts();
        t.view.physicalSize = const Size(640, 420);
        t.view.devicePixelRatio = 1;
        addTearDown(t.view.resetPhysicalSize);
        addTearDown(t.view.resetDevicePixelRatio);
        final dark = brightness == Brightness.dark;
        // Superficies sRGB de las muestras PNG nativas del 30 de octubre.
        // El vidrio claro real es más oscuro que el fixture blanco anterior.
        await t.pumpWidget(
          MaterialApp(
            theme: ThemeData(brightness: brightness),
            home: MediaQuery(
              data: MediaQueryData(textScaler: TextScaler.linear(scale)),
              child: RepaintBoundary(
                key: const ValueKey('capture'),
                child: Material(
                  type: MaterialType.transparency,
                  child: Row(
                    children: [
                      SizedBox(
                        width: 216,
                        child: ColoredBox(
                          color: dark
                              ? const Color(0xff2b2b2f)
                              : const Color(0xffdbdbda),
                          child: const SidebarEmpty(),
                        ),
                      ),
                      Expanded(
                        child: ColoredBox(
                          color: dark
                              ? const Color(0xff26262a)
                              : const Color(0xffcececd),
                          child: const ContentEmpty(title: 'Encargos'),
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ),
        );
        await t.pumpAndSettle();
        const message = 'No hay contenido disponible';
        final ratios = await measureContrast(t, {
          'sidebar': find.descendant(
            of: find.byType(SidebarEmpty),
            matching: find.text(message),
          ),
          'contenido': find.descendant(
            of: find.byType(ContentEmpty),
            matching: find.text(message),
          ),
        });
        expect(t.takeException(), isNull);
        if (const bool.fromEnvironment('CAPTURE_UI')) {
          final output = Directory(
            const String.fromEnvironment('UI_CAPTURE_DIR'),
          )..createSync(recursive: true);
          await t.runAsync(() async {
            final image = await t
                .renderObject<RenderRepaintBoundary>(
                  find.byKey(const ValueKey('capture')),
                )
                .toImage();
            final png = await image.toByteData(format: ui.ImageByteFormat.png);
            File(
              '${output.path}/empty-${brightness.name}-$scale.png',
            ).writeAsBytesSync(png!.buffer.asUint8List());
            File(
              '${output.path}/ratios-${brightness.name}-$scale.txt',
            ).writeAsStringSync('$ratios\n');
            image.dispose();
          });
        }
      });
    }
  }
}
