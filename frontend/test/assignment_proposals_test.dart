import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/main.dart';
import 'package:gennie/src/backend.dart';
import 'widget_test.dart' as fixture;
import 'chats_preview_test.dart' show loadFonts;
import 'tool_evidence_test.dart' show capture, measureContrast;

Map<String, dynamic> plan() => {
  'title': 'Resumen de correo',
  'description': 'Revisar los mensajes nuevos',
  'kind': 'recurring',
  'priority': 'normal',
  'status': 'active',
  'timezone': 'America/Bogota',
  'recurrence': {'frequency': 'daily', 'local_time': '08:00', 'weekdays': []},
  'preparation': {
    'interval_minutes': 30,
    'rationale': 'Agrupar cambios',
    'steps': [],
  },
  'applications': [],
  'follow_up': {
    'scope': 'Mensajes nuevos',
    'signals': [],
    'decision_guidance': 'Avisar solo ante cambios relevantes',
    'action_scope': null,
  },
};

class ProposalBackend extends fixture.FakeBackend
    implements AssignmentProposalBackend {
  ProposalBackend() {
    value = fixture.snapshot(
      plans: [
        AssignmentRecord(
          id: '1',
          instanceId: 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
          revision: 3,
          createdAt: '',
          updatedAt: '',
          document: jsonEncode(plan()),
        ),
      ],
    );
  }
  final requests = <Map<String, dynamic>>[];
  bool showProposal = false,
      current = true,
      question = false,
      failBegin = false;
  bool failResolve = false;
  bool botAvailable = true;
  Completer<String>? pendingResolve;
  @override
  Future<Snapshot> send(String text) async {
    sent.add(text);
    return value;
  }

  Map<String, dynamic> review() {
    final changed = plan()..remove('follow_up');
    changed['description'] = 'Preparar un único resumen de cambios';
    return {
      'id': 'fixture-review',
      'state': 'ready',
      'current': current,
      'guide': null,
      'scope': {
        'assignments': [
          {
            'id': 1,
            'instance_id': 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
            'revision': 3,
            'document': plan(),
          },
        ],
      },
      'proposals': [
        {
          'id': 77,
          'resolution': 'pending',
          'candidate': {
            'assignment_id': 1,
            'title': 'Agrupar las novedades',
            'rationale': 'Reducir interrupciones repetidas',
            'expected_benefit': 'Un resumen más útil',
            'related_ids': [],
            'questions': question ? ['¿Qué carpeta quieres revisar?'] : [],
            'document': question ? null : changed,
          },
        },
      ],
    };
  }

  @override
  Future<String> proposalRequest(String text) async {
    final request = Map<String, dynamic>.from(jsonDecode(text) as Map);
    requests.add(request);
    switch (request['action']) {
      case 'availability':
        return jsonEncode({
          'guide_available': true,
          'bot_available': botAvailable,
        });
      case 'list':
        return jsonEncode({
          'reviews': showProposal ? [review()] : [],
        });
      case 'begin':
        if (failBegin) throw StateError('El encargo cambió');
        return jsonEncode({'id': 'fixture-review', 'current': true});
      case 'resolve':
        if (failResolve) throw StateError('Conflicto de revisión');
        return pendingResolve?.future ?? Future.value('{}');
      default:
        return '{}';
    }
  }
}

Future<ProposalBackend> mount(
  WidgetTester t, {
  ProposalBackend? backend,
  Size size = const Size(1100, 720),
}) async {
  final b = backend ?? ProposalBackend();
  t.view.physicalSize = size;
  t.view.devicePixelRatio = 1;
  addTearDown(t.view.resetPhysicalSize);
  addTearDown(t.view.resetDevicePixelRatio);
  final state = DesktopState(b);
  await state.start();
  addTearDown(state.dispose);
  await t.pumpWidget(
    RepaintBoundary(
      key: const ValueKey('capture'),
      child: GennieApp(state: state),
    ),
  );
  await t.pumpAndSettle();
  await t.tap(fixture.rail('Encargos'));
  await t.pumpAndSettle();
  await t.tap(find.text('Resumen de correo').first);
  await t.pumpAndSettle();
  return b;
}

