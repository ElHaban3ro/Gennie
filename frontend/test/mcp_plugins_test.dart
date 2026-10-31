import 'dart:async';
import 'dart:convert';
import 'capture_output.dart';
import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/src/plugins.dart';
import 'plugins_test.dart' show PluginsBackend, mount;
import 'chats_preview_test.dart' show loadFonts;
import 'tool_evidence_test.dart' show measureContrast;

Map<String, dynamic> source() => {
  'specification': {
    'id': 'notes',
    'name': 'Notas compartidas',
    'connection': {
      'type': 'http',
      'url': 'https://example.com/mcp',
      'bearer_token_env_var': null,
    },
    'timeout_seconds': 60,
  },
  'revision': 1,
  'enabled': true,
  'connection_status': 'disconnected',
  'allowed_tools': ['echo'],
  'schema_status': {
    'echo': 'ready',
    'nested': 'ready',
    'unsupported': 'unsupported',
  },
  'tools': {
    'echo': {
      'name': 'echo',
      'description': 'Preparar un texto con el destinatario indicado.',
      'inputSchema': {
        'type': 'object',
        'properties': {
          'recipient': {
            'type': 'string',
            'title': 'Destinatario',
            'minLength': 1,
          },
          'message': {'type': 'string', 'title': 'Mensaje', 'minLength': 1},
        },
        'required': ['recipient', 'message'],
        'additionalProperties': false,
      },
    },
    'nested': {
      'name': 'nested',
      'description': 'Procesar datos estructurados.',
      'inputSchema': {
        'type': 'object',
        'properties': {
          'items': {
            'type': 'array',
            'items': {'type': 'integer'},
          },
        },
        'required': ['items'],
      },
    },
    'unsupported': {
      'name': 'unsupported',
      'inputSchema': {'\$ref': 'https://example.com/private'},
    },
  },
  'checked_at': '2026-10-29T12:00:00Z',
  'pending_attempts': <dynamic>[],
  'pending_count': 0,
  'pending_next_cursor': null,
  'recent_reviews': <dynamic>[],
};

Map<String, dynamic> pendingAttempt(
  int id, {
  String tool = 'echo',
  bool active = false,
}) => {
  'id': id,
  'source_id': 'notes',
  'source_revision': 1,
  'server_name': 'Notas compartidas',
  'tool': tool,
  'state': active ? 'running' : 'uncertain',
  'active': active,
  'reason': 'transport_error',
  'created_at': '2026-10-29T12:00:00Z',
  'resolution': null,
  'reviewed_at': null,
};

class McpBackend extends PluginsBackend {
  Map<String, dynamic> server = source();
  Completer<String>? pendingMcp;
  bool failSave = false, failReview = false;
  int nextAttempt = 1, activity = 0;
  String runStatus = 'ok';
  List<Map<String, dynamic>> get effects =>
      requests.where((r) => r['action'] == 'mcp_run').toList();
  Map<String, dynamic> presented() {
    final pending = server['pending_attempts'] as List;
    return {
      ...server,
      'pending_attempts': pending.take(50).toList(),
      'pending_next_cursor': pending.length > 50 ? pending[49]['id'] : null,
    };
  }

