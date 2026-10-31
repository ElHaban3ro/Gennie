import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/src/plugin_parameter.dart';
import 'plugins_test.dart' show PluginsBackend, record, mount, select;
import 'chats_preview_test.dart' show loadFonts;
import 'tool_evidence_test.dart' show measureContrast;

PluginsBackend typedBackend() {
  final plugin = record();
  final fn = (plugin['manifest']['functions'] as List).first as Map;
  fn['description'] = 'Preparar un mensaje con los datos que indiques.';
  fn['transport'] = 'program';
  fn['parameters'] = {
    'recipient': {
      'type': 'string',
      'label': 'Destinatario',
      'description': 'Persona a la que se enviará el mensaje.',
    },
    'message': {
      'type': 'string',
      'label': 'Mensaje',
      'description': 'Texto exacto que quieres enviar.',
      'maxLength': 100,
    },
    'urgent': {'type': 'boolean', 'label': 'Urgente'},
    'quantity': {
      'type': 'integer',
      'label': 'Cantidad',
      'required': false,
      'minimum': 0,
      'maximum': 20,
    },
    'tone': {
      'type': 'string',
      'label': 'Tono',
      'required': false,
      'enum': ['Formal', 'Cercano'],
    },
  };
  return PluginsBackend()..records = [plugin];
}

Future<void> enter(WidgetTester tester, String name, String value) async {
  final target = find.byKey(ValueKey('plugin-param-$name'));
  await tester.ensureVisible(target);
  await tester.enterText(target, value);
  await tester.pumpAndSettle();
}

Future<void> choose(WidgetTester tester, String name, String value) async {
  final target = find.byKey(ValueKey('plugin-param-$name'));
  await tester.ensureVisible(target);
  await tester.tap(target);
  await tester.pumpAndSettle();
  await tester.tap(find.text(value).last);
  await tester.pumpAndSettle();
}

Future<void> execute(WidgetTester tester) async {
  await tester.ensureVisible(find.text('Ejecutar función'));
  await tester.tap(find.text('Ejecutar función'));
  await tester.pumpAndSettle();
}

