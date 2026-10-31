import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gennie/src/backend.dart';
import 'package:gennie/src/system_motion.dart';
import 'widget_test.dart' show mount, snapshot, rail;

void main() {
  testWidgets('AppKit inicial y cambios gobiernan el indicador sin guardar', (
    t,
  ) async {
    const channel = MethodChannel('gennie/system-motion');
    final messenger = t.binding.defaultBinaryMessenger;
    final methods = <String>[];
    messenger.setMockMethodCallHandler(channel, (call) async {
      methods.add(call.method);
      return null;
    });
    addTearDown(() => messenger.setMockMethodCallHandler(channel, null));
    Future<void> emit(bool value) async {
      await messenger.handlePlatformMessage(
        channel.name,
        const StandardMethodCodec().encodeSuccessEnvelope(value),
        (_) {},
      );
      await t.pump();
    }

    final motion = SystemMotion();
    final ready = motion.start();
    expect(identical(ready, motion.start()), isTrue);
    await t.pump();
    await emit(true);
    await ready;
    expect(motion.value, isTrue);
    final (backend, _) = await mount(t, systemMotion: motion);
    backend.events.add(snapshot(modelState: ModelState.loading));
    await t.pump();
    await t.pump(const Duration(milliseconds: 100));
    final indicator = find.descendant(
      of: rail('Laya'),
      matching: find.byType(CircularProgressIndicator),
    );
    expect(t.widget<CircularProgressIndicator>(indicator).value, .75);
    await emit(false);
    expect(t.widget<CircularProgressIndicator>(indicator).value, isNull);
    backend.events.add(
      snapshot(modelState: ModelState.loading, reducedMotion: true),
    );
    await t.pump();
    await emit(true);
    await emit(false);
    expect(t.widget<CircularProgressIndicator>(indicator).value, .75);
    expect(backend.motionSaved, isNull);
    expect(backend.modelRequests, isEmpty);
    await t.pumpWidget(const SizedBox());
    motion.dispose();
    await t.pump();
    expect(methods, ['listen', 'cancel']);
    expect(t.takeException(), isNull);
  }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));
}
