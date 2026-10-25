import 'dart:async';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:gennie/src/rust/api.dart' as api;
import 'package:gennie/src/rust/frb_generated.dart';

// Biblioteca propia y .env con SQLite temporal: solo presencia activa.
// No carga modelos ni inicia dictado, LLM, voz o decisiones.
void main() {
  const library = String.fromEnvironment('GENNIE_BRIDGE_SMOKE_LIBRARY');
  const env = String.fromEnvironment('GENNIE_BRIDGE_SMOKE_ENV');
  const helper = String.fromEnvironment('GENNIE_BRIDGE_SMOKE_HELPER');
  test(
    'FRB real: stream, catálogo, errores y persistencia al reabrir',
    () async {
      await RustLib.init(externalLibrary: ExternalLibrary.open(library));
      bool running = false;
      StreamSubscription<api.Snapshot>? subscription;
      try {
        debugPrint('FRB smoke: initialize');
        final initial = await api.initialize(envFile: env, helperPath: helper);
        running = true;
        debugPrint('FRB smoke: initialized');
        expect(initial.assistant, isNull);
        expect(initial.dictation, isNull);
        expect(initial.voice, isNull);
        expect(
          initial.conversations,
          isNotEmpty,
          reason: 'El fixture debe tener una conversación local',
        );
        final observed = Completer<api.Snapshot>();
        subscription = api.watch().listen(
          (value) {
            if (!observed.isCompleted) observed.complete(value);
          },
          onError: (Object error, StackTrace stack) {
            if (!observed.isCompleted) observed.completeError(error, stack);
          },
        );
        debugPrint('FRB smoke: watch subscribed');
        final streamed = await observed.future.timeout(
          const Duration(seconds: 5),
        );
        expect(streamed.conversations.length, initial.conversations.length);
        debugPrint('FRB smoke: stream received');
        final chat = initial.conversations.first;
        final selected = await api.selectConversation(id: chat.id);
        expect(
          selected.messages,
          isNotEmpty,
          reason: 'El fixture debe tener un mensaje',
        );
        final originalMessages = selected.messages;
        const editedTitle = 'Verificación FRB';
        debugPrint('FRB smoke: selected');
        final renamed = await api.renameConversation(
          id: chat.id,
          title: editedTitle,
        );
        expect(
          renamed.conversations.singleWhere((c) => c.id == chat.id).title,
          editedTitle,
        );
        await expectLater(
          api.renameConversation(id: chat.id, title: '   '),
          throwsA(anything),
        );
        final reread = await api.selectConversation(id: chat.id);
        expect(reread.messages, originalMessages);
        expect(
          reread.conversations.singleWhere((c) => c.id == chat.id).title,
          editedTitle,
        );
        final motion = !initial.reducedMotion;
        expect(
          (await api.saveReducedMotion(value: motion)).reducedMotion,
          motion,
        );
        debugPrint('FRB smoke: shutdown closes retained stream sinks');
        await api.shutdown();
        running = false;
        await subscription.cancel();
        subscription = null;
        debugPrint('FRB smoke: reopen');
        final reopened = await api.initialize(envFile: env, helperPath: helper);
        running = true;
        expect(reopened.reducedMotion, motion);
        expect(
          reopened.conversations.singleWhere((c) => c.id == chat.id).title,
          editedTitle,
        );
        expect(
          (await api.selectConversation(id: chat.id)).messages,
          originalMessages,
        );
      } finally {
        if (running) {
          debugPrint('FRB smoke: final shutdown');
          await api.shutdown();
        }
        await subscription?.cancel();
      }
    },
    skip: library.isEmpty || env.isEmpty || helper.isEmpty,
    timeout: const Timeout(Duration(seconds: 30)),
  );
}
