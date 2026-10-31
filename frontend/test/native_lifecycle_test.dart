import 'dart:async';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'widget_test.dart' show FakeBackend, mount;

class ShutdownBackend extends FakeBackend {
  Completer<void>? gate;
  bool fail = false;
  int calls = 0;
  @override
  Future<void> shutdown() async {
    calls++;
    await gate?.future;
    if (fail) throw StateError('Fallo de cierre ficticio');
  }
}

Future<Object?> requestQuit() {
  const codec = StandardMethodCodec();
  return TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
      .handlePlatformMessage(
        'gennie/lifecycle',
        codec.encodeMethodCall(const MethodCall('requestQuit')),
        null,
      )
      .then((data) => codec.decodeEnvelope(data!));
}

void main() {
  testWidgets('requestQuit espera el cierre y rechaza un duplicado pendiente', (
    t,
  ) async {
    final backend = ShutdownBackend()..gate = Completer<void>();
    await mount(t, backend: backend);
    var completed = false;
    late Future<Object?> first;
    await t.runAsync(() async {
      first = requestQuit().then((reply) {
        completed = true;
        return reply;
      });
    });
    await t.pump();
    expect(backend.calls, 1);
    expect(completed, isFalse);
    expect(await t.runAsync(requestQuit), isFalse);
    expect(backend.calls, 1);
    backend.gate!.complete();
    await t.pump();
    expect(await t.runAsync(() => first), isTrue);
  });

  testWidgets('requestQuit rechazado conserva el error y permite reintentar', (
    t,
  ) async {
    final backend = ShutdownBackend()..fail = true;
    final (_, state) = await mount(t, backend: backend);
    final first = await t.runAsync(requestQuit);
    await t.pump();
    expect(first, isFalse);
    expect(state.failure, contains('Fallo de cierre ficticio'));
    expect(find.textContaining('Fallo de cierre ficticio'), findsWidgets);
    backend.fail = false;
    final second = await t.runAsync(requestQuit);
    await t.pump();
    expect(second, isTrue);
    expect(backend.calls, 2);
  });
}
