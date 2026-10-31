import 'dart:async';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

/// Preferencia efímera de AppKit, independiente de SQLite y del motor Flutter.
class SystemMotion extends ValueNotifier<bool> {
  SystemMotion() : super(false);

  static const _channel = EventChannel('gennie/system-motion');
  StreamSubscription<bool>? _subscription;
  Future<void>? _ready;
  bool _disposed = false;

  Future<void> start() => _ready ??= _listen();

  Future<void> _listen() {
    final first = Completer<void>();
    _subscription = _channel.receiveBroadcastStream().cast<bool>().listen(
      (reduced) {
        if (_disposed) return;
        value = reduced;
        if (!first.isCompleted) first.complete();
      },
      onError: (Object error, StackTrace stack) {
        if (!first.isCompleted) {
          first.completeError(error, stack);
        } else if (!_disposed) {
          FlutterError.reportError(
            FlutterErrorDetails(
              exception: error,
              stack: stack,
              library: 'Gennie: accesibilidad del sistema',
            ),
          );
        }
      },
    );
    return first.future;
  }

  @override
  void dispose() {
    _disposed = true;
    unawaited(_subscription?.cancel());
    super.dispose();
  }
}
