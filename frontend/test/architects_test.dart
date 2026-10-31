import 'dart:async';
import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/src/architects.dart';
import 'package:gennie/src/backend.dart';
import 'package:gennie/src/sidebar.dart';
import 'package:gennie/src/glass.dart';
import 'widget_test.dart' as fixture;
import 'chats_preview_test.dart' show loadFonts;
import 'tool_evidence_test.dart' show capture;

List<Map<String, dynamic>> profiles() => [
  for (final (id, role, provider) in [
    ('gennie', 'bot', 'codex_chatgpt'),
    ('plugin_builder', 'helper', 'codex_chatgpt'),
    ('proactivity_guide', 'guide', 'openrouter_decisions'),
  ])
    {
      'id': id,
      'name': switch (role) {
        'bot' => 'Gennie',
        'helper' => 'Constructor de plugins',
        _ => 'Guía de proactividad',
      },
      'role': role,
      'revision': 1,
      'provider': provider,
      'enabled': role != 'guide',
      'model': role == 'guide' ? 'typesafe/jev-1.14' : 'gpt-6-luna',
      'effort': role == 'guide' ? null : 'low',
      'personality': '',
      'instructions': '',
      'read_files': role != 'guide',
      'write_files': role != 'guide',
      'run_commands': role != 'guide',
      'local_actions': role != 'guide',
    },
];

class ArchitectFake extends fixture.FakeBackend implements ArchitectBackend {
  final records = profiles();
  final saves = <Map<String, dynamic>>[];
  Completer<String>? pending;
  String? failure;
  bool listFails = false;
  int nextId = 1;
  @override
  Future<String> architectRequest(String text) async {
    final request = jsonDecode(text) as Map<String, dynamic>;
    if (request['action'] == 'list') {
      if (listFails) throw Exception('Lectura no disponible');
      return jsonEncode({'profiles': records});
    }
    if (request['action'] == 'remove') {
      records.removeWhere((p) => p['id'] == request['id']);
      return jsonEncode({'removed': request['id']});
    }
    final profile = Map<String, dynamic>.of(
      request['profile'] as Map<String, dynamic>,
    );
    saves.add(Map.of(profile));
    if (failure != null) throw Exception(failure!);
    if (pending != null) return pending!.future;
    profile['revision'] = (profile['revision'] as int) + 1;
    if (request['action'] == 'create') {
      profile['id'] = 'architect_${nextId++}';
      records.add(profile);
    } else {
      records[records.indexWhere((p) => p['id'] == profile['id'])] = profile;
    }
    return jsonEncode({'profile': profile});
  }
}

Future<ArchitectFake> mount(
  WidgetTester t, {
  ArchitectFake? backend,
  Size size = const Size(1100, 720),
}) async {
  final b = backend ?? ArchitectFake();
  await fixture.mount(t, backend: b, size: size);
  await t.tap(fixture.rail('Arquitectos'));
  await t.pumpAndSettle();
  expect(find.byType(ArchitectSettingsPanel), findsOneWidget);
  return b;
}

Future<void> select(WidgetTester t, String name) async {
  final row = find.widgetWithText(SidebarTile, name);
  await t.ensureVisible(row);
  await t.tap(row);
  await t.pumpAndSettle();
}

Future<void> click(WidgetTester t, String text) async {
  final button = find.text(text).last;
  await t.ensureVisible(button);
  await t.tap(button);
  await t.pumpAndSettle();
}

