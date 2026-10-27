import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:ui' show ImageByteFormat, PointerDeviceKind;
import 'package:flutter/material.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/src/backend.dart';
import 'package:gennie/src/assignment_logo.dart';
import 'package:gennie/src/sidebar.dart';
import 'package:gennie/src/assignments.dart';
import 'package:gennie/src/plugins.dart';
import 'package:gennie/main.dart';
import 'widget_test.dart' show FakeBackend, rail, snapshot;

Map<String, dynamic> record() => {
  'manifest': {
    'id': 'gmail_test',
    'name': 'Correo de prueba',
    'origin': 'https://example.com',
    'description': 'Consulta la bandeja de entrada de la cuenta conectada.',
    'capabilities': ['Listar mensajes recientes', 'Consultar asuntos'],
    'limitations': ['Solo lectura', 'No envía ni elimina correos'],
    'functions': [
      {
        'name': 'list_messages',
        'description': 'Recuperar la lista de mensajes',
        'parameters': {'limit': 'integer'},
        'transport': 'http',
        'path': '/api/messages',
        'result_type': 'array',
      },
    ],
  },
  'revision': 1,
  'verified': {
    'list_messages': {'revision': 1, 'verified_at': '2026-10-26T12:00:00Z'},
  },
};

class PluginsBackend extends FakeBackend implements PluginBackend {
  final requests = <Map<String, dynamic>>[];
  List<Map<String, dynamic>> records = [record()], jobs = [];
  Completer<String>? pending;
  String status = 'ok';
  bool conflict = false;
  String? catalogueStatus;
  String? catalogueError;
  @override
  Future<String> pluginRequest(String text) async {
    final request = Map<String, dynamic>.from(jsonDecode(text) as Map);
    requests.add(request);
    switch (request['action']) {
      case 'list':
        return jsonEncode({
          'plugins': catalogueError == null ? records : null,
          'jobs': jobs,
          'catalogue_status': catalogueStatus ?? 'ready',
          'catalogue_error': catalogueError,
        });
      case 'run':
      case 'verify':
        return pending?.future ??
            jsonEncode({
              'status': status,
              'data': [
                {'id': 'mail-1', 'subject': 'Correo recibido'},
              ],
            });
      case 'build':
        final input = request['request'] as Map;
        jobs.insert(0, {
          'id': 11,
          'plugin_id': input['plugin_id'],
          'origin': input['origin'],
          'objective': input['objective'],
          'trigger_conversation_id': request['trigger_conversation_id'],
          'state': 'queued',
          'summary': 'En cola',
          'attempts': 0,
          'max_attempts': input['max_attempts'],
          'max_seconds': input['max_seconds'],
        });
        return jsonEncode({'status': 'queued'});
      case 'cancel':
        for (final job in jobs) {
          if (job['id'] == request['job_id']) {
            job['state'] = 'cancelled';
            job['summary'] = 'Construcción cancelada.';
          }
        }
        return jsonEncode({'status': 'cancelled'});
      case 'connect':
        return jsonEncode({'status': 'opened'});
      case 'save':
        if (conflict) return jsonEncode({'status': 'revision_conflict'});
        records = [
          {
            'manifest': request['manifest'],
            'revision': 2,
            'verified': <String, dynamic>{},
          },
        ];
        return jsonEncode({'status': 'saved'});
      case 'delete':
        records = [];
        return jsonEncode({'status': 'deleted'});
      default:
        return jsonEncode({'status': 'recorded'});
    }
  }
}

Future<DesktopState> mount(
  WidgetTester tester,
  PluginsBackend backend, {
  Size size = const Size(1100, 720),
  Brightness brightness = Brightness.light,
  GlobalKey? boundary,
}) async {
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  final state = DesktopState(backend);
  await state.start();
  tester.platformDispatcher.platformBrightnessTestValue = brightness;
  addTearDown(tester.platformDispatcher.clearPlatformBrightnessTestValue);
  await tester.pumpWidget(
    RepaintBoundary(
      key: boundary,
      child: GennieApp(state: state),
    ),
  );
  await tester.pumpAndSettle();
  await tester.tap(rail('Plugins'));
  await tester.pumpAndSettle();
  return state;
}