  @override
  Future<String> pluginRequest(String text) async {
    final request = Map<String, dynamic>.from(jsonDecode(text) as Map);
    if (request['action'] == 'list') {
      final result = Map<String, dynamic>.from(
        jsonDecode(await super.pluginRequest(text)) as Map,
      );
      result['mcp_sources'] = [
        {
          'id': 'notes',
          'name': server['specification']['name'],
          'enabled': server['enabled'],
          'revision': server['revision'],
          'tool_count': 3,
          'allowed_count': (server['allowed_tools'] as List).length,
          'pending_count': (server['pending_attempts'] as List).length,
          'activity_revision': '$activity',
        },
      ];
      return jsonEncode(result);
    }
    if (!(request['action'] as String).startsWith('mcp_')) {
      return super.pluginRequest(text);
    }
    requests.add(request);
    switch (request['action']) {
      case 'mcp_get':
        return jsonEncode({'server': presented()});
      case 'mcp_save':
        if (failSave) {
          throw StateError('La conexión cambió. El borrador se conserva.');
        }
        server = {
          ...server,
          'specification': request['specification'],
          'enabled': false,
          'allowed_tools': [],
          'tools': {},
          'schema_status': {},
          'revision': 2,
        };
        return jsonEncode({'server': server});
      case 'mcp_connect':
        server['connection_status'] = 'connected';
        return jsonEncode({'server': server});
      case 'mcp_permissions':
        server = {
          ...server,
          'enabled': request['enabled'],
          'allowed_tools': request['allowed_tools'],
          'revision': (server['revision'] as int) + 1,
        };
        return jsonEncode({'server': server});
      case 'mcp_run':
        final response = Map<String, dynamic>.from(
          jsonDecode(
                await (pendingMcp?.future ??
                    Future.value(
                      jsonEncode({
                        'status': runStatus,
                        'executed': true,
                        'result': {
                          'content': [
                            {'type': 'text', 'text': 'Resultado ficticio'},
                          ],
                        },
                      }),
                    )),
              )
              as Map,
        );
        if (response['status'] == 'result_uncertain' ||
            response['review_required'] == true) {
          final attempt = pendingAttempt(
            nextAttempt++,
            tool: request['tool'] as String,
          );
          (server['pending_attempts'] as List).add(attempt);
          server['pending_count'] = (server['pending_attempts'] as List).length;
          response['attempt'] = attempt;
          activity++;
        }
        return jsonEncode(response);
      case 'mcp_reconcile':
        if (failReview) {
          throw StateError('La revisión cambió. Recarga la ficha.');
        }
        if (request['expected_revision'] != server['revision']) {
          throw StateError('Revisión obsoleta');
        }
        final attempt = (server['pending_attempts'] as List).singleWhere(
          (a) => a['id'] == request['attempt_id'],
        );
        if (attempt['active'] == true) {
          throw StateError('La llamada sigue en curso');
        }
        (server['pending_attempts'] as List).remove(attempt);
        (server['recent_reviews'] as List).insert(0, {
          ...attempt as Map,
          'resolution': request['resolution'],
          'state': 'reviewed',
        });
        server['pending_count'] = (server['pending_attempts'] as List).length;
        server['revision'] = (server['revision'] as int) + 1;
        activity++;
        return jsonEncode({'server': presented()});
      case 'mcp_attempts':
        final rows = (server['pending_attempts'] as List)
            .where((a) => a['id'] > (request['after'] ?? 0))
            .take(51)
            .toList();
        return jsonEncode({
          'attempts': rows.take(50).toList(),
          'total': server['pending_count'],
          'next_cursor': rows.length > 50 ? rows[49]['id'] : null,
        });
      case 'mcp_disconnect':
        return jsonEncode({'status': 'disconnected'});
      case 'mcp_delete':
        return jsonEncode({'status': 'deleted'});
    }
    throw StateError('Petición inesperada');
  }
}

Future<void> click(WidgetTester t, String text) async {
  await t.ensureVisible(find.text(text).last);
  await t.tap(find.text(text).last);
  await t.pumpAndSettle();
}

Future<void> open(WidgetTester t) async => click(t, 'Notas compartidas');
Future<void> tool(WidgetTester t, String name) async {
  final target = find.byKey(const ValueKey('mcp-tool'));
  await t.ensureVisible(target);
  await t.tap(target);
  await t.pumpAndSettle();
  await t.tap(find.text(name).last);
  await t.pumpAndSettle();
}

