import 'dart:convert';
import 'dart:io';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/main.dart';
import 'package:gennie/src/backend.dart';
import 'widget_test.dart' as fixture;
import 'assignment_proposals_test.dart' show plan;
import 'chats_preview_test.dart' show loadFonts;
import 'tool_evidence_test.dart' show capture, measureContrast;

class ExecutionFixture extends fixture.FakeBackend
    implements ExecutionBackend, PluginBackend {
  ExecutionFixture({bool preparationOnly = false, bool followUp = true}) {
    document = plan()
      ..addAll({
        'method': {
          'plugin_id': 'mcp:fixture',
          'functions': ['read'],
        },
        'authorization': {
          'scope': 'Consultar únicamente novedades',
          'allowed_routes': ['mcp'],
        },
      });
    if (preparationOnly) document.remove('recurrence');
    if (!followUp) document.remove('follow_up');
    value = fixture.snapshot(
      plans: [
        AssignmentRecord(
          id: '1',
          instanceId: 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
          revision: 3,
          createdAt: '',
          updatedAt: '',
          document: jsonEncode(document),
        ),
      ],
    );
  }
  late Map<String, dynamic> document;
  final requests = <Map<String, dynamic>>[];
  bool existing = false, fail = false, active = true, botAvailable = true;
  bool contextual = false;
  String runState = 'uncertain';
  Map<String, dynamic>? notification;
  int pages = 0;
  Map<String, dynamic> policy() => {
    'id': 'policy-fixture',
    'revision': 2,
    'enabled': true,
    'state': 'active',
    'current': true,
    'running': false,
    'next_at': '2026-10-30T13:00:00Z',
    'configuration': {
      'trigger': contextual ? 'preparation' : 'delivery',
      'contextual_notice': contextual,
      if (contextual)
        'delivery': {
          'conversation_id': 3,
          'bot_id': 'gennie',
          'epoch': 1,
          'provider_thread': 'origin',
        },
      'assignment': {'document': document},
      'target': {
        'kind': 'mcp',
        'server_id': 'fixture',
        'tool': 'read',
        'revision': 1,
      },
      'calendar': {},
      'arguments': {'query': 'actual'},
      'misfire': 'skip_missed',
      'max_lateness_minutes': 5,
    },
  };
  @override
  Future<String> pluginRequest(String text) async => jsonEncode({
    'server': {
      'revision': 2,
      'tools': {
        'read': {
          'description': 'Consulta sin modificar mensajes',
          'inputSchema': {
            'type': 'object',
            'properties': {
              'query': {'type': 'string', 'title': 'Consulta'},
            },
            'required': ['query'],
          },
        },
      },
    },
  });
  @override
  Future<String> executionRequest(String text) async {
    final r = Map<String, dynamic>.from(jsonDecode(text) as Map);
    requests.add(r);
    switch (r['action']) {
      case 'notice_origin':
        if (!botAvailable) {
          throw StateError('Inicia una conversación con Gennie');
        }
        return jsonEncode({
          'conversation_id': 3,
          'bot_id': 'gennie',
          'epoch': 1,
          'provider_thread': 'origin',
        });
      case 'list':
        return jsonEncode({
          'runner_active': active,
          'storage_failed': false,
          'policies': existing ? [policy()] : [],
        });
      case 'configure':
        if (fail) throw StateError('El encargo cambió');
        return '{}';
      case 'revoke_guide':
        contextual = false;
        return '{}';
      case 'runs':
        pages++;
        return jsonEncode({
          'runs': pages == 1
              ? [
                  {
                    'id': 77,
                    'scheduled_at': '2026-10-29T13:00:00Z',
                    'state': runState,
                  },
                ]
              : [],
          'next_cursor': pages == 1 ? 77 : null,
        });
      case 'run_details':
        return jsonEncode({
          'id': 77,
          'state': runState,
          'notification': notification,
          'scheduled_at': '2026-10-29T13:00:00Z',
          'configuration': {
            ...policy()['configuration'] as Map,
            'target': {
              'kind': 'mcp',
              'server_id': 'original',
              'tool': 'original_read',
              'revision': 1,
            },
            'arguments': {'query': 'original'},
          },
          'output': contextual
              ? {
                  'changes': ['Una revisión nueva'],
                }
              : {'mcp_attempt_id': 9},
        });
      default:
        return '{}';
    }
  }
}

