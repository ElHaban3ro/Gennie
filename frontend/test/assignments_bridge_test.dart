import 'dart:convert';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:gennie/src/rust/api.dart' as api;
import 'package:gennie/src/rust/frb_generated.dart';

void main() {
  const library = String.fromEnvironment('GENNIE_ASSIGNMENTS_SMOKE_LIBRARY');
  const env = String.fromEnvironment('GENNIE_ASSIGNMENTS_SMOKE_ENV');
  const helper = String.fromEnvironment('GENNIE_ASSIGNMENTS_SMOKE_HELPER');
  const legacy = bool.fromEnvironment('GENNIE_ASSIGNMENTS_SMOKE_LEGACY');
  test(
    'FRB real registra, valida y conserva encargos al reabrir sin proveedor',
    () async {
      await RustLib.init(externalLibrary: ExternalLibrary.open(library));
      var running = false;
      try {
        final initial = await api.initialize(envFile: env, helperPath: helper);
        running = true;
        expect(initial.assignmentsEnabled, isTrue);
        expect(initial.assistant, isNull);
        expect(initial.assignments, isEmpty);
        final document = jsonEncode({
          'title': 'Objetivo desde Flutter',
          'description': 'Investigar IA con fuentes primarias',
          'kind': 'goal',
          'priority': 'high',
          'status': 'active',
          'timezone': 'America/Bogota',
          'preparation': {
            'interval_minutes': 10,
            'rationale': 'Observar cambios entre entregas',
          },
          'method': {
            'plugin_id': 'research',
            'functions': ['list_sources'],
          },
          'authorization': {
            'scope': 'Consultar fuentes sin cambios',
            'allowed_routes': ['http', 'web'],
          },
          'follow_up': {
            'scope': 'Seguir avances y obstáculos de la investigación',
            'signals': ['¿Podemos adelantar algún paso útil?'],
            'decision_guidance':
                'Valorar contexto y cambios; registrar avances sin interrumpir salvo que aporte ayuda.',
            'action_scope': null,
          },
        });
        final saved = await api.saveAssignment(
          id: null,
          expectedRevision: null,
          requestKey: 'frb-own-fixture',
          document: document,
        );
        expect(saved.assignments.length, 1);
        expect(saved.confirmation, contains('Encargo guardado'));
        final row = saved.assignments.single;
        expect(
          jsonDecode(row.methodResolution!)['status'],
          'catalogue_unavailable',
        );
        final duplicate = await api.saveAssignment(
          id: null,
          expectedRevision: null,
          requestKey: 'frb-own-fixture',
          document: document,
        );
        expect(duplicate.assignments.length, 1);
        await expectLater(
          api.saveAssignment(
            id: row.id,
            expectedRevision: 0,
            requestKey: 'unused',
            document: document,
          ),
          throwsA(anything),
        );
        final edited = jsonDecode(row.document) as Map<String, dynamic>;
        edited['status'] = 'paused';
        final changed = await api.saveAssignment(
          id: row.id,
          expectedRevision: row.revision,
          requestKey: 'unused',
          document: jsonEncode(edited),
        );
        expect(changed.assignments.single.revision, row.revision + 1);
        await api.shutdown();
        running = false;
        final reopened = await api.initialize(envFile: env, helperPath: helper);
        running = true;
        expect(reopened.assignments.single.id, row.id);
        expect(
          jsonDecode(reopened.assignments.single.document)['status'],
          'paused',
        );
        expect(
          jsonDecode(reopened.assignments.single.document)['method'],
          jsonDecode(document)['method'],
        );
        expect(
          jsonDecode(reopened.assignments.single.document)['authorization'],
          jsonDecode(document)['authorization'],
        );
        expect(
          jsonDecode(reopened.assignments.single.methodResolution!)['status'],
          'catalogue_unavailable',
        );
        expect(
          jsonDecode(reopened.assignments.single.document)['follow_up'],
          jsonDecode(document)['follow_up'],
        );
      } finally {
        if (running) {
          await api.shutdown();
        }
      }
    },
    skip: legacy || library.isEmpty || env.isEmpty || helper.isEmpty,
    timeout: const Timeout(Duration(seconds: 20)),
  );
  test(
    'FRB real migra events a assignments conservando datos e idempotencia',
    () async {
      await RustLib.init(externalLibrary: ExternalLibrary.open(library));
      var running = false;
      try {
        final initial = await api.initialize(envFile: env, helperPath: helper);
        running = true;
        expect(initial.assignmentsEnabled, isTrue);
        expect(initial.assistant, isNull);
        final row = initial.assignments.single;
        expect(row.id, '41');
        expect(row.revision, 7);
        expect(row.createdAt, '2026-10-24T13:00:00.000Z');
        final document = jsonDecode(row.document) as Map<String, dynamic>;
        expect(document['title'], 'Encargo anterior');
        expect(document['follow_up']['scope'], 'Seguir mis pendientes');
        final repeated = await api.saveAssignment(
          id: null,
          expectedRevision: null,
          requestKey: 'legacy-key',
          document: row.document,
        );
        expect(repeated.assignments.single.id, '41');
        expect(repeated.assignments.single.revision, 7);
        await api.shutdown();
        running = false;
        final reopened = await api.initialize(envFile: env, helperPath: helper);
        running = true;
        expect(reopened.assignments.single.document, row.document);
        expect(reopened.assignments.single.id, row.id);
      } finally {
        if (running) {
          await api.shutdown();
        }
      }
    },
    skip: !legacy || library.isEmpty || env.isEmpty || helper.isEmpty,
    timeout: const Timeout(Duration(seconds: 20)),
  );
}
