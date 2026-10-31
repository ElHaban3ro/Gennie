// Sonda de lectura real; el proveedor está apagado y la cancelación usa una identidad vencida.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/main.dart';
import 'package:gennie/src/backend.dart';
import 'assistant_chat_bridge_test.dart'
    show BridgeBackend, ensureLibrary, pumpUntil, capture;
import 'chats_preview_test.dart' show loadFonts;

void main() {
  const library = String.fromEnvironment('GENNIE_STOP_LIBRARY');
  const helper = String.fromEnvironment('GENNIE_STOP_HELPER');
  const env = String.fromEnvironment('GENNIE_STOP_ENV');
  const ending = String.fromEnvironment('GENNIE_STOP_ENDING');
  testWidgets('fragmento $ending y cancelación tardía por FRB real', (t) async {
    t.view.physicalSize = const Size(1100, 720);
    t.view.devicePixelRatio = 1;
    addTearDown(t.view.resetPhysicalSize);
    addTearDown(t.view.resetDevicePixelRatio);
    await t.runAsync(loadFonts);
    await t.runAsync(() => ensureLibrary(library));
    final backend = BridgeBackend(env, helper);
    final state = DesktopState(backend);
    try {
      await t.runAsync(state.start);
      expect(state.failure, isNull);
      expect(state.snapshot!.turn, isNull);
      expect(state.snapshot!.assistant, isNull);
      final before = (await t.runAsync(() => backend.loadChatPage(null)))!;
      expect(before.messages.length, 3);
      expect(before.messages[1].message.interruption, isNull);
      expect(
        before.messages.last.message.interruption,
        ending == 'cancelled'
            ? MessageInterruption.cancelled
            : MessageInterruption.failed,
      );
      // Un error nuevo debe seguir visible aunque llegue una cancelación vieja.
      final failure = await t.runAsync(() async {
        try {
          await backend.rename('99999', 'No existe');
          return null;
        } catch (error) {
          return '$error';
        }
      });
      expect(failure, isNotNull);
      await pumpUntil(t, () => state.snapshot?.error != null);
      final currentError = state.snapshot!.error;
      final withError = (await t.runAsync(() => backend.loadChatPage(null)))!;
      await t.runAsync(() => backend.stopResponse('already-finished'));
      await t.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 40)),
      );
      await t.pump();
      expect(state.snapshot!.error, currentError);
      final after = (await t.runAsync(() => backend.loadChatPage(null)))!;
      expect(
        after.messages.map((r) => r.message).toList(),
        withError.messages.map((r) => r.message).toList(),
      );
      await t.pumpWidget(
        RepaintBoundary(
          key: const ValueKey('real-chat-capture'),
          child: GennieApp(state: state),
        ),
      );
      final label = ending == 'cancelled'
          ? 'Respuesta detenida'
          : 'Respuesta interrumpida';
      await pumpUntil(t, () => find.text(label).evaluate().isNotEmpty);
      await t.pumpAndSettle();
      expect(find.text(label), findsOneWidget);
      expect(find.byKey(const ValueKey('stop-response')), findsNothing);
      expect(t.takeException(), isNull);
      await capture(t, 'stop-frb-$ending-1100');
    } finally {
      await t.pumpWidget(const SizedBox());
      await t.runAsync(state.close);
      state.dispose();
    }
  }, skip: library.isEmpty || helper.isEmpty || env.isEmpty);
}
