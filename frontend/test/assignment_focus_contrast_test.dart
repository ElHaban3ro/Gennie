import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/src/assignments.dart';
import 'package:gennie/src/backend.dart';
import 'assignments_test.dart' show AssignmentBackend;
import 'chats_preview_test.dart' show loadFonts;
import 'heading_form_contrast_test.dart' show surface;

void main() {
  for (final brightness in Brightness.values) {
    for (final scale in [1.0, 1.5]) {
      testWidgets('foco de campo visible ${brightness.name} $scale', (t) async {
        await loadFonts();
        t.view.physicalSize = const Size(640, 420);
        t.view.devicePixelRatio = 1;
        addTearDown(t.view.resetPhysicalSize);
        addTearDown(t.view.resetDevicePixelRatio);
        final backend = AssignmentBackend();
        final state = DesktopState(backend);
        await state.start();
        addTearDown(() async {
          await t.pumpWidget(const SizedBox());
          state.dispose();
          await backend.events.close();
        });
        await t.pumpWidget(
          surface(
            brightness,
            scale,
            AssignmentEditor(
              state: state,
              onBack: () {},
              onReload: () => fail('Consultar no recarga'),
              onSaved: (_) => fail('Enfocar no guarda'),
            ),
          ),
        );
        await t.pumpAndSettle();
        final field = find.byType(TextFormField).first;
        await Scrollable.ensureVisible(t.element(field), alignment: .5);
        await t.pumpAndSettle();
        await t.tap(field);
        await t.pumpAndSettle();
        final editable = find.descendant(
          of: field,
          matching: find.byType(EditableText),
        );
        expect(t.widget<EditableText>(editable).focusNode.hasFocus, isTrue);
        final container = InputDecorator.containerOf(t.element(editable))!;
        final boundary = t.renderObject<RenderRepaintBoundary>(
          find.byKey(const ValueKey('capture')),
        );
        final origin = boundary.globalToLocal(
          container.localToGlobal(Offset.zero),
        );
        final x = (origin.dx + 24).round();
        final y = (origin.dy + container.size.height).floor() - 1;
        expect(y, inInclusiveRange(4, 416));
        late Color background;
        late List<Color> paintedLine;
        await t.runAsync(() async {
          final image = await boundary.toImage(pixelRatio: 1);
          try {
            final bytes = (await image.toByteData(
              format: ui.ImageByteFormat.rawRgba,
            ))!;
            Color pixel(int px, int py) {
              final offset = (py * image.width + px) * 4;
              expect(bytes.getUint8(offset + 3), 255);
              return Color.fromARGB(
                255,
                bytes.getUint8(offset),
                bytes.getUint8(offset + 1),
                bytes.getUint8(offset + 2),
              );
            }

            // Línea pintada y superficie adyacente, lejos de texto/cursor.
            background = pixel(x, y - 4);
            paintedLine = [for (var dx = 0; dx < 5; dx++) pixel(x + dx, y)];
          } finally {
            image.dispose();
          }
        });
        expect(paintedLine, everyElement(isNot(background)));
        final decoration = t.widget<InputDecorator>(
          find
              .ancestor(of: editable, matching: find.byType(InputDecorator))
              .first,
        );
        expect(decoration.isFocused, isTrue);
        final theme = Theme.of(t.element(editable));
        final foreground =
            theme.inputDecorationTheme.focusedBorder!.borderSide.color;
        // WCAG 1.4.11 usa el color declarado, no el borde suavizado de una
        // línea de un píxel situada entre coordenadas fraccionarias.
        final a = Color.alphaBlend(foreground, background).computeLuminance();
        final b = background.computeLuminance();
        final ratio = (a > b ? a + .05 : b + .05) / (a > b ? b + .05 : a + .05);
        expect(ratio, greaterThanOrEqualTo(3), reason: 'Línea de foco');
        expect(backend.saved, isEmpty);
        expect(t.takeException(), isNull);
      });
    }
  }
}
