import 'package:flutter/material.dart';
import 'backend.dart';
import 'glass.dart';

class SettingsDialog extends StatefulWidget {
  const SettingsDialog({super.key, required this.state});
  final DesktopState state;
  @override
  State<SettingsDialog> createState() => _SettingsDialogState();
}

class _SettingsDialogState extends State<SettingsDialog> {
  final _model = TextEditingController();
  final _shortcut = TextEditingController();
  String effort = 'low', voice = 'ef_dora';
  bool read = true, write = true, commands = true, actions = true;
  bool enabledVoice = true,
      enabledDictation = true,
      hold = true,
      reduced = false;
  double speed = 100, dictationMinutes = 10, voiceMinutes = 10;
  bool keepVoice = false, saving = false;
  String? microphone, error, confirmation;
  List<String> devices = [];
  int tab = 0;
  DesktopState get state => widget.state;
  @override
  void initState() {
    super.initState();
    final s = state.snapshot!;
    final a = s.assistant;
    if (a != null) {
      _model.text = a.model;
      effort = a.effort;
      read = a.readFiles;
      write = a.writeFiles;
      commands = a.runCommands;
      actions = a.localActions;
    }
    final v = s.voice;
    if (v != null) {
      voice = v.voice;
      enabledVoice = v.enabled;
      speed = v.speedPercent.toDouble();
      keepVoice = v.unloadMinutes == null;
      voiceMinutes = (v.unloadMinutes ?? 10).toDouble();
    }
    final d = s.dictation;
    if (d != null) {
      _shortcut.text = d.shortcut;
      enabledDictation = d.enabled;
      hold = d.hold;
      dictationMinutes = d.unloadMinutes.toDouble();
      microphone = d.microphone;
    }
    reduced = s.reducedMotion;
    state.addListener(_updated);
    state.backend
        .microphones()
        .then((values) {
          if (mounted) setState(() => devices = values);
        })
        .catchError((Object e) {
          if (mounted) setState(() => error = '$e');
        });
  }

  void _updated() {
    if (!mounted) return;
    final s = state.snapshot;
    setState(() {
      error = state.failure;
      if (saving && (s?.confirmation != null || error != null)) {
        saving = false;
        confirmation = s?.confirmation;
      }
    });
  }

  @override
  void dispose() {
    state.removeListener(_updated);
    _model.dispose();
    _shortcut.dispose();
    super.dispose();
  }