Future<void> select(WidgetTester tester) async {
  await tester.tap(find.text('Correo de prueba').first);
  await tester.pumpAndSettle();
}

void main() {
  for (final brightness in Brightness.values) {
    for (final width in [640.0, 1100.0, 1600.0]) {
      testWidgets(
        'las fichas comparten márgenes a $width px en ${brightness.name}',
        (tester) async {
          final backend = PluginsBackend()
            ..value = snapshot(
              plans: [
                AssignmentRecord(
                  id: 'layout',
                  revision: 1,
                  createdAt: '',
                  updatedAt: '',
                  document: jsonEncode({
                    'title': 'Encargo de geometría',
                    'description': 'Descripción',
                    'kind': 'goal',
                    'status': 'active',
                    'priority': 'normal',
                  }),
                ),
              ],
            );
          await mount(
            tester,
            backend,
            size: Size(width, 720),
            brightness: brightness,
          );
          await select(tester);
          final pluginIcon = find
              .descendant(
                of: find.byType(PluginProfile),
                matching: find.byType(AssignmentLogo),
              )
              .first;
          final pluginRect = tester.getRect(pluginIcon);
          final pluginEdit = tester.getRect(
            find.byKey(const ValueKey('plugin-edit')),
          );
          await tester.tap(rail('Encargos'));
          await tester.pumpAndSettle();
          await tester.tap(find.text('Encargo de geometría').first);
          await tester.pumpAndSettle();
          final assignmentIcon = find
              .descendant(
                of: find.byType(AssignmentProfile),
                matching: find.byType(AssignmentIcon),
              )
              .first;
          expect(pluginRect.left, tester.getRect(assignmentIcon).left);
          expect(pluginRect.width, tester.getRect(assignmentIcon).width);
          expect(
            pluginEdit,
            tester.getRect(find.byKey(const ValueKey('assignment-edit'))),
          );
          expect(tester.takeException(), isNull);
        },
      );
    }
  }
  for (final size in [const Size(1100, 720), const Size(640, 420)]) {
    testWidgets('el vacío de Plugins se centra verticalmente a $size', (
      tester,
    ) async {
      await mount(tester, PluginsBackend(), size: size);
      final empty = find.byType(ContentEmpty);
      final title = find.descendant(of: empty, matching: find.text('Plugins'));
      final message = find.descendant(
        of: empty,
        matching: find.text('No hay contenido disponible'),
      );
      final center =
          (tester.getTopLeft(title).dy + tester.getBottomLeft(message).dy) / 2;
      expect(center, closeTo((size.height + 56) / 2, 1));
    });
  }
  testWidgets('menú contextual elimina el plugin y resuelve su favicon', (
    tester,
  ) async {
    AssignmentLogo.clearMemoryCache();
    final calls = <MethodCall>[];
    const channel = MethodChannel('gennie/assignment-logo');
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (call) async {
          calls.add(call);
          return null;
        });
    addTearDown(
      () => TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(channel, null),
    );
    final backend = PluginsBackend();
    await mount(tester, backend);
    expect(
      calls.any(
        (call) => (call.arguments as Map)['url'] == 'https://example.com',
      ),
      isTrue,
    );
    final gesture = await tester.createGesture(
      kind: PointerDeviceKind.mouse,
      buttons: kSecondaryMouseButton,
    );
    await gesture.down(tester.getCenter(find.text('Correo de prueba').first));
    await gesture.up();
    await tester.pumpAndSettle();
    expect(find.text('Eliminar plugin'), findsOneWidget);
    await tester.tap(find.text('Eliminar plugin'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Eliminar').last);
    await tester.pumpAndSettle();
    expect(
      backend.requests.any(
        (r) =>
            r['action'] == 'delete' &&
            r['plugin_id'] == 'gmail_test' &&
            r['expected_revision'] == 1,
      ),
      isTrue,
    );
    expect(find.text('Correo de prueba'), findsNothing);
  });
  testWidgets(
    'cancelar sin plugin publicado retira la fila y conserva el trabajo',
    (tester) async {
      final backend = PluginsBackend()..records = [];
      backend.jobs = [
        {
          'id': 10,
          'plugin_id': 'mail',
          'origin': 'https://mail.google.com',
          'state': 'waiting_user',
          'summary': 'Necesita acceso',
          'objective': 'Leer mensajes',
          'max_seconds': 600,
          'trigger_conversation_id': 7,
          'attempts': 1,
          'max_attempts': 4,
        },
      ];
      await mount(tester, backend);
      await tester.tap(find.text('mail.google.com'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Cancelar trabajo'));
      await tester.pumpAndSettle();
      expect(find.text('mail.google.com'), findsNothing);
      expect(find.text('Construcción cancelada.'), findsNothing);
      expect(backend.jobs.single['state'], 'cancelled');
    },
  );

  testWidgets(
    'desde el historial se puede generar de nuevo un trabajo cancelado',
    (tester) async {
      final backend = PluginsBackend()..records = [];
      backend.value = snapshot(
        selected: '7',
        messages: [
          const Message(
            speaker: 'system',
            text: 'Plugin · example.com: Construcción cancelada.',
            createdAt: '2026-10-26T12:00:00Z',
          ),
        ],
      );
      backend.jobs = [
        {
          'id': 10,
          'plugin_id': 'mail',
          'origin': 'https://example.com',
          'objective': 'Leer mensajes',
          'state': 'cancelled',
          'summary': 'Construcción cancelada.',
          'trigger_conversation_id': 7,
          'attempts': 1,
          'max_attempts': 4,
          'max_seconds': 600,
          'events': [],
        },
      ];
      await mount(tester, backend);
      expect(find.text('example.com'), findsNothing);
      await tester.tap(rail('Historial'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Plugin · example.com'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Volver a generar'));
      await tester.pumpAndSettle();
      final request = backend.requests.singleWhere(
        (r) => r['action'] == 'build',
      );
      expect(request['trigger_conversation_id'], 7);
      expect(request['request']['objective'], 'Leer mensajes');
      expect(request['request']['retry_of'], 10);
      expect(backend.jobs.any((j) => j['state'] == 'cancelled'), isTrue);
      expect(find.text('example.com'), findsWidgets);
    },
  );
  testWidgets(
    'un plugin publicado permanece visible tras cancelar su construcción',
    (tester) async {
      final backend = PluginsBackend();
      backend.jobs = [
        {
          'id': 10,
          'plugin_id': 'gmail_test',
          'origin': 'https://example.com',
          'state': 'cancelled',
          'summary': 'Construcción cancelada.',
          'trigger_conversation_id': 7,
          'attempts': 1,
          'max_attempts': 4,
        },
      ];
      await mount(tester, backend);
      expect(find.text('Correo de prueba'), findsOneWidget);
      await select(tester);
      expect(
        find.text('Consulta la bandeja de entrada de la cuenta conectada.'),
        findsOneWidget,
      );
    },
  );
  testWidgets('el remitente abre la ficha del plugin correspondiente', (
    tester,
  ) async {
    final backend = PluginsBackend();
    backend.value = snapshot(
      selected: '7',
      messages: [
        const Message(
          speaker: 'system',
          text: 'Plugin · example.com: Ya investigué.',
          createdAt: '2026-10-26T12:00:00Z',
        ),
      ],
    );
    backend.jobs = [
      {
        'id': 1,
        'plugin_id': 'gmail_test',
        'origin': 'https://example.com',
        'trigger_conversation_id': 7,
        'state': 'completed',
        'summary': 'Finalizado',
        'attempts': 1,
        'max_attempts': 4,
        'events': [
          {'message': 'Ya investigué.', 'created_at': '2026-10-26T12:00:00Z'},
        ],
      },
    ];
    await mount(tester, backend);
    await tester.tap(rail('Historial'));
    await tester.pumpAndSettle();
    expect(find.text('Plugin · example.com'), findsOneWidget);
    await tester.tap(find.text('Plugin · example.com'));
    await tester.pumpAndSettle();
    expect(
      find.text('Consulta la bandeja de entrada de la cuenta conectada.'),
      findsOneWidget,
    );
    expect(find.text('Correo de prueba · Completado'), findsOneWidget);
  });

  setUpAll(() async {
    final inter = FontLoader('Inter');
    for (final name in [
      'Inter-Regular',
      'Inter-Medium',
      'Inter-SemiBold',
      'Inter-Bold',
    ]) {
      inter.addFont(rootBundle.load('assets/fonts/$name.ttf'));
    }
    await inter.load();
    final icons = FontLoader('MaterialIcons')
      ..addFont(rootBundle.load('fonts/MaterialIcons-Regular.otf'));
    await icons.load();
  });
  testWidgets(
    'muestra capacidades, límites y resultado de una función con parámetros',
    (tester) async {
      final backend = PluginsBackend();
      await mount(tester, backend);
      await select(tester);
      expect(
        find.text('Listar mensajes recientes', findRichText: true),
        findsNothing,
      ); // Las capacidades incluyen viñeta visible.
      expect(find.text('• Listar mensajes recientes'), findsOneWidget);
      expect(find.text('• No envía ni elimina correos'), findsOneWidget);
      await tester.ensureVisible(
        find.byKey(const ValueKey('plugin-param-limit')),
      );
      await tester.enterText(
        find.byKey(const ValueKey('plugin-param-limit')),
        '5',
      );
      backend.pending = Completer<String>();
      await tester.ensureVisible(find.text('Ejecutar función'));
      await tester.tap(find.text('Ejecutar función'));
      await tester.pump();
      expect(find.text('Ejecutando…'), findsOneWidget);
      expect(backend.requests.last['arguments'], {'limit': 5});
      expect(backend.requests.last['action'], 'run');
      backend.pending!.complete(
        jsonEncode({
          'status': 'ok',
          'data': [
            {'subject': 'Correo recibido'},
          ],
        }),
      );
      await tester.pumpAndSettle();
      await tester.ensureVisible(find.byKey(const ValueKey('plugin-result')));
      expect(find.textContaining('Correo recibido'), findsOneWidget);
      expect(find.text('Iniciar sesión en Gennie Web'), findsNothing);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );
  testWidgets(
    'login solo tras una respuesta que requiere autenticación y prueba borradores',
    (tester) async {
      final backend = PluginsBackend();
      backend.records.first['verified'] = <String, dynamic>{};
      backend.status = 'authentication_required';
      await mount(tester, backend);
      await select(tester);
      expect(find.text('Iniciar sesión en Gennie Web'), findsNothing);
      await tester.ensureVisible(
        find.byKey(const ValueKey('plugin-param-limit')),
      );
      await tester.enterText(
        find.byKey(const ValueKey('plugin-param-limit')),
        '2',
      );
      await tester.ensureVisible(find.text('Ejecutar función'));
      await tester.tap(find.text('Ejecutar función'));
      await tester.pumpAndSettle();
      expect(backend.requests.where((r) => r['action'] == 'verify').length, 1);
      await tester.ensureVisible(find.text('Iniciar sesión en Gennie Web'));
      await tester.tap(find.text('Iniciar sesión en Gennie Web'));
      await tester.pumpAndSettle();
      expect(backend.requests.last['action'], 'connect');
      await tester.pumpWidget(const SizedBox());
    },
  );
  testWidgets(
    'editar conserva el borrador ante conflicto y permite eliminar desde la ficha',
    (tester) async {
      final backend = PluginsBackend();
      await mount(tester, backend);
      await select(tester);
      await tester.tap(find.byKey(const ValueKey('plugin-edit')));
      await tester.pumpAndSettle();
      final edited = Map<String, dynamic>.from(
        backend.records.first['manifest'] as Map,
      )..['description'] = 'Cobertura editada';
      await tester.enterText(
        find.byKey(const ValueKey('plugin-editor')),
        jsonEncode(edited),
      );
      backend.conflict = true;
      await tester.tap(find.text('Guardar'));
      await tester.pumpAndSettle();
      expect(find.textContaining('El plugin cambió.'), findsOneWidget);
      expect(
        tester
            .widget<TextField>(find.byKey(const ValueKey('plugin-editor')))
            .controller!
            .text,
        contains('Cobertura editada'),
      );
      backend.conflict = false;
      await tester.tap(find.text('Guardar'));
      await tester.pumpAndSettle();
      expect(find.text('Cobertura editada'), findsOneWidget);
      await tester.ensureVisible(find.text('Eliminar'));
      await tester.tap(find.text('Eliminar'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Eliminar').last);
      await tester.pumpAndSettle();
      expect(backend.records, isEmpty);
      await tester.pumpWidget(const SizedBox());
    },
  );
  testWidgets('los trabajos conservan origen y permiten continuar tras login', (
    tester,
  ) async {
    final backend = PluginsBackend();
    backend.records = [];
    backend.jobs = [
      {
        'id': 10,
        'plugin_id': 'mail',
        'state': 'waiting_user',
        'summary': 'Necesita acceso a la cuenta',
        'trigger_conversation_id': 7,
        'attempts': 1,
        'max_attempts': 4,
      },
    ];
    await mount(tester, backend);
    await tester.tap(find.text('Plugin en preparación'));
    await tester.pumpAndSettle();
    expect(find.textContaining('Chat 7'), findsOneWidget);
    expect(find.text('Abrir sitio en Gennie Web'), findsOneWidget);
    expect(
      find.text(
        'Abre el sitio y completa la acción solicitada. Después, reanuda la construcción.',
      ),
      findsOneWidget,
    );
    await tester.tap(find.text('Reanudar construcción'));
    await tester.pumpAndSettle();
    final resume = backend.requests.singleWhere((r) => r['action'] == 'resume');
    expect(resume['job_id'], 10);
    expect(resume.containsKey('trigger_conversation_id'), false);
    await tester.pumpWidget(const SizedBox());
  });
  testWidgets(
    'el navegador ocupado conserva los plugins y muestra la construcción',
    (tester) async {
      final backend = PluginsBackend();
      await mount(tester, backend);
      await select(tester);
      backend.catalogueStatus = 'busy';
      backend.catalogueError = 'El navegador de Gennie está ocupado';
      backend.jobs = [
        {
          'id': 13,
          'plugin_id': 'gmail_test',
          'origin': 'https://example.com',
          'state': 'running',
          'summary': 'Comprobando la lectura y la retirada de Recibidos',
          'trigger_conversation_id': 7,
          'attempts': 1,
        },
      ];
      await tester.tap(find.widgetWithIcon(IconButton, Icons.refresh));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 100));
      expect(find.text('Correo de prueba'), findsWidgets);
      expect(find.text('Construyendo'), findsWidgets);
      expect(find.text('No hay contenido disponible'), findsNothing);
      expect(
        find.text(
          'El navegador está trabajando. El catálogo se actualizará automáticamente.',
        ),
        findsOneWidget,
      );
      await tester.pumpWidget(const SizedBox());
    },
  );
  testWidgets(
    'una primera carga ocupada muestra trabajos y recupera el catálogo sin recarga manual',
    (tester) async {
      final backend = PluginsBackend()
        ..catalogueStatus = 'busy'
        ..catalogueError = 'El navegador de Gennie está ocupado';
      backend.jobs = [
        {
          'id': 13,
          'plugin_id': 'gmail_test',
          'origin': 'https://example.com',
          'state': 'queued',
          'summary': 'Preparación en cola',
          'trigger_conversation_id': 7,
          'attempts': 0,
        },
      ];
      await mount(tester, backend);
      expect(find.text('example.com'), findsOneWidget);
      await tester.tap(find.text('example.com'));
      await tester.pumpAndSettle();
      expect(find.text('Preparación en cola'), findsOneWidget);
      expect(find.text('No hay contenido disponible'), findsNothing);
      backend.catalogueStatus = 'ready';
      backend.catalogueError = null;
      await tester.pump(const Duration(seconds: 2));
      await tester.pumpAndSettle();
      expect(find.text('Correo de prueba'), findsWidgets);
      expect(
        find.text(
          'El navegador está trabajando. El catálogo se actualizará automáticamente.',
        ),
        findsNothing,
      );
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );
  testWidgets(
    'un catálogo inaccesible no se presenta como vacío y reintenta aunque no haya trabajos',
    (tester) async {
      final backend = PluginsBackend()
        ..catalogueStatus = 'unavailable'
        ..catalogueError = 'Falta preparar el navegador';
      await mount(tester, backend);
      expect(
        find.text('Catálogo temporalmente no disponible'),
        findsNWidgets(2),
      );
      expect(find.text('No hay contenido disponible'), findsNothing);
      backend.catalogueError = null;
      backend.catalogueStatus = 'ready';
      await tester.pump(const Duration(seconds: 2));
      await tester.pumpAndSettle();
      expect(find.text('Correo de prueba'), findsOneWidget);
      await tester.pumpWidget(const SizedBox());
    },
  );
  testWidgets(
    'los plugins en preparación aparecen en la lista y se pueden seleccionar',
    (tester) async {
      final backend = PluginsBackend()..records = [];
      backend.jobs = [
        {
          'id': 10,
          'plugin_id': 'assignment_2_0',
          'origin': 'https://mail.google.com',
          'state': 'waiting_user',
          'summary': 'Necesita acceso a la cuenta',
          'trigger_conversation_id': 7,
          'attempts': 1,
          'max_attempts': 4,
        },
      ];
      await mount(tester, backend);
      expect(find.text('mail.google.com'), findsOneWidget);
      await tester.tap(find.text('mail.google.com'));
      await tester.pumpAndSettle();
      expect(find.text('Plugin'), findsOneWidget);
      expect(find.text('https://mail.google.com'), findsOneWidget);
      expect(find.text('Necesita acceso a la cuenta'), findsOneWidget);
      expect(find.text('Reanudar construcción'), findsOneWidget);
      expect(find.text('No hay contenido disponible'), findsNothing);
      await tester.pumpWidget(const SizedBox());
    },
  );
  testWidgets('al volver de Encargos conserva la ficha de construcción', (
    tester,
  ) async {
    final backend = PluginsBackend()..records = [];
    backend.jobs = [
      {
        'id': 10,
        'plugin_id': 'mail',
        'origin': 'https://mail.google.com',
        'state': 'blocked',
        'summary': 'Acceso bloqueado',
        'trigger_conversation_id': 7,
        'attempts': 2,
        'max_attempts': 4,
      },
    ];
    await mount(tester, backend);
    await tester.tap(find.text('mail.google.com'));
    await tester.pumpAndSettle();
    expect(find.text('Plugin'), findsOneWidget);
    await tester.tap(rail('Encargos'));
    await tester.pumpAndSettle();
    await tester.tap(rail('Plugins'));
    await tester.pumpAndSettle();
    expect(find.text('Plugin'), findsOneWidget);
    expect(find.text('https://mail.google.com'), findsOneWidget);
  });
  testWidgets('una actualización comparte la fila del plugin publicado', (
    tester,
  ) async {
    final backend = PluginsBackend();
    backend.jobs = [
      {
        'id': 3,
        'plugin_id': 'gmail_test',
        'origin': 'https://example.com',
        'state': 'waiting_user',
        'summary': 'Necesita acceso',
        'trigger_conversation_id': 7,
        'attempts': 1,
        'max_attempts': 4,
      },
    ];
    await mount(tester, backend);
    expect(find.text('Correo de prueba'), findsOneWidget);
    expect(find.text('example.com'), findsNothing);
    await select(tester);
    expect(find.text('Reanudar construcción'), findsOneWidget);
    await tester.pumpWidget(const SizedBox());
  });
  testWidgets('muestra solo el último trabajo y su hora local', (tester) async {
    final backend = PluginsBackend()..records = [];
    final updated = DateTime(2026, 10, 26, 17, 19).toUtc().toIso8601String();
    backend.jobs = [
      {
        'id': 1,
        'plugin_id': 'mail',
        'origin': 'https://mail.google.com',
        'state': 'running',
        'summary': 'Resumen antiguo',
        'trigger_conversation_id': 7,
        'attempts': 1,
        'max_attempts': 4,
        'updated_at': updated,
      },
      {
        'id': 2,
        'plugin_id': 'mail',
        'origin': 'https://mail.google.com',
        'state': 'blocked',
        'summary': 'Resultado más reciente',
        'trigger_conversation_id': 7,
        'attempts': 2,
        'max_attempts': 4,
        'updated_at': updated,
      },
    ];
    await mount(tester, backend);
    await tester.tap(find.text('mail.google.com'));
    await tester.pumpAndSettle();
    expect(find.text('Resultado más reciente'), findsOneWidget);
    expect(find.text('Resumen antiguo'), findsNothing);
    expect(find.text('Actualizado 26/10/2026 · 17:19'), findsOneWidget);
    expect(find.text('Bloqueado'), findsWidgets);
    expect(find.text('Construyendo'), findsNothing);
    await tester.pumpWidget(const SizedBox());
  });
  testWidgets('el último bloqueo muestra su paso y permite volver a generar', (
    tester,
  ) async {
    final backend = PluginsBackend()..records = [];
    backend.jobs = [
      {
        'id': 4,
        'plugin_id': 'mail',
        'origin': 'https://mail.google.com',
        'objective': 'Revisar correo',
        'state': 'blocked',
        'summary': 'Bloqueo anterior',
        'trigger_conversation_id': 7,
        'attempts': 1,
        'max_attempts': 4,
        'max_seconds': 600,
      },
      {
        'id': 5,
        'plugin_id': 'mail',
        'origin': 'https://mail.google.com',
        'objective': 'Revisar correo',
        'state': 'blocked',
        'summary': 'No se puede archivar',
        'spent_seconds': 29,
        'trigger_conversation_id': 7,
        'attempts': 2,
        'max_attempts': 4,
        'max_seconds': 600,
        'events': [
          {
            'message': 'Última comprobación: lectura parcial',
            'created_at': '2026-10-26T22:19:20Z',
          },
        ],
      },
    ];
    await mount(tester, backend);
    await tester.tap(find.text('mail.google.com'));
    await tester.pumpAndSettle();
    expect(find.text('No se puede archivar'), findsOneWidget);
    expect(find.text('Tiempo de construcción: 29 s'), findsOneWidget);
    expect(find.text('Última comprobación: lectura parcial'), findsOneWidget);
    expect(find.text('Bloqueo anterior'), findsNothing);
    await tester.tap(find.text('Volver a generar'));
    await tester.pumpAndSettle();
    final request = backend.requests.singleWhere((r) => r['action'] == 'build');
    expect(request['request']['objective'], 'Revisar correo');
    expect(request['request']['retry_of'], 5);
    expect(request['trigger_conversation_id'], 7);
    await tester.pumpWidget(const SizedBox());
  });
  testWidgets(
    'la construcción activa muestra actividad y edad de la solicitud',
    (tester) async {
      final backend = PluginsBackend()..records = [];
      backend.jobs = [
        {
          'id': 10,
          'plugin_id': 'mail',
          'origin': 'https://mail.google.com',
          'state': 'waiting_user',
          'summary': 'Revisando acceso',
          'trigger_conversation_id': 7,
          'attempts': 1,
          'max_attempts': 4,
          'created_at': DateTime.now()
              .toUtc()
              .subtract(const Duration(minutes: 3))
              .toIso8601String(),
        },
      ];
      await mount(tester, backend);
      await tester.tap(find.text('mail.google.com'));
      await tester.pumpAndSettle();
      backend.jobs.first['state'] = 'running';
      await tester.tap(find.byIcon(Icons.refresh));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 100));
      expect(find.byType(CircularProgressIndicator), findsNWidgets(2));
      expect(find.text('Construyendo'), findsNWidgets(2));
      expect(find.text('Solicitud: hace 3 min'), findsOneWidget);
      expect(find.text('Revisando acceso'), findsOneWidget);
      backend.jobs.first['state'] = 'blocked';
      await tester.tap(find.byIcon(Icons.refresh));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 100));
      expect(find.byType(CircularProgressIndicator), findsNothing);
      expect(find.textContaining('Solicitud: hace'), findsNothing);
      await tester.pumpWidget(const SizedBox());
    },
  );
  for (final brightness in Brightness.values) {
    for (final size in [const Size(1100, 720), const Size(640, 420)]) {
      testWidgets('ficha de construcción ${brightness.name} ${size.width}', (
        tester,
      ) async {
        final backend = PluginsBackend()..records = [];
        backend.jobs = [
          {
            'id': 10,
            'plugin_id': 'mail',
            'origin': 'https://mail.google.com',
            'objective': 'Consultar mensajes recientes',
            'state': 'waiting_user',
            'summary': 'Necesita acceso a la cuenta',
            'trigger_conversation_id': 7,
            'attempts': 1,
            'max_attempts': 4,
            'events': [],
          },
        ];
        final key = GlobalKey();
        await mount(
          tester,
          backend,
          size: size,
          brightness: brightness,
          boundary: key,
        );
        await tester.tap(find.text('mail.google.com'));
        await tester.pumpAndSettle();
        expect(find.text('Para qué'), findsOneWidget);
        expect(find.text('Consultar mensajes recientes'), findsOneWidget);
        expect(tester.takeException(), isNull);
        if (const bool.fromEnvironment('CAPTURE_UI')) {
          await tester.runAsync(() async {
            final boundary =
                key.currentContext!.findRenderObject()!
                    as RenderRepaintBoundary;
            final image = await boundary.toImage();
            final bytes = (await image.toByteData(
              format: ImageByteFormat.png,
            ))!.buffer.asUint8List();
            await File(
              '../.verification/plugins-job-${brightness.name}-${size.width.toInt()}.png',
            ).writeAsBytes(bytes);
            image.dispose();
          });
        }
        await tester.pumpWidget(const SizedBox());
      });
    }
  }
  testWidgets('Un trabajo terminado conserva sus pasos de investigación', (
    tester,
  ) async {
    final backend = PluginsBackend()..records = [];
    backend.jobs = [
      {
        'id': 11,
        'plugin_id': 'mail',
        'origin': 'https://mail.example.com',
        'state': 'completed',
        'summary': 'Funciones verificadas',
        'trigger_conversation_id': 7,
        'attempts': 2,
        'max_attempts': 4,
        'events': [
          {
            'id': 1,
            'message': 'Investigando la petición de mensajes',
            'created_at': '2026-10-26T12:00:00Z',
          },
          {
            'id': 2,
            'message': 'Extracción web verificada',
            'created_at': '2026-10-26T12:01:00Z',
          },
        ],
      },
    ];
    final capture = GlobalKey();
    await mount(tester, backend, boundary: capture);
    await tester.tap(find.text('mail.example.com'));
    await tester.pumpAndSettle();
    expect(find.text('Trabajo actual'), findsOneWidget);
    await tester.ensureVisible(find.text('Pasos de investigación'));
    await tester.tap(find.text('Pasos de investigación'));
    await tester.pumpAndSettle();
    expect(
      find.textContaining('1. Investigando la petición de mensajes'),
      findsOneWidget,
    );
    expect(find.textContaining('2. Extracción web verificada'), findsOneWidget);
    expect(find.text('Reanudar construcción'), findsNothing);
    expect(find.text('Abrir sitio en Gennie Web'), findsNothing);
    expect(tester.takeException(), isNull);
    if (const bool.fromEnvironment('CAPTURE_UI')) {
      final boundary =
          capture.currentContext!.findRenderObject()! as RenderRepaintBoundary;
      await tester.runAsync(() async {
        final image = await boundary.toImage();
        final bytes = await image.toByteData(format: ImageByteFormat.png);
        await File(
          '../.verification/plugins-investigation.png',
        ).writeAsBytes(bytes!.buffer.asUint8List());
        image.dispose();
      });
    }
    await tester.pumpWidget(const SizedBox());
  });
  for (final brightness in Brightness.values) {
    for (final size in [const Size(1100, 720), const Size(640, 420)]) {
      testWidgets('ficha utilizable ${brightness.name} ${size.width}', (
        tester,
      ) async {
        final backend = PluginsBackend();
        final key = GlobalKey();
        await mount(
          tester,
          backend,
          size: size,
          brightness: brightness,
          boundary: key,
        );
        await select(tester);
        await tester.ensureVisible(find.text('Ejecutar función'));
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
        if (const bool.fromEnvironment('CAPTURE_UI')) {
          await tester.runAsync(() async {
            final boundary =
                key.currentContext!.findRenderObject()!
                    as RenderRepaintBoundary;
            final image = await boundary.toImage();
            final bytes = (await image.toByteData(
              format: ImageByteFormat.png,
            ))!.buffer.asUint8List();
            final file = File(
              '../.verification/plugins-${brightness.name}-${size.width.toInt()}.png',
            );
            await file.parent.create(recursive: true);
            await file.writeAsBytes(bytes);
            image.dispose();
          });
        }
        await tester.pumpWidget(const SizedBox());
      });
    }
  }
}
