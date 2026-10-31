import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/src/assignments.dart';
import 'package:gennie/src/backend.dart';
import 'package:gennie/src/detail_view.dart';
import 'assignments_test.dart' show AssignmentBackend, openEditorSection;
import 'chats_preview_test.dart' show loadFonts;
import 'tool_evidence_test.dart' show measureContrast;

Widget surface(Brightness brightness, double scale, Widget child) =>
    MaterialApp(
      theme: ThemeData(brightness: brightness),
      home: MediaQuery(
        data: MediaQueryData(textScaler: TextScaler.linear(scale)),
        child: RepaintBoundary(
          key: const ValueKey('capture'),
          child: Material(
            color: brightness == Brightness.dark
                ? const Color(0xff26262a)
                : const Color(0xffcececd),
            child: child,
          ),
        ),
      ),
    );

void main() {
  for (final brightness in Brightness.values) {
    for (final scale in [1.0, 1.5]) {
      testWidgets('encabezado legible ${brightness.name} $scale', (t) async {
        await loadFonts();
        t.view.physicalSize = const Size(640, 420);
        t.view.devicePixelRatio = 1;
        addTearDown(t.view.resetPhysicalSize);
        addTearDown(t.view.resetDevicePixelRatio);
        await t.pumpWidget(
          surface(
            brightness,
            scale,
            const Padding(
              padding: EdgeInsets.all(24),
              child: AppDetailHeading(
                icon: Icon(Icons.calendar_month, size: 72),
                label: 'Encargo',
                title: 'Resumen de prueba',
                subtitle: Text('Fuente de prueba'),
              ),
            ),
          ),
        );
        await t.pumpAndSettle();
        await measureContrast(t, {
          'etiqueta': find.text('Encargo'),
          'subtítulo': find.text('Fuente de prueba'),
        });
        expect(t.takeException(), isNull);
      });

      testWidgets('etiquetas y ayudas legibles ${brightness.name} $scale', (
        t,
      ) async {
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
              onReload: () => fail('No debe recargar al consultar'),
              onSaved: (_) => fail('No debe guardar al consultar'),
            ),
          ),
        );
        await t.pumpAndSettle();
        await openEditorSection(t, 'preparation');
        for (final label in [
          'Intervalo de revisión',
          'Minutos entre revisiones. Opcional.',
          'Pasos de preparación',
          'Un paso por línea.',
        ]) {
          final target = find.text(label);
          await Scrollable.ensureVisible(t.element(target), alignment: .5);
          await t.pumpAndSettle();
          expect(t.getRect(target).top, greaterThanOrEqualTo(2));
          await measureContrast(t, {label: target});
          expect(
            t.renderObject<RenderParagraph>(target).didExceedMaxLines,
            isFalse,
          );
        }
        expect(backend.saved, isEmpty);
        expect(t.takeException(), isNull);
      });
    }
  }
}
