import 'package:flutter/material.dart';
import 'backend.dart';
import 'glass.dart';

// El círculo de estado representa residencia en memoria, no preferencias persistidas.
class ModelControls extends StatefulWidget {
  const ModelControls({super.key, required this.state});
  final DesktopState state;
  @override
  State<ModelControls> createState() => _ModelControlsState();
}

class _ModelControlsState extends State<ModelControls> {
  final _pending = <LocalModel>{};
  final _memory = <LocalModel, String>{};
  final _memoryGeneration = <LocalModel, int>{};

  Future<void> _readMemory(LocalModel model) async {
    final generation = (_memoryGeneration[model] ?? 0) + 1;
    _memoryGeneration[model] = generation;
    setState(() => _memory[model] = 'RAM …');
    int? bytes;
    try {
      bytes = await widget.state.backend.modelMemory(model);
    } catch (_) {
      /* La medición no está disponible. */
    }
    if (!mounted || _memoryGeneration[model] != generation) return;
    setState(() {
      _memory[model] = bytes == null
          ? 'RAM —'
          : bytes >= 1024 * 1024 * 1024
          ? '${(bytes / (1024 * 1024 * 1024)).toStringAsFixed(1)} GB RAM'
          : '${(bytes / (1024 * 1024)).toStringAsFixed(0)} MB RAM';
    });
  }

  Future<void> _toggle(ModelInfo info) async {
    if (_pending.contains(info.model)) return;
    setState(() => _pending.add(info.model));
    await widget.state.perform(
      () => widget.state.backend.setModelLoaded(
        info.model,
        info.state != ModelState.ready,
      ),
    );
    if (mounted) setState(() => _pending.remove(info.model));
  }

  @override
  Widget build(BuildContext context) {
    final models = widget.state.snapshot?.models ?? const <ModelInfo>[];
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        for (final model in LocalModel.values) ...[
          if (model != LocalModel.values.first) const SizedBox(height: 6),
          _button(
            models.where((info) => info.model == model).firstOrNull ??
                ModelInfo(model: model, state: ModelState.unavailable),
          ),
        ],
      ],
    );
  }

  Widget _button(ModelInfo info) {
    final loaded = info.state == ModelState.ready;
    final busy =
        _pending.contains(info.model) ||
        info.state == ModelState.loading ||
        info.state == ModelState.downloading;
    final available =
        info.state != ModelState.unavailable &&
        info.state != ModelState.disabled;
    final (name, icon) = switch (info.model) {
      LocalModel.laya => ('Laya', Icons.psychology_outlined),
      LocalModel.voice => ('Voz · Kokoro', Icons.volume_up_outlined),
      LocalModel.dictation => ('Transcripción · Nemotron', Icons.mic_none),
    };
    final details = _details(
      info,
      _pending.contains(info.model),
      _memory[info.model] ?? 'RAM —',
    );
    return RailButton(
      key: ValueKey('model-${info.model.name}'),
      label: name,
      description: details.semanticLabel,
      details: details,
      icon: icon,
      asset: switch (info.model) {
        LocalModel.laya => 'assets/icons/brain.svg',
        LocalModel.voice => 'assets/icons/speaker.svg',
        LocalModel.dictation => 'assets/icons/microphone.svg',
      },
      onTooltipRequested: () => _readMemory(info.model),
      statusColor: details.statusColor,
      appearance: RailButtonAppearance.modelStatus,
      selected: loaded,
      toggled: loaded,
      busy: busy,
      onPressed: available && !busy ? () => _toggle(info) : null,
    );
  }
}

AppTooltipDetails _details(ModelInfo info, bool pending, String memory) {
  final idle = switch (info.unloadMinutes) {
    null => 'Sin liberación por inactividad.',
    0 => 'Se libera al terminar de usarlo.',
    final minutes => 'Se libera tras $minutes min de inactividad.',
  };
  final (stateLabel, color) = pending
      ? ('Preparando…', const Color(0xffd9a34a))
      : switch (info.state) {
          ModelState.ready => ('Encendido', const Color(0xff32b869)),
          ModelState.loading => ('Cargando…', const Color(0xffd9a34a)),
          ModelState.downloading => ('Descargando…', const Color(0xffd9a34a)),
          ModelState.failed => ('Fallo', const Color(0xffd9a34a)),
          ModelState.disabled => ('Desactivado', const Color(0xff858585)),
          ModelState.unavailable => ('No disponible', const Color(0xff858585)),
          ModelState.unloaded => ('Apagado', const Color(0xffe05b56)),
        };
  return AppTooltipDetails(
    title: switch (info.model) {
      LocalModel.laya => 'Laya',
      LocalModel.voice => 'Kokoro',
      LocalModel.dictation => 'Nemotron',
    },
    status: stateLabel,
    statusColor: color,
    memory: memory,
    message: info.state == ModelState.failed && !pending
        ? 'No se pudo cargar el modelo.'
        : null,
    footer:
        info.state == ModelState.unavailable ||
            info.state == ModelState.disabled
        ? null
        : idle,
  );
}
