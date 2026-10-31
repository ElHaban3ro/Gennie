import 'dart:convert';
import 'dart:io';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'assignment_execution_test.dart' show ExecutionFixture, mount, press;
import 'chats_preview_test.dart' show loadFonts;
import 'tool_evidence_test.dart' show capture, measureContrast;

const consent = 'Valorar cuándo avisarme con la guía remota';
SwitchListTile toggle(WidgetTester t, String title) =>
    t.widget<SwitchListTile>(find.widgetWithText(SwitchListTile, title));

void main() {
  setUpAll(loadFonts);
  testWidgets(
    'retira consentimiento sin plugin activo ni reconfigurar la función',
    (t) async {
      final b = ExecutionFixture()
        ..existing = true
        ..contextual = true
        ..active = false;
      await mount(t, backend: b);
      await press(t, 'Desactivar guía y avisos');
      expect(b.requests.where((r) => r['action'] == 'revoke_guide').single, {
        'action': 'revoke_guide',
        'policy_id': 'policy-fixture',
        'expected_revision': 2,
      });
      expect(b.requests.where((r) => r['action'] == 'configure'), isEmpty);
      expect(find.text('Desactivar guía y avisos'), findsNothing);
      expect(t.takeException(), isNull);
    },
  );
  testWidgets(
    'consentimiento remoto separado, desmarcado y conservado ante error',
    (t) async {
      final b = await mount(
        t,
        backend: ExecutionFixture(preparationOnly: true)..fail = true,
      );
      await press(t, 'Configurar ejecución');
      expect(toggle(t, consent).value, false);
      expect(toggle(t, consent).onChanged, isNull);
      await t.enterText(
        find.widgetWithText(TextFormField, 'Consulta'),
        'novedades',
      );
      await press(t, 'Recibir resultados en el chat de Gennie');
      await press(t, consent);
      expect(toggle(t, consent).value, true);
      await press(t, 'Guardar pausada');
      expect(
        find.textContaining('No se pudo guardar la ejecución').hitTestable(),
        findsOneWidget,
      );
      expect(toggle(t, consent).value, true);
      final config = b.requests.singleWhere(
        (r) => r['action'] == 'configure',
      )['configuration'];
      expect(config['contextual_notice'], true);
      expect(config['enabled'], false);
      expect(config['delivery']['conversation_id'], 3);
      await press(t, 'Recibir resultados en el chat de Gennie');
      await press(t, 'Guardar pausada');
      expect(
        b.requests
            .where((r) => r['action'] == 'configure')
            .last['configuration']['contextual_notice'],
        false,
      );
      expect(t.takeException(), isNull);
    },
  );
  testWidgets('sin criterios no ofrece ni autoriza valoración contextual', (
    t,
  ) async {
    final b = await mount(
      t,
      backend: ExecutionFixture(preparationOnly: true, followUp: false),
    );
    await press(t, 'Configurar ejecución');
    expect(find.text(consent), findsNothing);
    await t.enterText(
      find.widgetWithText(TextFormField, 'Consulta'),
      'novedades',
    );
    await press(t, 'Guardar pausada');
    expect(
      b.requests.singleWhere(
        (r) => r['action'] == 'configure',
      )['configuration']['contextual_notice'],
      false,
    );
  });
  testWidgets(
    'consultar y actualizar una valoración pendiente no ejecuta ni reconcilia',
    (t) async {
      final b = ExecutionFixture()
        ..existing = true
        ..contextual = true
        ..runState = 'succeeded'
        ..notification = {'state': 'rendering', 'guide_state': 'reserved'};
      await mount(t, backend: b);
      await press(t, 'Ver ejecuciones');
      await press(t, 'Ver detalle');
      await press(t, 'Actualizar valoración');
      b.notification = {
        'state': 'discarded',
        'guide_state': 'ready',
        'action': 'record',
        'reason': 'Sin novedades que interrumpan el acuerdo',
        'guide_advice': {
          'probabilities': {'record': 0.987654321},
          'cost_reported': 0.002,
        },
      };
      await press(t, 'Actualizar valoración');
      expect(find.text('Resultado guardado sin aviso'), findsOneWidget);
      expect(
        find.text('Sin novedades que interrumpan el acuerdo'),
        findsOneWidget,
      );
      expect(find.textContaining('0.987654321'), findsNothing);
      expect(
        b.requests.where(
          (r) => ['configure', 'reconcile'].contains(r['action']),
        ),
        isEmpty,
      );
      expect(t.takeException(), isNull);
    },
  );
  for (final brightness in Brightness.values) {
    for (final (size, scale) in [
      (const Size(1100, 720), 1.0),
      (const Size(640, 420), 1.0),
      (const Size(640, 420), 1.5),
    ]) {
      testWidgets(
        'guía contextual ${brightness.name} ${size.width} texto $scale',
        (t) async {
          t.platformDispatcher.platformBrightnessTestValue = brightness;
          t.platformDispatcher.textScaleFactorTestValue = scale;
          addTearDown(t.platformDispatcher.clearPlatformBrightnessTestValue);
          addTearDown(t.platformDispatcher.clearTextScaleFactorTestValue);
          final b = await mount(
            t,
            size: size,
            backend: ExecutionFixture(preparationOnly: true),
          );
          await press(t, 'Configurar ejecución');
          await press(t, 'Recibir resultados en el chat de Gennie');
          await press(t, consent);
          final name =
              'contextual-${brightness.name}-${size.width.toInt()}-$scale';
          expect(find.text('Guardar pausada').hitTestable(), findsOneWidget);
          await capture(t, '$name-consent');
          final ratios = await measureContrast(t, {
            'consentimiento': find.text(consent),
            'cancelar': find.text('Cancelar'),
          });
          await press(t, 'Cancelar');
          b.existing = true;
          b.contextual = true;
          b.runState = 'succeeded';
          b.notification = {
            'state': 'discarded',
            'guide_state': 'unavailable',
            'action': 'record',
            'failure_code': 'render_failed',
          };
          await t.pumpWidget(const SizedBox.shrink());
          await mount(t, size: size, backend: b);
          await press(t, 'Ver ejecuciones');
          await press(t, 'Ver detalle');
          await Scrollable.ensureVisible(
            t.element(find.text('Resultado guardado sin aviso')),
            alignment: .5,
          );
          await t.pumpAndSettle();
          expect(find.text('Cerrar').last.hitTestable(), findsOneWidget);
          await capture(t, '$name-recorded');
          ratios.addAll(
            await measureContrast(t, {
              'resultado': find.text('Resultado guardado sin aviso'),
              'estado': find.textContaining(
                'La valoración no quedó confirmada',
              ),
            }),
          );
          if (const bool.fromEnvironment('CAPTURE_UI')) {
            File(
              '../.verification/assistant-chat-real/$name-contrast.json',
            ).writeAsStringSync(jsonEncode(ratios));
          }
          expect(t.takeException(), isNull);
        },
      );
    }
  }
}