void main() {
  testWidgets('eliminar una instancia adicional conserva las tres bases', (
    t,
  ) async {
    final b = await mount(t);
    await click(t, 'Duplicar');
    await click(t, 'Crear arquitecto');
    expect(b.records.length, 4);
    await t.tap(fixture.appTooltip('Eliminar arquitecto'));
    await t.pumpAndSettle();
    await t.tap(find.widgetWithText(FilledButton, 'Eliminar'));
    await t.pumpAndSettle();
    expect(b.records.length, 3);
    expect(find.widgetWithText(SidebarTile, 'Gennie'), findsOneWidget);
    expect(find.text('Arquitecto retirado'), findsOneWidget);
    expect(t.takeException(), isNull);
  });
  testWidgets(
    'tab propio conserva bases y borradores separados por identidad',
    (t) async {
      final b = await mount(t);
      for (final name in [
        'Gennie',
        'Constructor de plugins',
        'Guía de proactividad',
      ]) {
        expect(find.widgetWithText(SidebarTile, name), findsOneWidget);
      }
      await t.enterText(
        find.byKey(const ValueKey('gennie-personality')),
        'Cálido',
      );
      await select(t, 'Constructor de plugins');
      await t.enterText(
        find.byKey(const ValueKey('plugin_builder-personality')),
        'Metódico',
      );
      await select(t, 'Gennie');
      expect(find.text('Cálido'), findsOneWidget);
      await click(t, 'Guardar cambios');
      expect(b.saves.single['id'], 'gennie');
      expect(b.saves.single['personality'], 'Cálido');
      await select(t, 'Constructor de plugins');
      expect(find.text('Metódico'), findsOneWidget);
      await click(t, 'Guardar cambios');
      expect(b.saves.last['id'], 'plugin_builder');
      expect(fixture.appTooltip('Eliminar arquitecto'), findsNothing);
      expect(t.takeException(), isNull);
    },
  );

  testWidgets('crea varios helpers y tipos propios sin sustituir las bases', (
    t,
  ) async {
    final b = await mount(t);
    for (final (name, type) in [
      ('Investigador', 'helper'),
      ('Revisor', 'helper'),
      ('Especialista', 'Investigación'),
    ]) {
      await t.tap(fixture.appTooltip('Nuevo arquitecto'));
      await t.pumpAndSettle();
      await t.enterText(find.byKey(const ValueKey('new-architect-name')), name);
      await t.enterText(find.byKey(const ValueKey('new-architect-type')), type);
      await click(t, 'Crear arquitecto');
      expect(find.widgetWithText(SidebarTile, name), findsOneWidget);
    }
    expect(b.records.length, 6);
    expect(b.records.where((p) => p['role'] == 'helper').length, 3);
    expect(b.records.last['role'], 'Investigación');
    expect(b.records.take(3).map((p) => p['id']), [
      'gennie',
      'plugin_builder',
      'proactivity_guide',
    ]);
    expect(b.records.last['read_files'], false);
    await click(t, 'Guardar cambios');
    expect(b.saves.last['id'], 'architect_3');
    expect(b.saves.last['revision'], 1);
    expect(t.takeException(), isNull);
  });

  testWidgets(
    'conflicto conserva borrador y guardado bloquea navegación y duplicados',
    (t) async {
      final b = await mount(
        t,
        backend: ArchitectFake()..failure = 'El arquitecto cambió',
      );
      await t.enterText(
        find.byKey(const ValueKey('gennie-personality')),
        'Mi borrador',
      );
      await click(t, 'Guardar cambios');
      expect(find.textContaining('El arquitecto cambió'), findsOneWidget);
      expect(find.text('Mi borrador'), findsOneWidget);
      b.failure = null;
      b.pending = Completer<String>();
      await t.tap(find.text('Guardar cambios'));
      await t.pump();
      expect(t.widget<RailButton>(fixture.rail('Chats')).onPressed, isNull);
      expect(
        t.widget<RailButton>(fixture.rail('Configuración')).onPressed,
        isNull,
      );
      expect(
        t
            .widget<FilledButton>(
              find.widgetWithText(FilledButton, 'Guardando…'),
            )
            .onPressed,
        isNull,
      );
      b.pending!.complete(
        jsonEncode({
          'profile': {...b.saves.last, 'revision': 2},
        }),
      );
      await t.pumpAndSettle();
      expect(b.saves.length, 2);
      expect(find.text('Arquitecto guardado'), findsOneWidget);
    },
  );

  testWidgets(
    'duplicar base prepara una instancia independiente y descartar no guarda',
    (t) async {
      final b = await mount(t);
      await click(t, 'Duplicar');
      expect(find.text('Gennie · copia'), findsWidgets);
      final type = t.widget<TextFormField>(
        find.byKey(const ValueKey('new-architect-type')),
      );
      expect(type.enabled, isTrue);
      await t.tap(fixture.appTooltip('Descartar borrador'));
      await t.pumpAndSettle();
      expect(b.saves, isEmpty);
      expect(b.records.length, 3);
    },
  );

  testWidgets(
    'modelo inválido no envía y guía conserva proveedor sin herramientas',
    (t) async {
      final b = await mount(t);
      await click(t, 'Opciones avanzadas');
      await t.ensureVisible(find.byKey(const ValueKey('gennie-model')));
      await t.enterText(
        find.byKey(const ValueKey('gennie-model')),
        'modelo inválido',
      );
      await click(t, 'Guardar cambios');
      expect(b.saves, isEmpty);
      expect(
        find.text('Introduce un identificador de modelo válido.').hitTestable(),
        findsOneWidget,
      );
      await select(t, 'Guía de proactividad');
      await click(t, 'Opciones avanzadas');
      expect(find.text('Leer archivos'), findsNothing);
      await click(t, 'Guardar cambios');
      expect(b.saves.single['enabled'], false);
      expect(b.saves.single['provider'], 'openrouter_decisions');
    },
  );

  testWidgets(
    'lectura fallida permite reintentar y configuración no contiene arquitectos',
    (t) async {
      final b = await mount(t, backend: ArchitectFake()..listFails = true);
      expect(find.textContaining('Lectura no disponible'), findsOneWidget);
      b.listFails = false;
      await click(t, 'Reintentar');
      expect(find.text('Guardar cambios'), findsOneWidget);
      await t.tap(fixture.rail('Configuración'));
      await t.pumpAndSettle();
      expect(find.widgetWithText(ChoiceChip, 'Arquitectos'), findsNothing);
      expect(find.widgetWithText(ChoiceChip, 'Asistente'), findsOneWidget);
    },
  );

  for (final brightness in Brightness.values) {
    for (final size in [const Size(1100, 720), const Size(640, 420)]) {
      testWidgets('pantalla arquitectos ${brightness.name} ${size.width}', (
        t,
      ) async {
        t.platformDispatcher.platformBrightnessTestValue = brightness;
        addTearDown(t.platformDispatcher.clearPlatformBrightnessTestValue);
        await loadFonts();
        await mount(t, size: size);
        expect(find.text('Guardar cambios').hitTestable(), findsOneWidget);
        expect(t.takeException(), isNull);
        await capture(
          t,
          'architects-tab-${brightness.name}-${size.width.toInt()}',
        );
        await t.tap(fixture.appTooltip('Nuevo arquitecto'));
        await t.pumpAndSettle();
        await t.enterText(
          find.byKey(const ValueKey('new-architect-name')),
          'Investigador',
        );
        expect(find.text('Crear arquitecto').hitTestable(), findsOneWidget);
        expect(t.takeException(), isNull);
        await capture(
          t,
          'architects-new-${brightness.name}-${size.width.toInt()}',
        );
      });
    }
  }
}
