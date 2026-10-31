import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/src/assignments.dart';
import 'package:gennie/src/backend.dart';
import 'assignments_test.dart' show AssignmentBackend, openEditorSection;
import 'chats_preview_test.dart' show loadFonts;
import 'heading_form_contrast_test.dart' show surface;
import 'tool_evidence_test.dart' show measureContrast;

Future<void> showTarget(WidgetTester t, Finder target) async {
  await Scrollable.ensureVisible(t.element(target), alignment: .5);
  await t.pumpAndSettle();
  expect(t.getRect(target).top, greaterThanOrEqualTo(2));
}

void main() {
  for (final brightness in Brightness.values) {
    for (final scale in [1.0, 1.5]) {
      testWidgets('resúmenes del editor legibles ${brightness.name} $scale', (
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
        for (final text in [
          'La tarea, sus fuentes y cuándo recibirla.',
          'Ninguna aplicación añadida',
          'Método pendiente; autorización independiente',
          'Información adicional para interpretar la tarea.',
          'Cadencia y pasos para reunir la información.',
          'Activo · Importancia normal',
        ]) {
          final target = find.text(text);
          await showTarget(t, target);
          await measureContrast(t, {text: target});
        }
        await openEditorSection(t, 'preparation');
        final note = find.text(
          'Reunir información no cambia la hora del aviso.',
        );
        await showTarget(t, note);
        await measureContrast(t, {'nota de preparación': note});
        expect(backend.saved, isEmpty);
        expect(t.takeException(), isNull);
      });

      testWidgets(
        'textos secundarios de ficha legibles ${brightness.name} $scale',
        (t) async {
          await loadFonts();
          t.view.physicalSize = const Size(640, 420);
          t.view.devicePixelRatio = 1;
          addTearDown(t.view.resetPhysicalSize);
          addTearDown(t.view.resetDevicePixelRatio);
          const channel = MethodChannel('gennie/assignment-logo');
          TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
              .setMockMethodCallHandler(channel, (_) async => null);
          addTearDown(
            () => TestDefaultBinaryMessengerBinding
                .instance
                .defaultBinaryMessenger
                .setMockMethodCallHandler(channel, null),
          );
          final record = AssignmentRecord(
            id: 'fixture',
            revision: 1,
            createdAt: '',
            updatedAt: '',
            document: jsonEncode({
              'title': 'Resumen de prueba',
              'description': 'Revisar novedades',
              'kind': 'recurring',
              'priority': 'normal',
              'status': 'active',
              'timezone': 'America/Bogota',
              'recurrence': {'frequency': 'daily', 'local_time': '08:00'},
              'applications': [
                {
                  'name': 'Aplicación de prueba',
                  'kind': 'desktop',
                  'url': 'https://example.invalid/notas',
                  'plugin_available': false,
                  'manual_required': true,
                },
              ],
              'preparation': {
                'interval_minutes': 15,
                'rationale': 'Reunir información',
                'steps': ['Primer paso', 'Segundo paso'],
              },
              'follow_up': {
                'scope': 'Cambios relevantes',
                'signals': ['Cambio confirmado'],
              },
            }),
          );
          await t.pumpWidget(
            surface(
              brightness,
              scale,
              AssignmentProfile(
                record: record,
                onEdit: () => fail('Sin edición'),
              ),
            ),
          );
          await t.pumpAndSettle();
          final details = find.text('Detalles del encargo');
          await showTarget(t, details);
          await t.tap(details);
          await t.pumpAndSettle();
          for (final target in [
            find.text('https://example.invalid/notas'),
            find.text('Cada 15 minutos · 2 pasos de preparación'),
            find.text('Cambios relevantes'),
          ]) {
            await showTarget(t, target);
            await measureContrast(t, {'texto secundario': target});
          }
          for (final (section, markers) in [
            ('Preparación interna', ['1.', '2.']),
            ('Seguimiento', ['•']),
          ]) {
            final button = find.text(section);
            await showTarget(t, button);
            await t.tap(button);
            await t.pumpAndSettle();
            for (final marker in markers) {
              final target = find.text(marker);
              await showTarget(t, target);
              await measureContrast(t, {marker: target});
            }
          }
          expect(t.takeException(), isNull);
        },
      );
    }
  }
}
