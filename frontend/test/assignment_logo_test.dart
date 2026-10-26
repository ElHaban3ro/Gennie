import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/src/assignment_logo.dart';
import 'package:gennie/src/sidebar.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('gennie/assignment-logo');
  final png = base64Decode(
    'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScLttAAAAABJRU5ErkJggg==',
  );
  final calls = <MethodCall>[];
  setUp(() {
    AssignmentLogo.clearMemoryCache();
    calls.clear();
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (call) async {
          calls.add(call);
          return png;
        });
  });
  tearDown(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, null);
  });

  Future<void> show(WidgetTester tester, Map<String, dynamic> document) async {
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: Center(
            child: SizedBox(
              width: 216,
              child: SidebarTile(
                title: 'Revisar mis tareas pendientes de la semana',
                subtitle: 'Recurrente · Registrado',
                selected: true,
                onSelect: () {},
                leading: AssignmentLogo(document: document),
              ),
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
  }

  testWidgets(
    'Logo real a la izquierda al ancho mínimo y petición compartida',
    (tester) async {
      final doc = {
        'title': 'Mis notas',
        'applications': [
          {'name': 'Notas de Apple', 'kind': 'desktop'},
        ],
      };
      await show(tester, doc);
      expect(find.byType(Image), findsOneWidget);
      final logo = tester.getRect(find.byType(AssignmentLogo));
      final title = tester.getRect(
        find.text('Revisar mis tareas pendientes de la semana'),
      );
      expect(logo.width, 38);
      expect(logo.right, lessThan(title.left));
      expect(calls.single.arguments, {
        'name': 'Notas de Apple',
        'kind': 'desktop',
        'url': '',
      });
      await tester.pumpWidget(const SizedBox());
      await show(tester, doc);
      expect(calls.length, 1);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets('Editar la fuente sustituye el logo y conserva la URL web', (
    tester,
  ) async {
    await show(tester, {
      'title': 'Seguimiento',
      'applications': [
        {'name': 'Calendario local', 'kind': 'desktop'},
      ],
    });
    await show(tester, {
      'title': 'Seguimiento',
      'applications': [
        {'name': 'Mi web', 'kind': 'web', 'url': 'https://example.com/tasks'},
      ],
    });
    expect(calls.length, 2);
    expect(calls.last.arguments['url'], 'https://example.com/tasks');
    expect(find.byType(Image), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('Sin fuente o sin imagen siempre hay avatar de reserva', (
    tester,
  ) async {
    await show(tester, {'title': 'Investigar un tema', 'applications': []});
    expect(find.text('I'), findsOneWidget);
    expect(calls, isEmpty);
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (_) async => null);
    await show(tester, {
      'title': 'Revisar',
      'applications': [
        {'name': 'Fuente sin icono', 'kind': 'desktop'},
      ],
    });
    expect(find.text('F'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('El encargo muestra calendario y badge solo con aplicación', (
    tester,
  ) async {
    Future<void> showIcon(Map<String, dynamic> document) async {
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(body: AssignmentIcon(document: document)),
        ),
      );
      await tester.pumpAndSettle();
    }

    await showIcon({'title': 'Encargo', 'applications': []});
    expect(find.byIcon(Icons.calendar_month_outlined), findsOneWidget);
    expect(find.byType(AssignmentLogo), findsNothing);
    await showIcon({
      'title': 'Encargo',
      'applications': [
        {'name': 'Notas', 'kind': 'desktop'},
      ],
    });
    final calendar = tester.getRect(find.byIcon(Icons.calendar_month_outlined));
    final badge = tester.getRect(find.byType(AssignmentLogo));
    expect(badge.center.dx, greaterThan(calendar.center.dx));
    expect(badge.center.dy, greaterThan(calendar.center.dy));
  });
}