Future<ExecutionFixture> mount(
  WidgetTester t, {
  ExecutionFixture? backend,
  Size size = const Size(1100, 720),
}) async {
  final b = backend ?? ExecutionFixture();
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

Future<void> press(WidgetTester t, String text) async {
  final f = find.text(text).last;
  await Scrollable.ensureVisible(t.element(f), alignment: .5);
  await t.pumpAndSettle();
  await t.tap(f);
  await t.pumpAndSettle();
}

void main() {
  setUpAll(loadFonts);
  testWidgets(
    'solo preparación, activación desmarcada y parámetros obligatorios',
    (t) async {
      final b = await mount(
        t,
        backend: ExecutionFixture(preparationOnly: true),
      );
      await press(t, 'Configurar ejecución');
      expect(t.takeException(), isNull);
      expect(find.text('Cadencia de preparación'), findsOneWidget);
      expect(
        t
            .widget<SwitchListTile>(
              find.widgetWithText(
                SwitchListTile,
                'Habilitar ejecución periódica',
              ),
            )
            .value,
        false,
      );
      await press(t, 'Guardar pausada');
      expect(b.requests.where((r) => r['action'] == 'configure'), isEmpty);
      expect(find.text('Completa Consulta.').hitTestable(), findsOneWidget);
      expect(
        t
            .widget<EditableText>(
              find.descendant(
                of: find.widgetWithText(TextFormField, 'Consulta'),
                matching: find.byType(EditableText),
              ),
            )
            .focusNode
            .hasFocus,
        isTrue,
      );
      await t.enterText(
        find.widgetWithText(TextFormField, 'Consulta'),
        'novedades',
      );
      await press(t, 'Guardar pausada');
      final config = b.requests.singleWhere(
        (r) => r['action'] == 'configure',
      )['configuration'];
      expect(config['enabled'], false);
      expect(config['trigger'], 'preparation');
      expect(config['arguments'], {'query': 'novedades'});
      expect(config['assignment'], {
        'id': 1,
        'instance_id': 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
        'revision': 3,
      });
      expect(config['target']['revision'], 2);
      expect(t.takeException(), isNull);
    },
  );
  testWidgets(
    'segunda cadencia no sobrescribe existente; conflicto conserva parámetros',
    (t) async {
      final b = await mount(
        t,
        backend: ExecutionFixture()
          ..existing = true
          ..fail = true,
      );
      await press(t, 'Configurar ejecución');
      expect(find.text('Cadencia de preparación'), findsOneWidget);
      await t.enterText(
        find.widgetWithText(TextFormField, 'Consulta'),
        'mi borrador',
      );
      await press(t, 'Guardar pausada');
      expect(
        find.textContaining('El encargo cambió').hitTestable(),
        findsOneWidget,
      );
      expect(
        t
            .widget<TextFormField>(
              find.widgetWithText(TextFormField, 'Consulta'),
            )
            .controller!
            .text,
        'mi borrador',
      );
      expect(b.requests.last['configuration']['expected_policy_revision'], 0);
      expect(t.takeException(), isNull);
    },
  );
  testWidgets(
    'revisión usa versión original sin elección automática; paginación y pausa CAS',
    (t) async {
      final b = await mount(t, backend: ExecutionFixture()..existing = true);
      await press(t, 'Pausar ejecuciones');
      expect(
        b.requests.singleWhere(
          (r) => r['action'] == 'pause',
        )['expected_revision'],
        2,
      );
      await press(t, 'Ver ejecuciones');
      await press(t, 'Cargar ejecuciones anteriores');
      expect(b.requests.where((r) => r['action'] == 'runs').last['before'], 77);
      await press(t, 'Revisar resultado');
      expect(find.textContaining('original_read'), findsOneWidget);
      expect(
        t
            .widget<FilledButton>(
              find.widgetWithText(FilledButton, 'Guardar comprobación'),
            )
            .onPressed,
        isNull,
      );
      await press(t, 'La acción se aplicó');
      await press(t, 'Guardar comprobación');
      expect(b.requests.singleWhere((r) => r['action'] == 'reconcile'), {
        'action': 'reconcile',
        'run_id': 77,
        'resolution': 'applied',
      });
      expect(t.takeException(), isNull);
    },
  );
  testWidgets('avisos desmarcados y origen fijado solo por selección humana', (
    t,
  ) async {
    final b = await mount(t, backend: ExecutionFixture(preparationOnly: true));
    await press(t, 'Configurar ejecución');
    final toggle = find.widgetWithText(
      SwitchListTile,
      'Recibir resultados en el chat de Gennie',
    );
    expect(t.widget<SwitchListTile>(toggle).value, false);
    await Scrollable.ensureVisible(t.element(toggle), alignment: .5);
    await t.pumpAndSettle();
    await t.tap(toggle);
    await t.pumpAndSettle();
    await t.enterText(
      find.widgetWithText(TextFormField, 'Consulta'),
      'novedades',
    );
    await press(t, 'Guardar pausada');
    final saved = b.requests.singleWhere(
      (r) => r['action'] == 'configure',
    )['configuration'];
    expect(saved['delivery'], {
      'conversation_id': 3,
      'bot_id': 'gennie',
      'epoch': 1,
      'provider_thread': 'origin',
    });
    expect(saved['enabled'], false);
    expect(t.takeException(), isNull);
  });
  testWidgets(
    'falta de bot permite guardar silenciosa y explica avisos deshabilitados',
    (t) async {
      final b = await mount(
        t,
        backend: ExecutionFixture(preparationOnly: true)..botAvailable = false,
      );
      await press(t, 'Configurar ejecución');
      final toggle = t.widget<SwitchListTile>(
        find.widgetWithText(
          SwitchListTile,
          'Recibir resultados en el chat de Gennie',
        ),
      );
      expect(toggle.value, false);
      expect(toggle.onChanged, isNull);
      await t.enterText(
        find.widgetWithText(TextFormField, 'Consulta'),
        'novedades',
      );
      await press(t, 'Guardar pausada');
      expect(
        b.requests.singleWhere(
          (r) => r['action'] == 'configure',
        )['configuration']['delivery'],
        isNull,
      );
    },
  );
  for (final brightness in Brightness.values) {
    for (final (size, scale) in [
      (const Size(1100, 720), 1.0),
      (const Size(640, 420), 1.0),
      (const Size(640, 420), 1.5),
    ]) {
      testWidgets('ejecución ${brightness.name} ${size.width} texto $scale', (
        t,
      ) async {
        t.platformDispatcher.platformBrightnessTestValue = brightness;
        t.platformDispatcher.textScaleFactorTestValue = scale;
        addTearDown(t.platformDispatcher.clearPlatformBrightnessTestValue);
        addTearDown(t.platformDispatcher.clearTextScaleFactorTestValue);
        await mount(
          t,
          size: size,
          backend: ExecutionFixture(preparationOnly: true)..fail = true,
        );
        await press(t, 'Configurar ejecución');
        final name =
            'execution-${brightness.name}-${size.width.toInt()}-$scale';
        expect(find.text('Guardar pausada').hitTestable(), findsOneWidget);
        await capture(t, '$name-configure');
        final ratios = await measureContrast(
          t,
          {
            'guardar': find.text('Guardar pausada'),
            'cancelar': find.text('Cancelar'),
          },
          backgroundMaterials: {
            'guardar': find
                .ancestor(
                  of: find.text('Guardar pausada'),
                  matching: find.byType(Material),
                )
                .first,
          },
        );
        await press(t, 'Guardar pausada');
        expect(find.text('Completa Consulta.').hitTestable(), findsOneWidget);
        await t.enterText(
          find.widgetWithText(TextFormField, 'Consulta'),
          'novedades',
        );
        await press(t, 'Guardar pausada');
        final error = find.textContaining('El encargo cambió');
        expect(error.hitTestable(), findsOneWidget);
        ratios.addAll(await measureContrast(t, {'error': error}));
        await capture(t, '$name-error');
        if (const bool.fromEnvironment('CAPTURE_UI')) {
          File(
            '../.verification/assistant-chat-real/$name-contrast.json',
          ).writeAsStringSync(jsonEncode(ratios));
        }
        expect(t.takeException(), isNull);
      }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));
    }
  }
}
