import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:ui' show ImageByteFormat;
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/src/backend.dart';
import 'package:gennie/main.dart';
import 'widget_test.dart' show FakeBackend, rail;

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
  @override
  Future<String> pluginRequest(String text) async {
    final request = Map<String, dynamic>.from(jsonDecode(text) as Map);
    requests.add(request);
    switch (request['action']) {
      case 'list':
        return jsonEncode({'plugins': records, 'jobs': jobs});
      case 'run':
      case 'verify':
        return pending?.future ??
            jsonEncode({
              'status': status,
              'data': [
                {'id': 'mail-1', 'subject': 'Correo recibido'},
              ],
            });
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
      await tester.tap(find.text('Editar'));
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
    expect(find.textContaining('Chat 7'), findsOneWidget);
    await tester.tap(find.text('Continuar'));
    await tester.pumpAndSettle();
    final resume = backend.requests.singleWhere((r) => r['action'] == 'resume');
    expect(resume['job_id'], 10);
    expect(resume.containsKey('trigger_conversation_id'), false);
    await tester.pumpWidget(const SizedBox());
  });
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
      expect(find.text('Continuar'), findsOneWidget);
      expect(find.text('No hay contenido disponible'), findsNothing);
      await tester.pumpWidget(const SizedBox());
    },
  );
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
    expect(find.text('Continuar'), findsOneWidget);
    await tester.pumpWidget(const SizedBox());
  });
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
    expect(find.text('mail.example.com · Completado'), findsOneWidget);
    await tester.ensureVisible(find.text('Pasos de investigación'));
    await tester.tap(find.text('Pasos de investigación'));
    await tester.pumpAndSettle();
    expect(
      find.textContaining('1. Investigando la petición de mensajes'),
      findsOneWidget,
    );
    expect(find.textContaining('2. Extracción web verificada'), findsOneWidget);
    expect(find.text('Continuar'), findsNothing);
    expect(find.text('Abrir Gennie Web'), findsNothing);
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
