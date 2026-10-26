import 'dart:async';
import 'dart:io';
import 'package:flutter/foundation.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'rust/api.dart' as api;
import 'rust/frb_generated.dart';
export 'rust/api.dart'
    show
        Snapshot,
        AssignmentRecord,
        LocalModel,
        ModelState,
        ModelInfo,
        Conversation,
        Message,
        AssistantSettings,
        DictationSettings,
        VoiceSettings;

abstract class DesktopBackend {
  Future<api.Snapshot> initialize();
  Stream<api.Snapshot> watch();
  Future<api.Snapshot> select(String? id);
  Future<api.Snapshot> send(String text);
  Future<api.Snapshot> newChat();
  Future<api.Snapshot> connect();
  Future<api.Snapshot> saveAssignment(
    String? id,
    int? revision,
    String requestKey,
    String document,
  );
  Future<api.Snapshot> rename(String id, String title);
  Future<api.Snapshot> delete(String id);
  Future<api.Snapshot> saveAssistant(api.AssistantSettings settings);
  Future<api.Snapshot> saveVoice(api.VoiceSettings settings);
  Future<api.Snapshot> previewVoice(api.VoiceSettings settings);
  Future<api.Snapshot> saveDictation(api.DictationSettings settings);
  Future<api.Snapshot> motion(bool value);
  Future<int?> modelMemory(api.LocalModel model);
  Future<api.Snapshot> setModelLoaded(api.LocalModel model, bool loaded);
  Future<api.Snapshot> prepare();
  Future<api.Snapshot> release();
  Future<api.Snapshot> authorize();
  Future<List<String>> microphones();
  Future<void> shutdown();
}

class RustBackend implements DesktopBackend {
  bool _loaded = false;
  @override
  Future<api.Snapshot> initialize() async {
    const library = String.fromEnvironment('GENNIE_BACKEND');
    const helper = String.fromEnvironment('GENNIE_HELPER');
    const envFile = String.fromEnvironment('GENNIE_ENV');
    final executable = File(Platform.resolvedExecutable).parent;
    final bundledLibrary =
        '${executable.parent.path}/Frameworks/libgennie_backend.dylib';
    if (!_loaded) {
      await RustLib.init(
        externalLibrary: ExternalLibrary.open(
          library.isEmpty ? bundledLibrary : library,
        ),
      );
      _loaded = true;
    }
    final config = envFile;
    return api.initialize(
      envFile: config,
      helperPath: helper.isEmpty ? '${executable.path}/gennie-helper' : helper,
    );
  }

  @override
  Stream<api.Snapshot> watch() => api.watch();
  @override
  Future<api.Snapshot> select(String? id) => api.selectConversation(id: id);
  @override
  Future<api.Snapshot> send(String text) => api.sendMessage(text: text);
  @override
  Future<api.Snapshot> newChat() => api.newChat();
  @override
  Future<api.Snapshot> connect() => api.connect();
  @override
  Future<api.Snapshot> saveAssignment(
    String? id,
    int? revision,
    String requestKey,
    String document,
  ) => api.saveAssignment(
    id: id,
    expectedRevision: revision,
    requestKey: requestKey,
    document: document,
  );
  @override
  Future<api.Snapshot> rename(String id, String title) =>
      api.renameConversation(id: id, title: title);
  @override
  Future<api.Snapshot> delete(String id) => api.deleteConversation(id: id);
  @override
  Future<api.Snapshot> saveAssistant(api.AssistantSettings settings) =>
      api.saveAssistant(settings: settings);
  @override
  Future<api.Snapshot> saveVoice(api.VoiceSettings settings) =>
      api.saveVoice(settings: settings);
  @override
  Future<api.Snapshot> previewVoice(api.VoiceSettings settings) =>
      api.previewVoice(settings: settings);
  @override
  Future<api.Snapshot> saveDictation(api.DictationSettings settings) =>
      api.saveDictation(settings: settings);
  @override
  Future<api.Snapshot> motion(bool value) =>
      api.saveReducedMotion(value: value);
  @override
  Future<int?> modelMemory(api.LocalModel model) async =>
      (await api.modelMemoryBytes(model: model))?.toInt();
  @override
  Future<api.Snapshot> setModelLoaded(api.LocalModel model, bool loaded) =>
      api.setModelLoaded(model: model, loaded: loaded);
  @override
  Future<api.Snapshot> prepare() => api.prepareDictation();
  @override
  Future<api.Snapshot> release() => api.releaseDictation();
  @override
  Future<api.Snapshot> authorize() => api.authorizeShortcut();
  @override
  Future<List<String>> microphones() => api.microphones();
  @override
  Future<void> shutdown() => api.shutdown();
}

class DesktopState extends ChangeNotifier {
  DesktopState(this.backend);
  final DesktopBackend backend;
  api.Snapshot? snapshot;
  String? failure;
  bool loading = true;
  StreamSubscription<api.Snapshot>? _events;
  Future<void> start() async {
    try {
      snapshot = await backend.initialize();
      _events = backend.watch().listen(
        accept,
        onError: (Object error) {
          failure = '$error';
          notifyListeners();
        },
      );
    } catch (error) {
      failure = '$error';
    }
    loading = false;
    notifyListeners();
  }

  void accept(api.Snapshot value) {
    snapshot = value;
    failure = value.error;
    notifyListeners();
  }

  Future<bool> perform(Future<api.Snapshot> Function() action) async {
    try {
      accept(await action());
      return true;
    } catch (error) {
      failure = '$error';
      notifyListeners();
      return false;
    }
  }

  Future<void> close() async {
    // FRB termina el stream al liberar sus sinks durante shutdown.
    // Esperar cancel antes bloquearía el cierre del backend.
    final cancellation = _events?.cancel();
    _events = null;
    await backend.shutdown();
    await cancellation;
  }

  @override
  void dispose() {
    unawaited(_events?.cancel());
    super.dispose();
  }
}
