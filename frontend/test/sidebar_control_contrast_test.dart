import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/src/sidebar.dart';
import 'chats_preview_test.dart' show loadFonts;

void main() {
  for (final brightness in Brightness.values) {
    for (final scale in [1.0, 1.5]) {
      testWidgets('controles laterales visibles ${brightness.name} $scale', (
        t,
      ) async {
        await loadFonts();
        t.view.physicalSize = const Size(640, 420);
        t.view.devicePixelRatio = 1;
        addTearDown(t.view.resetPhysicalSize);
        addTearDown(t.view.resetDevicePixelRatio);
        final search = TextEditingController(text: 'resumen');
        addTearDown(search.dispose);
        final changes = <String>[];
        await t.pumpWidget(
          MaterialApp(
            theme: ThemeData(brightness: brightness),
            home: MediaQuery(
              data: MediaQueryData(textScaler: TextScaler.linear(scale)),
              child: RepaintBoundary(
                key: const ValueKey('capture'),
                child: Material(
                  type: MaterialType.transparency,
                  child: ColoredBox(
                    color: brightness == Brightness.dark
                        ? const Color(0xff26262a)
                        : const Color(0xffcececd),
                    child: Align(
                      alignment: Alignment.centerLeft,
                      child: StatefulBuilder(
                        builder: (context, rebuild) => AppSidebar(
                          width: 216,
                          title: 'Encargos',
                          searchController: search,
                          searchHint: 'Buscar encargos…',
                          onSearch: (value) =>
                              rebuild(() => changes.add(value)),
                          child: const SizedBox(),
                        ),
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ),
        );
        await t.pumpAndSettle();
        await t.tap(find.byType(TextField));
        await t.pumpAndSettle();
        final editable = find.byType(EditableText);
        expect(t.widget<EditableText>(editable).focusNode.hasFocus, isTrue);
        expect(
          t.widget<InputDecorator>(find.byType(InputDecorator)).isFocused,
          isTrue,
        );
        final boundary = t.renderObject<RenderRepaintBoundary>(
          find.byKey(const ValueKey('capture')),
        );
        final container = InputDecorator.containerOf(t.element(editable))!;
        final origin = boundary.globalToLocal(
          container.localToGlobal(Offset.zero),
        );
        final field = t.widget<TextField>(find.byType(TextField));
        final clear = find.byIcon(Icons.backspace_outlined);
        final icon = t.widget<Icon>(clear);
        final iconRect = t.getRect(clear);
        final bounds = Rect.fromLTWH(
          origin.dx,
          origin.dy,
          container.size.width,
          container.size.height,
        );
        expect(
          const Rect.fromLTWH(0, 0, 216, 420).contains(bounds.topLeft),
          isTrue,
        );
        expect(
          const Rect.fromLTWH(0, 0, 216, 420).contains(bounds.bottomRight),
          isTrue,
        );
        final ratios = <String, double>{};
        await t.runAsync(() async {
          final image = await boundary.toImage(pixelRatio: 1);
          try {
            final rgba = (await image.toByteData(
              format: ui.ImageByteFormat.rawRgba,
            ))!;
            Color pixel(Offset point) {
              final local = boundary.globalToLocal(point);
              final offset =
                  (local.dy.floor() * image.width + local.dx.floor()) * 4;
              expect(rgba.getUint8(offset + 3), 255);
              return Color.fromARGB(
                255,
                rgba.getUint8(offset),
                rgba.getUint8(offset + 1),
                rgba.getUint8(offset + 2),
              );
            }

            double contrast(Color ink, Color background) {
              final a = Color.alphaBlend(ink, background).computeLuminance();
              final b = background.computeLuminance();
              return (a > b ? a + .05 : b + .05) / (a > b ? b + .05 : a + .05);
            }

            final bg = pixel(
              container.localToGlobal(Offset(container.size.width / 2, -3)),
            );
            final line = pixel(
              container.localToGlobal(
                Offset(container.size.width / 2, container.size.height - 1),
              ),
            );
            expect(line, isNot(bg));
            ratios['foco'] = contrast(
              field.decoration!.focusedBorder!.borderSide.color,
              bg,
            );
            ratios['limpiar'] = contrast(
              icon.color!,
              pixel(Offset(iconRect.left - 3, iconRect.center.dy)),
            );
            if (const bool.fromEnvironment('CAPTURE_UI')) {
              final directory = Directory(
                const String.fromEnvironment('UI_CAPTURE_DIR'),
              )..createSync(recursive: true);
              final name = 'sidebar-controls-${brightness.name}-$scale';
              final png = (await image.toByteData(
                format: ui.ImageByteFormat.png,
              ))!;
              File(
                '${directory.path}/$name.png',
              ).writeAsBytesSync(png.buffer.asUint8List());
              File('${directory.path}/$name.json').writeAsStringSync(
                jsonEncode({
                  'ratios': ratios,
                  'field_bounds': [
                    bounds.left,
                    bounds.top,
                    bounds.width,
                    bounds.height,
                  ],
                  'clear_bounds': [
                    iconRect.left,
                    iconRect.top,
                    iconRect.width,
                    iconRect.height,
                  ],
                }),
              );
            }
          } finally {
            image.dispose();
          }
        });
        expect(changes, isEmpty);
        await t.tap(clear);
        await t.pumpAndSettle();
        expect(search.text, isEmpty);
        expect(changes, ['']);
        expect(find.byIcon(Icons.backspace_outlined), findsNothing);
        expect(t.takeException(), isNull);
        expect(
          ratios.values.every((ratio) => ratio >= 3),
          isTrue,
          reason: '$ratios',
        );
      });
    }
  }
}