void main() {
  test('tipos, Unicode, opcionales y límites coinciden con el contrato', () {
    final text = PluginParameter('message', {'type': 'string', 'maxLength': 2});
    expect(text.validate('😀á'), isNull);
    expect(text.validate('😀áx'), contains('2'));
    final integer = PluginParameter('count', {
      'type': 'integer',
      'required': false,
      'minimum': 0,
      'maximum': 20,
    });
    expect(integer.validate(integer.parse('')), isNull);
    expect(integer.validate(integer.parse('0')), isNull);
    expect(integer.validate(integer.parse('1.5')), contains('entero'));
    expect(integer.validate(integer.parse('21')), contains('20'));
    final number = PluginParameter('ratio', {
      'type': 'number',
      'minimum': -.5,
      'maximum': 1.5,
    });
    expect(number.validate(number.parse('-0.25')), isNull);
    expect(number.validate(number.parse('NaN')), isNotNull);
    expect(number.validate(number.parse('Infinity')), isNotNull);
    expect(PluginParameter('id', 'string').isRequired, isTrue);
    expect(
      PluginParameter('urgent', {'type': 'boolean'}).validate(false),
      isNull,
    );
  });
  testWidgets('pide los campos ausentes con error local y foco, sin ejecutar', (
    tester,
  ) async {
    final backend = typedBackend();
    await mount(tester, backend);
    await select(tester);
    await execute(tester);
    expect(find.text('Completa Destinatario.'), findsOneWidget);
    expect(find.text('Completa Mensaje.'), findsOneWidget);
    expect(find.text('Completa Urgente.'), findsOneWidget);
    expect(
      backend.requests.where(
        (r) => ['run', 'verify', 'build'].contains(r['action']),
      ),
      isEmpty,
    );
    final field = tester.widget<TextField>(
      find.descendant(
        of: find.byKey(const ValueKey('plugin-param-recipient')),
        matching: find.byType(TextField),
      ),
    );
    expect(field.focusNode!.hasFocus, isTrue);
    expect(tester.takeException(), isNull);
  });
  testWidgets(
    'envía texto literal, false y cero; omite opcional y bloquea doble envío',
    (tester) async {
      final backend = typedBackend();
      await mount(tester, backend);
      await select(tester);
      await enter(tester, 'recipient', 'Cata');
      await enter(tester, 'message', 'Llegaré a las 8\nNos vemos.');
      await choose(tester, 'urgent', 'No');
      await enter(tester, 'quantity', '0');
      backend.pending = Completer<String>();
      await tester.ensureVisible(find.text('Ejecutar función'));
      await tester.tap(find.text('Ejecutar función'));
      await tester.pump();
      final sent = backend.requests.where((r) => r['action'] == 'run').toList();
      expect(sent, hasLength(1));
      expect(sent.single['arguments'], {
        'recipient': 'Cata',
        'message': 'Llegaré a las 8\nNos vemos.',
        'urgent': false,
        'quantity': 0,
      });
      expect(
        tester
            .widget<FilledButton>(
              find.widgetWithText(FilledButton, 'Ejecutando…'),
            )
            .onPressed,
        isNull,
      );
      backend.pending!.complete(
        jsonEncode({
          'status': 'ok',
          'data': {'sent': true},
        }),
      );
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
    },
  );
  testWidgets(
    'elección opcional se puede retirar y errores conservan borrador',
    (tester) async {
      final backend = typedBackend();
      await mount(tester, backend);
      await select(tester);
      await enter(tester, 'recipient', 'Cata');
      await enter(tester, 'message', 'Mensaje de prueba');
      await choose(tester, 'urgent', 'Sí');
      await choose(tester, 'tone', 'Formal');
      await choose(tester, 'tone', 'Sin especificar');
      await enter(tester, 'quantity', '21');
      await execute(tester);
      expect(find.text('El máximo es 20.'), findsOneWidget);
      expect(backend.requests.where((r) => r['action'] == 'run'), isEmpty);
      await enter(tester, 'quantity', '2');
      backend.pending = Completer<String>();
      await tester.ensureVisible(find.text('Ejecutar función'));
      await tester.tap(find.text('Ejecutar función'));
      await tester.pump();
      backend.pending!.complete(
        jsonEncode({
          'status': 'invalid_arguments',
          'executed': false,
          'fields': [
            {'name': 'recipient', 'reason': 'type'},
          ],
        }),
      );
      await tester.pumpAndSettle();
      expect(
        find.text('Revisa Destinatario antes de ejecutar.'),
        findsOneWidget,
      );
      expect(
        tester
            .widget<TextFormField>(
              find.byKey(const ValueKey('plugin-param-message')),
            )
            .controller!
            .text,
        'Mensaje de prueba',
      );
      expect(
        (backend.requests.lastWhere((r) => r['action'] == 'run')['arguments']
                as Map)
            .containsKey('tone'),
        isFalse,
      );
      expect(tester.takeException(), isNull);
    },
  );
  testWidgets('el botón se alcanza por teclado y valida antes de llamar', (
    tester,
  ) async {
    final backend = typedBackend();
    await mount(tester, backend);
    await select(tester);
    final target = find.text('Ejecutar función');
    await tester.ensureVisible(target);
    Focus.of(tester.element(target)).requestFocus();
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(find.text('Completa Destinatario.'), findsOneWidget);
    expect(backend.requests.where((r) => r['action'] == 'run'), isEmpty);
    expect(tester.takeException(), isNull);
  });
  for (final brightness in Brightness.values) {
    for (final (size, scale) in [
      (const Size(1100, 720), 1.0),
      (const Size(640, 420), 1.0),
      (const Size(640, 420), 1.5),
    ]) {
      testWidgets('formulario ${brightness.name} $size texto $scale', (
        tester,
      ) async {
        tester.platformDispatcher.textScaleFactorTestValue = scale;
        addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
        await loadFonts();
        final boundary = GlobalKey();
        final backend = typedBackend();
        await mount(
          tester,
          backend,
          size: size,
          brightness: brightness,
          boundary: boundary,
        );
        await select(tester);
        await execute(tester);
        expect(
          find.text('Completa Destinatario.').hitTestable(),
          findsOneWidget,
        );
        expect(tester.takeException(), isNull);
        final ratios = await measureContrast(tester, {
          'error': find.text('Completa Destinatario.'),
        }, captureBoundary: find.byKey(boundary));
        await tester.ensureVisible(find.text('Ejecutar función'));
        await tester.pumpAndSettle();
        ratios.addAll(
          await measureContrast(tester, {
            'Ejecutar función': find.text('Ejecutar función'),
          }, captureBoundary: find.byKey(boundary)),
        );
        await execute(tester);
        if (const bool.fromEnvironment('CAPTURE_UI')) {
          await tester.runAsync(() async {
            final render =
                boundary.currentContext!.findRenderObject()!
                    as RenderRepaintBoundary;
            final image = await render.toImage();
            final bytes = await image.toByteData(
              format: ui.ImageByteFormat.png,
            );
            final file = File(
              '../.verification/assistant-chat-real/plugin-input-${brightness.name}-${size.width.toInt()}-$scale.png',
            );
            file.parent.createSync(recursive: true);
            file.writeAsBytesSync(bytes!.buffer.asUint8List());
            File(
              '../.verification/assistant-chat-real/plugin-input-contrast-${brightness.name}-${size.width.toInt()}-$scale.json',
            ).writeAsStringSync(jsonEncode(ratios));
            image.dispose();
          });
        }
      });
    }
  }
}