void main() {
  setUpAll(loadFonts);
  testWidgets('Escape cierra la propuesta libre sin resolverla', (t) async {
    final b = await mount(t, backend: ProposalBackend()..showProposal = true);
    final invoker = Focus.of(t.element(find.text('Agrupar las novedades')));
    invoker.requestFocus();
    await t.pump();
    await t.sendKeyEvent(LogicalKeyboardKey.space);
    await t.pumpAndSettle();
    expect(find.text('Cambios propuestos'), findsOneWidget);
    await t.sendKeyEvent(LogicalKeyboardKey.tab);
    await t.pump();
    await t.sendKeyEvent(LogicalKeyboardKey.escape);
    await t.pumpAndSettle();
    expect(find.byType(Dialog), findsNothing);
    expect(invoker.hasPrimaryFocus, isTrue);
    expect(b.requests.where((r) => r['action'] == 'resolve'), isEmpty);
    expect(find.text('Agrupar las novedades'), findsOneWidget);
    expect(t.takeException(), isNull);
  }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));
  testWidgets('Escape no intenta enfocar una propuesta retirada al refrescar', (
    t,
  ) async {
    final b = await mount(t, backend: ProposalBackend()..showProposal = true);
    final invoker = Focus.of(t.element(find.text('Agrupar las novedades')));
    invoker.requestFocus();
    await t.pump();
    await t.sendKeyEvent(LogicalKeyboardKey.space);
    await t.pumpAndSettle();
    b.showProposal = false;
    await t.sendKeyEvent(LogicalKeyboardKey.escape);
    await t.pumpAndSettle();
    expect(find.byType(Dialog), findsNothing);
    expect(find.text('Agrupar las novedades'), findsNothing);
    expect(b.requests.where((r) => r['action'] == 'resolve'), isEmpty);
    expect(t.takeException(), isNull);
  }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));
  testWidgets('Escape protege el guardado y vuelve a cerrar tras su fallo', (
    t,
  ) async {
    final b = await mount(
      t,
      backend: ProposalBackend()
        ..showProposal = true
        ..pendingResolve = Completer<String>(),
    );
    await t.tap(find.text('Agrupar las novedades'));
    await t.pumpAndSettle();
    await t.tap(find.text('Aplicar cambios'));
    await t.pump();
    await t.sendKeyEvent(LogicalKeyboardKey.escape);
    await t.pumpAndSettle();
    expect(find.byType(Dialog), findsOneWidget);
    expect(b.requests.where((r) => r['action'] == 'resolve').length, 1);
    b.pendingResolve!.completeError(StateError('Conflicto de revisión'));
    await t.pumpAndSettle();
    expect(find.textContaining('Conflicto de revisión'), findsOneWidget);
    await t.sendKeyEvent(LogicalKeyboardKey.escape);
    await t.pumpAndSettle();
    expect(find.byType(Dialog), findsNothing);
    expect(b.requests.where((r) => r['action'] == 'resolve').length, 1);
    expect(find.text('Agrupar las novedades'), findsOneWidget);
    expect(t.takeException(), isNull);
  }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));
  testWidgets('falta de bot visible al mínimo y sin envío', (t) async {
    final b = await mount(
      t,
      size: const Size(640, 420),
      backend: ProposalBackend()..botAvailable = false,
    );
    await t.tap(find.text('Proponer mejoras'));
    await t.pumpAndSettle();
    expect(
      find
          .text('Activa la conversación con Gennie para pedir propuestas.')
          .hitTestable(),
      findsOneWidget,
    );
    expect(
      t
          .widget<FilledButton>(
            find.widgetWithText(FilledButton, 'Pedir propuestas'),
          )
          .onPressed,
      isNull,
    );
    expect(b.sent, isEmpty);
  });
  testWidgets('selección explícita, guía desmarcada y mensaje natural al bot', (
    t,
  ) async {
    final b = await mount(t);
    await t.tap(find.text('Proponer mejoras'));
    await t.pumpAndSettle();
    final remote = t.widget<CheckboxListTile>(
      find.widgetWithText(CheckboxListTile, 'Valorar con la guía remota'),
    );
    expect(remote.value, false);
    await t.tap(find.text('Pedir propuestas'));
    await t.pumpAndSettle();
    final request = b.requests.where((r) => r['action'] == 'begin').single;
    expect(request['use_guide'], false);
    expect(request['assignments'], [
      {
        'id': 1,
        'instance_id': 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
        'revision': 3,
      },
    ]);
    expect(b.sent.single, contains('encargos que seleccioné'));
    expect(b.sent.single, isNot(contains('fixture-review')));
    expect(t.takeException(), isNull);
  });
  testWidgets(
    'consulta remota solo por elección explícita y error conserva borrador',
    (t) async {
      final b = await mount(t, backend: ProposalBackend()..failBegin = true);
      await t.tap(find.text('Proponer mejoras'));
      await t.pumpAndSettle();
      await t.tap(find.text('Valorar con la guía remota'));
      await t.pumpAndSettle();
      await t.tap(find.text('Pedir propuestas'));
      await t.pumpAndSettle();
      expect(find.textContaining('El encargo cambió'), findsOneWidget);
      expect(b.sent, isEmpty);
      final first = b.requests.where((r) => r['action'] == 'begin').single;
      b.failBegin = false;
      await t.tap(find.text('Pedir propuestas'));
      await t.pumpAndSettle();
      final last = b.requests.where((r) => r['action'] == 'begin').last;
      expect(last['request_key'], first['request_key']);
      expect(last['use_guide'], true);
      expect(b.sent.length, 1);
    },
  );
  testWidgets('revisión muestra campos retirados y bloquea doble aplicación', (
    t,
  ) async {
    final b = await mount(
      t,
      backend: ProposalBackend()
        ..showProposal = true
        ..pendingResolve = Completer<String>(),
    );
    await t.tap(find.text('Agrupar las novedades'));
    await t.pumpAndSettle();
    await t.ensureVisible(find.text('Seguimiento'));
    await t.pumpAndSettle();
    expect(find.text('Propuesto: Sin definir'), findsOneWidget);
    await t.tap(find.text('Aplicar cambios'));
    await t.pump();
    await t.tap(find.text('Guardando…'));
    await t.pump();
    expect(b.requests.where((r) => r['action'] == 'resolve').length, 1);
    b.pendingResolve!.completeError(StateError('Conflicto de revisión'));
    await t.pumpAndSettle();
    expect(find.textContaining('Conflicto de revisión'), findsOneWidget);
    expect(find.text('Cambios propuestos'), findsOneWidget);
  });
  testWidgets('contexto antiguo y preguntas impiden aplicar', (t) async {
    final b = await mount(
      t,
      backend: ProposalBackend()
        ..showProposal = true
        ..current = false
        ..question = true,
    );
    await t.tap(find.text('Agrupar las novedades'));
    await t.pumpAndSettle();
    expect(
      t
          .widget<FilledButton>(
            find.widgetWithText(FilledButton, 'Aplicar cambios'),
          )
          .onPressed,
      isNull,
    );
    expect(find.textContaining('¿Qué carpeta'), findsOneWidget);
    await t.tap(find.text('Descartar'));
    await t.pumpAndSettle();
    expect(
      b.requests.where((r) => r['action'] == 'resolve').single['apply'],
      false,
    );
  });
  testWidgets(
    'el primer error vuelve al campo aunque se haya desplazado el diálogo',
    (t) async {
      final b = await mount(t, size: const Size(640, 420));
      await t.tap(find.text('Proponer mejoras'));
      await t.pumpAndSettle();
      await t.enterText(
        find.byType(TextFormField),
        List.filled(601, 'a').join(),
      );
      final remote = find.text('Valorar con la guía remota');
      await Scrollable.ensureVisible(t.element(remote), alignment: .5);
      await t.pumpAndSettle();
      await t.tap(find.text('Pedir propuestas'));
      await t.pumpAndSettle();
      expect(
        find.text('Escribe entre 1 y 600 caracteres.').hitTestable(),
        findsOneWidget,
      );
      final field = find.descendant(
        of: find.byType(Dialog),
        matching: find.byType(TextField),
      );
      expect(t.widget<TextField>(field).focusNode!.hasFocus, true);
      expect(b.requests.where((r) => r['action'] == 'begin'), isEmpty);
    },
  );
  for (final brightness in Brightness.values) {
    for (final (size, scale) in [
      (const Size(1100, 720), 1.0),
      (const Size(640, 420), 1.0),
      (const Size(640, 420), 1.5),
    ]) {
      testWidgets('propuestas ${brightness.name} ${size.width} texto $scale', (
        t,
      ) async {
        t.platformDispatcher.platformBrightnessTestValue = brightness;
        t.platformDispatcher.textScaleFactorTestValue = scale;
        addTearDown(t.platformDispatcher.clearPlatformBrightnessTestValue);
        addTearDown(t.platformDispatcher.clearTextScaleFactorTestValue);
        await mount(
          t,
          size: size,
          backend: ProposalBackend()
            ..showProposal = true
            ..failResolve = true,
        );
        await t.tap(find.text('Proponer mejoras'));
        await t.pumpAndSettle();
        expect(find.text('Pedir propuestas').hitTestable(), findsOneWidget);
        final name =
            '${const String.fromEnvironment('CAPTURE_PREFIX')}proposals-${brightness.name}-${size.width.toInt()}-$scale';
        final ratios = await measureContrast(
          t,
          {'pedir': find.text('Pedir propuestas')},
          backgroundMaterials: {
            'pedir': find
                .ancestor(
                  of: find.text('Pedir propuestas'),
                  matching: find.byType(Material),
                )
                .first,
          },
        );
        await capture(t, '$name-request');
        if (const bool.fromEnvironment('CAPTURE_UI')) {
          File(
            '../.verification/assistant-chat-real/$name-contrast.json',
          ).writeAsStringSync(jsonEncode(ratios));
        }
        await t.tap(find.text('Cancelar'));
        await t.pumpAndSettle();
        final proposal = find.text('Agrupar las novedades');
        await Scrollable.ensureVisible(t.element(proposal), alignment: .5);
        await t.pumpAndSettle();
        await t.tap(proposal);
        await t.pumpAndSettle();
        expect(find.text('Aplicar cambios').hitTestable(), findsOneWidget);
        await capture(t, '$name-review');
        await t.tap(find.text('Aplicar cambios'));
        await t.pumpAndSettle();
        final error = find.textContaining('Conflicto de revisión');
        await Scrollable.ensureVisible(t.element(error), alignment: .5);
        await t.pumpAndSettle();
        final errorContrast = await measureContrast(
          t,
          {'error': error, 'aplicar': find.text('Aplicar cambios')},
          backgroundMaterials: {
            'aplicar': find
                .ancestor(
                  of: find.text('Aplicar cambios'),
                  matching: find.byType(Material),
                )
                .first,
          },
        );
        await capture(t, '$name-error');
        if (const bool.fromEnvironment('CAPTURE_UI')) {
          File(
            '../.verification/assistant-chat-real/$name-error-contrast.json',
          ).writeAsStringSync(jsonEncode(errorContrast));
        }
        expect(t.takeException(), isNull);
      }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));
    }
  }
}