Future<void> enter(WidgetTester t, String key, String value) async {
  final target = find.byKey(ValueKey(key));
  await t.ensureVisible(target);
  await t.enterText(target, value);
  await t.pumpAndSettle();
}

Future<void> capture(WidgetTester t, GlobalKey boundary, String name) async {
  if (!const bool.fromEnvironment('CAPTURE_UI')) return;
  await t.runAsync(() async {
    final render =
        boundary.currentContext!.findRenderObject()! as RenderRepaintBoundary;
    final image = await render.toImage();
    final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
    captureOutput(
      '../.verification/assistant-chat-real',
      '$name.png',
    ).writeAsBytesSync(bytes!.buffer.asUint8List());
    image.dispose();
  });
}

void main() {
  testWidgets('editar MCP se activa desde el botón accesible nombrado', (
    t,
  ) async {
    final semantics = t.ensureSemantics();
    try {
      final b = McpBackend();
      await mount(t, b);
      await open(t);
      final node = t.getSemantics(
        find.bySemanticsLabel('Editar conexión MCP').first,
      );
      final data = node.getSemanticsData();
      expect(data.flagsCollection.isButton, isTrue);
      expect(data.hasAction(ui.SemanticsAction.tap), isTrue);
      node.owner!.performAction(node.id, ui.SemanticsAction.tap);
      await t.pumpAndSettle();
      expect(find.byKey(const ValueKey('mcp-name')), findsOneWidget);
      expect(b.requests.where((r) => r['action'] == 'mcp_save'), isEmpty);
      expect(b.requests.where((r) => r['action'] == 'mcp_connect'), isEmpty);
      expect(t.takeException(), isNull);
    } finally {
      semantics.dispose();
    }
  }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));

  testWidgets('recargar Plugins tiene nombre y acción en el mismo nodo', (
    t,
  ) async {
    final semantics = t.ensureSemantics();
    try {
      final b = McpBackend();
      await mount(t, b);
      final before = b.requests.where((r) => r['action'] == 'list').length;
      final node = t.getSemantics(
        find.bySemanticsLabel('Recargar plugins').first,
      );
      expect(node.getSemanticsData().flagsCollection.isButton, isTrue);
      expect(node.getSemanticsData().hasAction(ui.SemanticsAction.tap), isTrue);
      node.owner!.performAction(node.id, ui.SemanticsAction.tap);
      await t.pumpAndSettle();
      expect(b.requests.where((r) => r['action'] == 'list').length, before + 1);
      expect(b.effects, isEmpty);
      expect(t.takeException(), isNull);
    } finally {
      semantics.dispose();
    }
  }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));

  test(
    'los esquemas complejos y las restricciones sin control equivalente conservan entrada JSON',
    () {
      expect(
        mcpSimpleParameters({
          'type': 'object',
          'properties': {
            'id': {'type': 'string', 'pattern': '^[a-z]+\$'},
          },
        }),
        isNull,
      );
      expect(
        mcpSimpleParameters(source()['tools']['nested']['inputSchema'] as Map),
        isNull,
      );
      final params = mcpSimpleParameters(
        source()['tools']['echo']['inputSchema'] as Map,
      )!;
      expect(params['recipient']['required'], true);
      expect(params['recipient']['label'], 'Destinatario');
    },
  );
  testWidgets(
    'crear conexión es accesible desde Nuevo plugin sin iniciar procesos',
    (t) async {
      final b = McpBackend();
      await mount(t, b);
      await t.tap(find.byIcon(Icons.add).last);
      await t.pumpAndSettle();
      await click(t, 'Conectar un servidor MCP existente');
      await enter(t, 'mcp-name', 'Nueva conexión');
      await enter(t, 'mcp-url', 'https://example.com/mcp');
      await click(t, 'Guardar conexión');
      final save = b.requests.singleWhere((r) => r['action'] == 'mcp_save');
      expect(save['expected_revision'], 0);
      expect(save['specification']['name'], 'Nueva conexión');
      expect(
        b.requests.where(
          (r) => [
            'mcp_connect',
            'mcp_run',
            'mcp_permissions',
          ].contains(r['action']),
        ),
        isEmpty,
      );
      expect(t.takeException(), isNull);
    },
  );
  testWidgets(
    'stdio conserva argumentos separados y enfoca un plazo inválido aunque esté plegado',
    (t) async {
      final b = McpBackend();
      await mount(t, b);
      await t.tap(find.byIcon(Icons.add).last);
      await t.pumpAndSettle();
      await click(t, 'Conectar un servidor MCP existente');
      final type = find.byKey(const ValueKey('mcp-transport'));
      await t.ensureVisible(type);
      await t.tap(type);
      await t.pumpAndSettle();
      await t.tap(find.text('Programa local · stdio').last);
      await t.pumpAndSettle();
      await enter(t, 'mcp-name', 'Servidor local');
      await enter(t, 'mcp-command', '/usr/local/bin/example');
      await enter(t, 'mcp-args', '["--label", "dos palabras"]');
      await click(t, 'Opciones avanzadas');
      await enter(t, 'mcp-timeout', '721');
      await click(t, 'Opciones avanzadas');
      await click(t, 'Guardar conexión');
      expect(
        find.text('Elige entre 1 y 720 segundos.').hitTestable(),
        findsOneWidget,
      );
      final field = t.widget<TextField>(
        find.descendant(
          of: find.byKey(const ValueKey('mcp-timeout')),
          matching: find.byType(TextField),
        ),
      );
      expect(field.focusNode!.hasFocus, isTrue);
      expect(b.requests.where((r) => r['action'] == 'mcp_save'), isEmpty);
      await enter(t, 'mcp-timeout', '60');
      await enter(t, 'mcp-env', 'EXAMPLE_TOKEN\nEXAMPLE_ACCOUNT');
      await click(t, 'Guardar conexión');
      final request = b.requests.singleWhere((r) => r['action'] == 'mcp_save');
      expect(request['specification']['connection']['args'], [
        '--label',
        'dos palabras',
      ]);
      expect(request['specification']['connection']['env_vars'], [
        'EXAMPLE_TOKEN',
        'EXAMPLE_ACCOUNT',
      ]);
      expect(b.requests.where((r) => r['action'] == 'mcp_connect'), isEmpty);
      expect(t.takeException(), isNull);
    },
  );
  testWidgets(
    'MCP sigue visible con catálogo web no disponible y descubrir no ejecuta',
    (t) async {
      final b = McpBackend()
        ..catalogueError = 'Navegador desconectado'
        ..catalogueStatus = 'unavailable';
      await mount(t, b);
      await open(t);
      expect(
        find.text(
          'No se pudo actualizar el catálogo web: Navegador desconectado',
        ),
        findsOneWidget,
      );
      await click(t, 'Descubrir herramientas');
      expect(b.effects, isEmpty);
      expect(
        b.requests.where((r) => r['action'] == 'mcp_permissions'),
        isEmpty,
      );
      expect(find.text('Conectado'), findsOneWidget);
      expect(t.takeException(), isNull);
    },
  );
  testWidgets(
    'editar valida junto al campo, conserva borrador al fallar y guardar no conecta',
    (t) async {
      final b = McpBackend()..failSave = true;
      await mount(t, b);
      await open(t);
      await t.tap(find.byIcon(Icons.edit_outlined).last);
      await t.pumpAndSettle();
      await enter(t, 'mcp-url', 'https://user:secret@example.com/mcp');
      await click(t, 'Guardar conexión');
      expect(find.textContaining('Usa una dirección admitida'), findsOneWidget);
      expect(b.requests.where((r) => r['action'] == 'mcp_save'), isEmpty);
      await enter(t, 'mcp-url', 'https://example.com/new');
      await enter(t, 'mcp-name', 'Mis notas');
      await click(t, 'Guardar conexión');
      expect(find.textContaining('El borrador se conserva.'), findsOneWidget);
      expect(
        t
            .widget<TextFormField>(find.byKey(const ValueKey('mcp-name')))
            .controller!
            .text,
        'Mis notas',
      );
      b.failSave = false;
      await click(t, 'Guardar conexión');
      expect(b.server['enabled'], false);
      expect(b.server['allowed_tools'], isEmpty);
      expect(b.requests.where((r) => r['action'] == 'mcp_connect'), isEmpty);
      expect(t.takeException(), isNull);
    },
  );
  testWidgets(
    'permisos son explícitos y esquema no compatible no se puede autorizar',
    (t) async {
      final b = McpBackend();
      b.server['enabled'] = false;
      b.server['allowed_tools'] = [];
      await mount(t, b);
      await open(t);
      final unsupported = find.widgetWithText(CheckboxListTile, 'unsupported');
      await t.ensureVisible(unsupported);
      expect(t.widget<CheckboxListTile>(unsupported).onChanged, isNull);
      await t.ensureVisible(find.text('echo').first);
      await t.tap(find.text('echo').first);
      await t.pumpAndSettle();
      expect(
        b.requests.where((r) => r['action'] == 'mcp_permissions'),
        isEmpty,
      );
      await t.ensureVisible(find.text('Habilitar este plugin'));
      await t.tap(find.text('Habilitar este plugin'));
      await t.pumpAndSettle();
      await click(t, 'Guardar permisos');
      final request = b.requests.singleWhere(
        (r) => r['action'] == 'mcp_permissions',
      );
      expect(request['allowed_tools'], ['echo']);
      expect(request['enabled'], true);
      expect(request['expected_revision'], 1);
      expect(b.effects, isEmpty);
      expect(t.takeException(), isNull);
    },
  );
  testWidgets(
    'campos requeridos, envío único y selección bloqueada mientras espera',
    (t) async {
      final b = McpBackend();
      await mount(t, b);
      await open(t);
      await tool(t, 'echo');
      await click(t, 'Ejecutar función');
      expect(b.effects, isEmpty);
      expect(find.text('Completa Destinatario.'), findsOneWidget);
      await enter(t, 'plugin-param-recipient', 'Cata');
      await enter(t, 'plugin-param-message', 'Llegaré a las 8.');
      b.pendingMcp = Completer<String>();
      await t.ensureVisible(find.text('Ejecutar función'));
      await t.tap(find.text('Ejecutar función'));
      await t.pump();
      expect(b.effects, hasLength(1));
      expect(b.effects.single['arguments'], {
        'recipient': 'Cata',
        'message': 'Llegaré a las 8.',
      });
      expect(
        t
            .widget<DropdownButtonFormField<String>>(
              find.byKey(const ValueKey('mcp-tool')),
            )
            .onChanged,
        isNull,
      );
      expect(
        t
            .widget<FilledButton>(
              find.widgetWithText(FilledButton, 'Ejecutando…'),
            )
            .onPressed,
        isNull,
      );
      b.pendingMcp!.complete(jsonEncode({'status': 'ok', 'executed': true}));
      await t.pumpAndSettle();
      expect(t.takeException(), isNull);
    },
  );
  testWidgets(
    'resultado incierto bloquea ratón y teclado, con borrador conservado',
    (t) async {
      final b = McpBackend()..runStatus = 'result_uncertain';
      await mount(t, b);
      await open(t);
      await tool(t, 'echo');
      await enter(t, 'plugin-param-recipient', 'Cata');
      await enter(t, 'plugin-param-message', 'Hola');
      await click(t, 'Ejecutar función');
      expect(b.effects, hasLength(1));
      expect(
        t
            .widget<FilledButton>(
              find.widgetWithText(FilledButton, 'Ejecutar función'),
            )
            .onPressed,
        isNull,
      );
      await t.sendKeyEvent(LogicalKeyboardKey.enter);
      await t.pumpAndSettle();
      expect(b.effects, hasLength(1));
      // Enter puede abrir el selector que recuperó el foco al deshabilitarse el envío.
      await t.sendKeyEvent(LogicalKeyboardKey.escape);
      await t.pumpAndSettle();
      await click(t, 'Revisar resultado');
      expect(
        t
            .widget<FilledButton>(
              find.widgetWithText(FilledButton, 'Guardar revisión'),
            )
            .onPressed,
        isNull,
      );
      await click(t, 'Comprobé que no se aplicó');
      await click(t, 'Guardar revisión');
      expect(b.server['pending_count'], 0);
      expect(b.effects, hasLength(1));
      expect(
        t
            .widget<TextFormField>(
              find.byKey(const ValueKey('plugin-param-message')),
            )
            .controller!
            .text,
        'Hola',
      );
      expect(
        t
            .widget<FilledButton>(
              find.widgetWithText(FilledButton, 'Ejecutar función'),
            )
            .onPressed,
        isNotNull,
      );
      expect(b.effects, hasLength(1));
      expect(t.takeException(), isNull);
    },
  );
  testWidgets(
    'JSON complejo rechaza formato inválido y conserva datos al rechazar esquema',
    (t) async {
      final b = McpBackend()..runStatus = 'invalid_arguments';
      (b.server['allowed_tools'] as List).add('nested');
      await mount(t, b);
      await open(t);
      await tool(t, 'nested');
      await enter(t, 'mcp-arguments', '[]');
      await click(t, 'Ejecutar herramienta');
      expect(b.effects, isEmpty);
      expect(find.text('Introduce un objeto JSON válido.'), findsOneWidget);
      await enter(t, 'mcp-arguments', '{"items":[1,2]}');
      await click(t, 'Ejecutar herramienta');
      expect(b.effects.single['arguments'], {
        'items': [1, 2],
      });
      expect(
        t
            .widget<TextFormField>(find.byKey(const ValueKey('mcp-arguments')))
            .controller!
            .text,
        '{"items":[1,2]}',
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
      testWidgets('MCP ${brightness.name} ${size.width} texto $scale', (
        t,
      ) async {
        t.platformDispatcher.textScaleFactorTestValue = scale;
        addTearDown(t.platformDispatcher.clearTextScaleFactorTestValue);
        await loadFonts();
        final boundary = GlobalKey();
        final b = McpBackend();
        final name = 'mcp-ui-${brightness.name}-${size.width.toInt()}-$scale';
        await mount(
          t,
          b,
          size: size,
          brightness: brightness,
          boundary: boundary,
        );
        await open(t);
        await capture(t, boundary, '$name-profile');
        expect(t.takeException(), isNull);
        await tool(t, 'echo');
        await click(t, 'Ejecutar función');
        final ratios = await measureContrast(t, {
          'error': find.text('Completa Destinatario.'),
        }, captureBoundary: find.byKey(boundary));
        await capture(t, boundary, '$name-error');
        await t.ensureVisible(find.byIcon(Icons.edit_outlined).last);
        await t.tap(find.byIcon(Icons.edit_outlined).last);
        await t.pumpAndSettle();
        await capture(t, boundary, '$name-editor');
        await t.ensureVisible(find.text('Guardar conexión'));
        await t.pumpAndSettle();
        ratios.addAll(
          await measureContrast(t, {
            'guardar': find.text('Guardar conexión'),
          }, captureBoundary: find.byKey(boundary)),
        );
        if (const bool.fromEnvironment('CAPTURE_UI')) {
          captureOutput(
            '../.verification/assistant-chat-real',
            '$name-contrast.json',
          ).writeAsStringSync(jsonEncode(ratios));
        }
        expect(t.takeException(), isNull);
      });
    }
  }
}