  VoiceSettings _voice() => VoiceSettings(
    enabled: enabledVoice,
    voice: voice,
    speedPercent: speed.round(),
    unloadMinutes: keepVoice ? null : voiceMinutes.round(),
  );
  Future<void> _save() async {
    setState(() {
      saving = true;
      error = null;
      confirmation = null;
    });
    final ok = await state.perform(() {
      if (tab == 0) {
        return state.backend.saveAssistant(
          AssistantSettings(
            model: _model.text.trim(),
            effort: effort,
            readFiles: read,
            writeFiles: write,
            runCommands: commands,
            localActions: actions,
          ),
        );
      }
      if (tab == 1) {
        return state.backend.saveDictation(
          DictationSettings(
            enabled: enabledDictation,
            shortcut: _shortcut.text.trim(),
            hold: hold,
            unloadMinutes: dictationMinutes.round(),
            microphone: microphone,
          ),
        );
      }
      if (tab == 2) return state.backend.saveVoice(_voice());
      return state.backend.motion(reduced);
    });
    if (!ok && mounted) {
      setState(() {
        saving = false;
        error = state.failure;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final s = state.snapshot!;
    final available = switch (tab) {
      0 => s.assistant != null,
      1 => s.dictation != null,
      2 => s.voice != null,
      _ => true,
    };
    return Dialog(
      backgroundColor: Colors.transparent,
      insetPadding: const EdgeInsets.all(24),
      child: Glass(
        padding: const EdgeInsets.all(24),
        child: SizedBox(
          width: 640,
          height: 530,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  const Expanded(
                    child: Text(
                      'Configuración',
                      style: TextStyle(
                        fontSize: 20,
                        fontWeight: FontWeight.w500,
                      ),
                    ),
                  ),
                  IconButton(
                    tooltip: 'Cerrar configuración',
                    onPressed: saving ? null : () => Navigator.pop(context),
                    icon: const Icon(Icons.close, size: 19),
                  ),
                ],
              ),
              const SizedBox(height: 16),
              Wrap(
                spacing: 6,
                children: ['Asistente', 'Dictado', 'Voz de salida', 'General']
                    .indexed
                    .map(
                      (entry) => ChoiceChip(
                        label: Text(entry.$2),
                        selected: tab == entry.$1,
                        onSelected: saving
                            ? null
                            : (_) => setState(() {
                                tab = entry.$1;
                                error = null;
                                confirmation = null;
                              }),
                      ),
                    )
                    .toList(),
              ),
              const SizedBox(height: 20),
              Expanded(
                child: !available
                    ? const Center(child: Text('Este módulo está desactivado'))
                    : SingleChildScrollView(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: switch (tab) {
                            0 => [
                              Row(
                                children: [
                                  Expanded(child: Text(s.connection)),
                                  TextButton(
                                    onPressed: () =>
                                        state.perform(state.backend.connect),
                                    child: const Text('Conectar'),
                                  ),
                                ],
                              ),
                              TextField(
                                controller: _model,
                                decoration: const InputDecoration(
                                  labelText: 'Modelo',
                                ),
                              ),
                              const SizedBox(height: 12),
                              DropdownButtonFormField<String>(
                                initialValue: effort,
                                decoration: const InputDecoration(
                                  labelText: 'Esfuerzo de razonamiento',
                                ),
                                items:
                                    [
                                          'low',
                                          'medium',
                                          'high',
                                          'xhigh',
                                          'max',
                                          'ultra',
                                        ]
                                        .map(
                                          (v) => DropdownMenuItem(
                                            value: v,
                                            child: Text(v),
                                          ),
                                        )
                                        .toList(),
                                onChanged: (v) => setState(() => effort = v!),
                              ),
                              _toggle('Leer archivos', read, (v) {
                                read = v;
                                if (!v) write = false;
                              }),
                              _toggle('Escribir archivos', write, (v) {
                                write = v;
                                if (v) read = true;
                              }),
                              _toggle('Ejecutar comandos', commands, (v) {
                                commands = v;
                                if (!v) actions = false;
                              }),
                              _toggle(
                                'Controlar aplicaciones locales',
                                actions,
                                (v) {
                                  actions = v;
                                  if (v) commands = true;
                                },
                              ),
                              const Text(
                                'La búsqueda web está activa. La sesión pertenece a Codex CLI.',
                                style: TextStyle(
                                  fontSize: 12,
                                  color: Color(0xff858585),
                                ),
                              ),
                            ],
                            1 => [
                              _toggle(
                                'Activar dictado',
                                enabledDictation,
                                (v) => enabledDictation = v,
                              ),
                              TextField(
                                controller: _shortcut,
                                decoration: const InputDecoration(
                                  labelText: 'Atajo',
                                  helperText: 'Por ejemplo: Ctrl+Option+Space',
                                ),
                              ),
                              const SizedBox(height: 12),
                              _toggle(
                                'Mantener pulsado para hablar',
                                hold,
                                (v) => hold = v,
                              ),
                              DropdownButtonFormField<String>(
                                initialValue: microphone ?? '',
                                decoration: const InputDecoration(
                                  labelText: 'Micrófono',
                                ),
                                items: <String>{'', ...devices, ?microphone}
                                    .map(
                                      (v) => DropdownMenuItem(
                                        value: v,
                                        child: Text(
                                          v.isEmpty
                                              ? 'Predeterminado del sistema'
                                              : v,
                                          overflow: TextOverflow.ellipsis,
                                        ),
                                      ),
                                    )
                                    .toList(),
                                onChanged: (v) => setState(
                                  () => microphone = v == '' ? null : v,
                                ),
                              ),
                              const SizedBox(height: 16),
                              Text(
                                'Liberar modelo tras ${dictationMinutes.round()} min',
                              ),
                              Slider(
                                value: dictationMinutes,
                                min: 0,
                                max: 10,
                                divisions: 10,
                                onChanged: (v) =>
                                    setState(() => dictationMinutes = v),
                              ),
                              Text(
                                s.dictationStatus,
                                style: const TextStyle(fontSize: 12),
                              ),
                              Wrap(
                                spacing: 8,
                                children: [
                                  TextButton(
                                    onPressed: () =>
                                        state.perform(state.backend.authorize),
                                    child: const Text('Activar atajo'),
                                  ),
                                  TextButton(
                                    onPressed: () =>
                                        state.perform(state.backend.prepare),
                                    child: const Text('Preparar modelo'),
                                  ),
                                  TextButton(
                                    onPressed: () =>
                                        state.perform(state.backend.release),
                                    child: const Text('Liberar modelo'),
                                  ),
                                ],
                              ),
                            ],
                            2 => [
                              _toggle(
                                'Leer respuestas en voz alta',
                                enabledVoice,
                                (v) => enabledVoice = v,
                              ),
                              DropdownButtonFormField<String>(
                                initialValue: voice,
                                decoration: const InputDecoration(
                                  labelText: 'Voz',
                                ),
                                items: const [
                                  DropdownMenuItem(
                                    value: 'ef_dora',
                                    child: Text('Dora'),
                                  ),
                                  DropdownMenuItem(
                                    value: 'em_alex',
                                    child: Text('Alex'),
                                  ),
                                  DropdownMenuItem(
                                    value: 'em_santa',
                                    child: Text('Santa'),
                                  ),
                                ],
                                onChanged: (v) => setState(() => voice = v!),
                              ),
                              const SizedBox(height: 16),
                              Text('Velocidad: ${speed.round()} %'),
                              Slider(
                                value: speed,
                                min: 70,
                                max: 130,
                                divisions: 60,
                                onChanged: (v) => setState(() => speed = v),
                              ),
                              _toggle(
                                'Mantener el modelo cargado',
                                keepVoice,
                                (v) => keepVoice = v,
                              ),
                              if (!keepVoice) ...[
                                Text(
                                  'Liberar tras ${voiceMinutes.round()} min',
                                ),
                                Slider(
                                  value: voiceMinutes,
                                  min: 1,
                                  max: 120,
                                  divisions: 119,
                                  onChanged: (v) =>
                                      setState(() => voiceMinutes = v),
                                ),
                              ],
                              TextButton(
                                onPressed: () => state.perform(
                                  () => state.backend.previewVoice(_voice()),
                                ),
                                child: const Text('Probar voz sin guardar'),
                              ),
                            ],
                            _ => [
                              _toggle(
                                'Reducir movimiento',
                                reduced,
                                (v) => reduced = v,
                              ),
                              const SizedBox(height: 16),
                              const Text(
                                'Gennie',
                                style: TextStyle(fontSize: 18),
                              ),
                              const Text(
                                'Asistente personal · 0.1.0',
                                style: TextStyle(fontSize: 12),
                              ),
                            ],
                          },
                        ),
                      ),
              ),
              if (error != null)
                Padding(
                  padding: const EdgeInsets.only(top: 12),
                  child: Text(
                    error!,
                    style: TextStyle(
                      fontSize: 12,
                      color: Theme.of(context).colorScheme.error,
                    ),
                  ),
                ),
              if (confirmation != null)
                Padding(
                  padding: const EdgeInsets.only(top: 12),
                  child: Text(
                    confirmation!,
                    style: const TextStyle(fontSize: 12),
                  ),
                ),
              const SizedBox(height: 16),
              Align(
                alignment: Alignment.centerRight,
                child: FilledButton(
                  onPressed: saving || !available ? null : _save,
                  child: Text(saving ? 'Guardando…' : 'Guardar cambios'),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _toggle(String label, bool value, void Function(bool) change) =>
      SwitchListTile.adaptive(
        contentPadding: EdgeInsets.zero,
        title: Text(label, style: const TextStyle(fontSize: 13)),
        value: value,
        onChanged: saving ? null : (v) => setState(() => change(v)),
      );
}
