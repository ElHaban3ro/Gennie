import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/src/assignments.dart';
import 'package:gennie/src/backend.dart';
import 'chats_preview_test.dart' show loadFonts;
import 'tool_evidence_test.dart' show measureContrast;

void main() {
  for (final brightness in Brightness.values) {
    for (final scale in [1.0, 1.5]) {
      testWidgets('entrega legible ${brightness.name} $scale', (tester) async {
        await loadFonts();
        tester.view.physicalSize = const Size(640, 420);
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);
        await tester.pumpWidget(
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
                  child: AssignmentProfile(
                    record: AssignmentRecord(
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
                        'recurrence': {
                          'frequency': 'daily',
                          'local_time': '08:00',
                        },
                      }),
                    ),
                    onEdit: () => fail('La lectura no debe abrir el editor'),
                  ),
                ),
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();
        await tester.ensureVisible(find.text('America/Bogota'));
        await tester.pumpAndSettle();
        await measureContrast(tester, {
          'rótulo de entrega': find.text('Cuándo recibir la información'),
          'zona horaria': find.text('America/Bogota'),
        });
        final viewport = tester.getRect(find.byType(SingleChildScrollView));
        for (final label in [
          'Cuándo recibir la información',
          'America/Bogota',
        ]) {
          final rect = tester.getRect(find.text(label));
          expect(rect.top, greaterThanOrEqualTo(viewport.top));
          expect(rect.bottom, lessThanOrEqualTo(viewport.bottom));
          expect(
            tester
                .renderObject<RenderParagraph>(find.text(label))
                .didExceedMaxLines,
            isFalse,
          );
        }
        expect(tester.takeException(), isNull);
      });
    }
  }
}
