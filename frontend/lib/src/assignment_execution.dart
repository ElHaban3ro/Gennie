import 'dart:async';
import 'dart:convert';
import 'package:flutter/material.dart';
import 'backend.dart';
import 'plugin_parameter.dart';
import 'plugins.dart' show mcpSimpleParameters;
import 'review_dialog.dart';

Future<Map<String, dynamic>> _call(
  DesktopState state,
  Map<String, dynamic> request,
) async => Map<String, dynamic>.from(
  jsonDecode(
        await (state.backend as ExecutionBackend).executionRequest(
          jsonEncode(request),
        ),
      )
      as Map,
);
String _date(Object? value) {
  final d = DateTime.tryParse(value?.toString() ?? '')?.toLocal();
  return d == null
      ? 'Sin próxima ejecución'
      : '${d.day.toString().padLeft(2, '0')}/${d.month.toString().padLeft(2, '0')}/${d.year} ${d.hour.toString().padLeft(2, '0')}:${d.minute.toString().padLeft(2, '0')}';
}

const _states = {
  'active': 'Activa',
  'paused': 'Pausada',
  'stale': 'El encargo cambió',
  'review_required': 'Resultado por revisar',
  'completed': 'Finalizada',
  'reserved': 'Preparando',
  'running': 'En curso',
  'succeeded': 'La función informó éxito',
  'rejected': 'No se envió la operación',
  'uncertain': 'Resultado sin confirmar',
  'cancelled': 'Omitida antes del envío',
  'skipped': 'Omitida por retraso o vencimiento',
};
void _reveal(GlobalKey key) {
  WidgetsBinding.instance.addPostFrameCallback((_) {
    final context = key.currentContext;
    if (context != null && context.mounted) {
      unawaited(Scrollable.ensureVisible(context, alignment: .5));
    }
  });
}

class _NoticeDecision extends StatelessWidget {
  const _NoticeDecision({
    required this.value,
    required this.busy,
    required this.refresh,
  });
  final Map? value;
  final bool busy;
  final VoidCallback refresh;
  @override
  Widget build(BuildContext context) {
    final action = value?['action'];
    final pending =
        value == null || ['pending', 'rendering'].contains(value?['state']);
    final text = switch (action) {
      'record' => 'Resultado guardado sin aviso',
      'notify' => 'Gennie preparó un aviso',
      'ask' => 'Gennie preparó una pregunta',
      'offer_help' => 'Gennie preparó una propuesta de ayuda',
      _ =>
        pending
            ? 'La valoración todavía no está confirmada'
            : 'Sin decisión contextual confirmada',
    };
    final guide = value?['guide_state'];
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Text(
            'Criterio de aviso',
            style: TextStyle(fontWeight: FontWeight.w600),
          ),
          Text(text),
          if (value?['failure_code'] == null && value?['reason'] is String)
            Text(value!['reason'] as String),
          if (action == 'record' &&
              (['failed', 'interrupted', 'unavailable'].contains(guide) ||
                  value?['failure_code'] != null))
            const Text(
              'La valoración no quedó confirmada. Se conserva el resultado sin repetir la consulta ni enviar un aviso contextual.',
            ),
          if (guide == 'bypassed')
            const Text(
              'Los resultados fallidos o inciertos se comunican por la vía habitual, sin filtrarlos por la guía.',
            ),
          if (value?['state'] == 'ready')
            const Text(
              'El mensaje está preparado y su entrega al chat sigue pendiente.',
            ),
          if (value?['state'] == 'discarded' && action != 'record')
            const Text(
              'El chat de destino ya no estaba disponible; el resultado sigue guardado.',
            ),
          if (pending)
            TextButton(
              onPressed: busy ? null : refresh,
              child: const Text('Actualizar valoración'),
            ),
        ],
      ),
    );
  }
}

class AssignmentExecution extends StatefulWidget {
  const AssignmentExecution({
    super.key,
    required this.state,
    required this.record,
    this.onOpenPlugin,
  });
  final DesktopState state;
  final AssignmentRecord record;
  final ValueChanged<String>? onOpenPlugin;
  @override
  State<AssignmentExecution> createState() => _AssignmentExecutionState();
}

class _AssignmentExecutionState extends State<AssignmentExecution> {
  List<Map<String, dynamic>> policies = [];
  bool loading = false, active = false, storageFailed = false;
  String? error, revision;
  int generation = 0;
  int noticesPending = 0;
  @override
  void initState() {
    super.initState();
    widget.state.addListener(changed);
    unawaited(load());
  }

  @override
  void didUpdateWidget(AssignmentExecution old) {
    super.didUpdateWidget(old);
    if (old.record.revision != widget.record.revision ||
        old.record.instanceId != widget.record.instanceId) {
      unawaited(load());
    }
  }

  @override
  void dispose() {
    generation++;
    widget.state.removeListener(changed);
    super.dispose();
  }

  void changed() {
    final next = widget.state.snapshot?.executionRevision;
    if (next != revision) {
      revision = next;
      unawaited(load());
    }
  }

  Future<void> load() async {
    final request = ++generation;
    setState(() {
      loading = true;
      error = null;
    });
    try {
      final result = await _call(widget.state, {
        'action': 'list',
        'assignment_id': int.parse(widget.record.id),
      });
      if (!mounted || request != generation) return;
      setState(() {
        policies = (result['policies'] as List)
            .map((p) => Map<String, dynamic>.from(p as Map))
            .toList();
        noticesPending = result['notices_pending'] as int? ?? 0;
        active = result['runner_active'] == true;
        storageFailed = result['storage_failed'] == true;
        loading = false;
      });
    } catch (e) {
      if (mounted && request == generation) {
        setState(() {
          error = 'No se pudo consultar la ejecución: $e';
          loading = false;
        });
      }
    }
  }

  Future<void> configure(Map<String, dynamic>? policy) async {
    await showDialog<void>(
      context: context,
      barrierDismissible: false,
      builder: (_) => _ExecutionDialog(
        state: widget.state,
        record: widget.record,
        policy: policy,
        occupiedTriggers: policies
            .map((p) => p['configuration']['trigger'] as String)
            .toSet(),
      ),
    );
    if (mounted) await load();
  }

  Future<void> changePolicy(Map<String, dynamic> policy, String action) async {
    if (loading) return;
    setState(() => loading = true);
    try {
      await _call(widget.state, {
        'action': action,
        'policy_id': policy['id'],
        'expected_revision': policy['revision'],
      });
      if (mounted) await load();
    } catch (e) {
      if (mounted) {
        setState(() {
          error = action == 'revoke_guide'
              ? 'No se pudo retirar el consentimiento: $e'
              : 'No se pudo pausar: $e';
          loading = false;
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) => Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [
      const SizedBox(height: 12),
      const Text(
        'Ejecución periódica',
        style: TextStyle(fontWeight: FontWeight.w600),
      ),
      if (!active)
        const Text(
          'Activa Plugins para habilitar el ejecutor. Puedes consultar su actividad y pausar configuraciones existentes.',
        ),
      if (noticesPending > 0)
        const Text(
          'Hay resultados guardados cuya entrega al chat aún no está confirmada.',
        ),
      if (storageFailed)
        Text(
          'La ejecución se detuvo al guardar su estado. Revisa la actividad antes de continuar.',
          style: TextStyle(color: Theme.of(context).colorScheme.error),
        ),
      Wrap(
        spacing: 8,
        runSpacing: 8,
        children: [
          TextButton(
            onPressed:
                !active || loading || widget.state.backend is! PluginBackend
                ? null
                : () {
                    final doc = jsonDecode(widget.record.document) as Map;
                    final available = [
                      if (doc['recurrence'] != null || doc['notify_at'] != null)
                        'delivery',
                      if (doc['preparation']?['interval_minutes'] != null)
                        'preparation',
                    ];
                    final existing = policies
                        .where(
                          (p) =>
                              available.contains(p['configuration']['trigger']),
                        )
                        .toList();
                    configure(
                      existing.length == available.length && existing.isNotEmpty
                          ? existing.first
                          : null,
                    );
                  },
            child: const Text('Configurar ejecución'),
          ),
          TextButton(
            onPressed: loading ? null : load,
            child: const Text('Actualizar actividad'),
          ),
        ],
      ),
      if (loading) const LinearProgressIndicator(),
      if (error != null)
        Text(
          error!,
          style: TextStyle(color: Theme.of(context).colorScheme.error),
        ),
      for (final policy in policies)
        Padding(
          padding: const EdgeInsets.symmetric(vertical: 8),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                '${policy['configuration']['trigger'] == 'delivery' ? 'Entrega' : 'Preparación'} · ${_states[policy['state']] ?? policy['state']}',
              ),
              if (policy['current'] != true)
                const Text(
                  'Vuelve a configurar con los datos actuales del encargo.',
                ),
              Text(
                'Próxima (hora de este equipo): ${_date(policy['next_at'])}',
              ),
              if (policy['configuration']['contextual_notice'] == true)
                const Text(
                  'Guía contextual autorizada. Al desactivarla, las futuras observaciones quedarán solo en Actividad. Una consulta en curso puede terminar.',
                ),
              if (policy['running'] == true)
                const Text(
                  'Hay una operación en curso. Pausar evita las siguientes; no deshace lo ya enviado.',
                ),
              Wrap(
                spacing: 8,
                runSpacing: 8,
                children: [
                  TextButton(
                    onPressed: !active || loading || policy['running'] == true
                        ? null
                        : () => configure(policy),
                    child: const Text('Revisar configuración'),
                  ),
                  if (policy['enabled'] == true)
                    TextButton(
                      onPressed: loading
                          ? null
                          : () => changePolicy(policy, 'pause'),
                      child: const Text('Pausar ejecuciones'),
                    ),
                  if (policy['configuration']['contextual_notice'] == true)
                    TextButton(
                      onPressed: loading
                          ? null
                          : () => changePolicy(policy, 'revoke_guide'),
                      child: const Text('Desactivar guía y avisos'),
                    ),
                  TextButton(
                    onPressed: () => showDialog<void>(
                      context: context,
                      barrierDismissible: false,
                      builder: (_) => _RunsDialog(
                        state: widget.state,
                        policy: policy,
                        onOpenPlugin: widget.onOpenPlugin,
                      ),
                    ),
                    child: const Text('Ver ejecuciones'),
                  ),
                ],
              ),
            ],
          ),
        ),
    ],
  );
}

class _ExecutionDialog extends StatefulWidget {
  const _ExecutionDialog({
    required this.state,
    required this.record,
    this.policy,
    this.occupiedTriggers = const {},
  });
  final DesktopState state;
  final AssignmentRecord record;
  final Map<String, dynamic>? policy;
  final Set<String> occupiedTriggers;
  @override
  State<_ExecutionDialog> createState() => _ExecutionDialogState();
}

class _ExecutionDialogState extends State<_ExecutionDialog> {
  final form = GlobalKey<FormState>(), errorKey = GlobalKey();
  final triggerKey = GlobalKey(),
      functionKey = GlobalKey(),
      jsonKey = GlobalKey(),
      latenessKey = GlobalKey();
  final triggerFocus = FocusNode(),
      functionFocus = FocusNode(),
      jsonFocus = FocusNode(),
      latenessFocus = FocusNode();
  final fields = <String, TextEditingController>{};
  final choices = <String, Object?>{};
  final keys = <String, GlobalKey>{};
  final focuses = <String, FocusNode>{};
  final jsonArguments = TextEditingController(text: '{}'),
      lateness = TextEditingController(text: '5');
  late final Map document = Map.from(jsonDecode(widget.record.document) as Map);
  String? selected, error;
  String? trigger;
  String misfire = 'skip_missed', repeated = 'earlier', missing = 'skip';
  bool loading = true, busy = false, enabled = false, reportResults = false;
  bool contextualNotice = false;
  Map<String, dynamic>? noticeOrigin;
  String? noticeError;
  int generation = 0;
  int? sourceRevision;
  Map<String, dynamic> definitions = {};
  Map<String, dynamic>? parameters;
  Map<String, dynamic>? target;
  DateTime? until;
  @override
  void initState() {
    super.initState();
    final saved = widget.policy?['configuration'] as Map?;
    if (saved != null) {
      trigger = saved['trigger'] as String;
      misfire = saved['misfire'] as String;
      lateness.text = '${saved['max_lateness_minutes']}';
      until = DateTime.tryParse(saved['until']?.toString() ?? '')?.toLocal();
      enabled = widget.policy?['enabled'] == true;
      final delivery = saved['delivery'] as Map?;
      reportResults = delivery != null;
      contextualNotice = saved['contextual_notice'] == true;
      if (delivery != null) noticeOrigin = Map<String, dynamic>.from(delivery);
      final t = saved['target'] as Map;
      selected = (t['function'] ?? t['tool']) as String?;
      jsonArguments.text = const JsonEncoder.withIndent(
        '  ',
      ).convert(saved['arguments']);
      final dst = saved['calendar']['dst'] as Map?;
      repeated = dst?['repeated'] as String? ?? repeated;
      missing = dst?['missing'] as String? ?? missing;
    }
    final available = triggers;
    trigger = available.contains(trigger)
        ? trigger
        : (widget.policy == null && available.length == 1
              ? available.single
              : null);
    unawaited(load());
    unawaited(loadNoticeOrigin());
  }

  Future<void> loadNoticeOrigin() async {
    try {
      final value = await _call(widget.state, {'action': 'notice_origin'});
      if (mounted) setState(() => noticeOrigin ??= value);
    } catch (e) {
      if (mounted) setState(() => noticeError = e.toString());
    }
  }

  List<String> get triggers => [
    if ((document['recurrence'] != null || document['notify_at'] != null) &&
        (widget.policy != null ||
            !widget.occupiedTriggers.contains('delivery')))
      'delivery',
    if (document['preparation']?['interval_minutes'] != null &&
        (widget.policy != null ||
            !widget.occupiedTriggers.contains('preparation')))
      'preparation',
  ];

  @override
  void dispose() {
    generation++;
    for (final c in fields.values) {
      c.dispose();
    }
    for (final f in focuses.values) {
      f.dispose();
    }
    jsonArguments.dispose();
    lateness.dispose();
    triggerFocus.dispose();
    functionFocus.dispose();
    jsonFocus.dispose();
    latenessFocus.dispose();
    super.dispose();
  }

  Future<void> load() async {
    final request = ++generation;
    setState(() {
      loading = true;
      error = null;
    });
    try {
      final method = document['method'] as Map?;
      if (method == null) {
        throw StateError(
          'Define primero el método y la autorización del encargo.',
        );
      }
      final id = method['plugin_id'] as String;
      final backend = widget.state.backend as PluginBackend;
      final result = Map<String, dynamic>.from(
        jsonDecode(
              await backend.pluginRequest(
                jsonEncode(
                  id.startsWith('mcp:')
                      ? {'action': 'mcp_get', 'server_id': id.substring(4)}
                      : {'action': 'list'},
                ),
              ),
            )
            as Map,
      );
      if (!mounted || request != generation) return;
      if (id.startsWith('mcp:')) {
        final server = result['server'] as Map;
        sourceRevision = server['revision'] as int;
        definitions = Map<String, dynamic>.from(server['tools'] as Map);
      } else {
        final plugin = (result['plugins'] as List).cast<Map>().firstWhere(
          (p) => p['manifest']['id'] == id,
        );
        sourceRevision = plugin['revision'] as int;
        definitions = {
          for (final f in plugin['manifest']['functions'] as List)
            f['name'] as String: f,
        };
      }
      final allowed = List<String>.from(method['functions'] as List);
      definitions.removeWhere((name, _) => !allowed.contains(name));
      if (definitions.isEmpty) {
        throw StateError('No están disponibles las funciones del método.');
      }
      selected = definitions.containsKey(selected) ? selected : null;
      if (selected == null && definitions.length == 1) {
        selected = definitions.keys.single;
      }
      if (selected != null) select(selected!);
      setState(() => loading = false);
    } catch (e) {
      if (mounted && request == generation) {
        setState(() {
          loading = false;
          error = 'No se pudo preparar la función: $e';
        });
        _reveal(errorKey);
      }
    }
  }

  void select(String name) {
    selected = name;
    final id = document['method']['plugin_id'] as String;
    final definition = Map<String, dynamic>.from(definitions[name] as Map);
    parameters = id.startsWith('mcp:')
        ? mcpSimpleParameters(definition['inputSchema'] as Map)
        : Map<String, dynamic>.from(definition['parameters'] as Map? ?? {});
    target = id.startsWith('mcp:')
        ? {
            'kind': 'mcp',
            'server_id': id.substring(4),
            'tool': name,
            'revision': sourceRevision,
          }
        : {
            'kind': 'web',
            'plugin_id': id,
            'function': name,
            'revision': sourceRevision,
          };
    Map<String, dynamic> existing = {};
    try {
      existing = Map<String, dynamic>.from(
        jsonDecode(jsonArguments.text) as Map,
      );
    } catch (_) {
      /* Conserva el borrador JSON sin impedir cambiar de función. */
    }
    for (final c in fields.values) {
      c.dispose();
    }
    fields.clear();
    choices.clear();
    for (final f in focuses.values) {
      f.dispose();
    }
    focuses.clear();
    keys.clear();
    if (parameters != null) {
      for (final entry in parameters!.entries) {
        final p = PluginParameter(entry.key, entry.value as Object);
        keys[p.name] = GlobalKey();
        focuses[p.name] = FocusNode();
        if (p.options.isNotEmpty) {
          choices[p.name] = existing[p.name];
        } else {
          fields[p.name] = TextEditingController(
            text: existing[p.name]?.toString() ?? '',
          );
        }
      }
    }
  }

  Future<void> expiry() async {
    final now = DateTime.now();
    final date = await showDatePicker(
      context: context,
      initialDate: until != null && until!.isAfter(now)
          ? until!
          : now.add(const Duration(days: 30)),
      firstDate: now,
      lastDate: DateTime(now.year + 10),
    );
    if (date == null || !mounted) return;
    final time = await showTimePicker(
      context: context,
      initialTime: until == null
          ? TimeOfDay.now()
          : TimeOfDay.fromDateTime(until!),
    );
    if (time != null && mounted) {
      setState(
        () => until = DateTime(
          date.year,
          date.month,
          date.day,
          time.hour,
          time.minute,
        ),
      );
    }
  }

  Future<void> save() async {
    if (busy || loading) return;
    if (!form.currentState!.validate()) {
      for (final (key, focus) in [
        (triggerKey, triggerFocus),
        (functionKey, functionFocus),
        for (final name in keys.keys) (keys[name]!, focuses[name]!),
        (jsonKey, jsonFocus),
        (latenessKey, latenessFocus),
      ]) {
        final field = key.currentState;
        if (field is FormFieldState && field.hasError) {
          focus.requestFocus();
          _reveal(key);
          return;
        }
      }
      return;
    }
    if (trigger == null || target == null || widget.record.instanceId == null) {
      setState(() => error = 'Selecciona una función vigente.');
      _reveal(errorKey);
      return;
    }
    Map<String, dynamic> args;
    try {
      args = parameters == null
          ? Map<String, dynamic>.from(jsonDecode(jsonArguments.text) as Map)
          : {
              for (final entry in parameters!.entries)
                if ((choices[entry.key] ??
                        PluginParameter(
                          entry.key,
                          entry.value as Object,
                        ).parse(fields[entry.key]?.text ?? '')) !=
                    null)
                  entry.key:
                      choices[entry.key] ??
                      PluginParameter(
                        entry.key,
                        entry.value as Object,
                      ).parse(fields[entry.key]?.text ?? ''),
            };
    } catch (_) {
      setState(() => error = 'Los parámetros deben ser un objeto JSON válido.');
      _reveal(errorKey);
      return;
    }
    setState(() {
      busy = true;
      error = null;
    });
    try {
      await _call(widget.state, {
        'action': 'configure',
        'configuration': {
          'assignment': {
            'id': int.parse(widget.record.id),
            'instance_id': widget.record.instanceId,
            'revision': widget.record.revision,
          },
          'expected_policy_revision': widget.policy?['revision'] ?? 0,
          'trigger': trigger,
          'dst': {'repeated': repeated, 'missing': missing},
          'target': target,
          'arguments': args,
          'misfire': misfire,
          'max_lateness_minutes': int.parse(lateness.text),
          'until': until?.toUtc().toIso8601String(),
          'enabled': enabled,
          'delivery': reportResults ? noticeOrigin : null,
          'contextual_notice':
              contextualNotice &&
              reportResults &&
              trigger == 'preparation' &&
              document['follow_up'] != null,
        },
      });
      if (mounted) Navigator.pop(context);
    } catch (e) {
      if (mounted) {
        setState(() {
          busy = false;
          error = 'No se pudo guardar la ejecución: $e';
        });
        _reveal(errorKey);
      }
    }
  }

  @override
  Widget build(BuildContext context) => AppReviewDialog(
    title: 'Configurar ejecución',
    busy: busy,
    actions: [
      TextButton(
        onPressed: busy ? null : () => Navigator.pop(context),
        child: const Text('Cancelar'),
      ),
      FilledButton(
        onPressed: busy || loading ? null : save,
        child: Text(
          busy
              ? 'Guardando…'
              : enabled
              ? 'Activar ejecuciones'
              : 'Guardar pausada',
        ),
      ),
    ],
    child: Form(
      key: form,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            document['title'] as String,
            style: const TextStyle(fontWeight: FontWeight.w600),
          ),
          const SizedBox(height: 8),
          const Text(
            'Gennie ejecutará esta función mientras la aplicación siga abierta. Los resultados se guardarán en la actividad del encargo.',
          ),
          const SizedBox(height: 12),
          Text(
            'Alcance autorizado: ${document['authorization']?['scope'] ?? 'Sin definir'}',
          ),
          const SizedBox(height: 16),
          if (loading) const LinearProgressIndicator(),
          DropdownButtonFormField<String>(
            key: triggerKey,
            focusNode: triggerFocus,
            autovalidateMode: AutovalidateMode.onUserInteraction,
            initialValue: trigger,
            decoration: const InputDecoration(labelText: 'Cuándo ejecutar'),
            items: [
              if (triggers.contains('delivery'))
                const DropdownMenuItem(
                  value: 'delivery',
                  child: Text('Horario de entrega del encargo'),
                ),
              if (triggers.contains('preparation'))
                const DropdownMenuItem(
                  value: 'preparation',
                  child: Text('Cadencia de preparación'),
                ),
            ],
            onChanged: busy || widget.policy != null
                ? null
                : (v) => setState(() => trigger = v),
            validator: (v) => v == null ? 'Elige un horario.' : null,
          ),
          const SizedBox(height: 12),
          Text('Zona del horario: ${document['timezone']}'),
          if (definitions.isNotEmpty)
            DropdownButtonFormField<String>(
              key: functionKey,
              focusNode: functionFocus,
              autovalidateMode: AutovalidateMode.onUserInteraction,
              initialValue: selected,
              isExpanded: true,
              decoration: const InputDecoration(labelText: 'Función'),
              items: [
                for (final name in definitions.keys)
                  DropdownMenuItem(
                    value: name,
                    child: Text(
                      (definitions[name] as Map)['title']?.toString() ?? name,
                    ),
                  ),
              ],
              onChanged: busy ? null : (v) => setState(() => select(v!)),
              validator: (v) => v == null ? 'Elige una función.' : null,
            ),
          if (selected != null) ...[
            const SizedBox(height: 8),
            Text(
              (definitions[selected] as Map)['description']?.toString() ?? '',
            ),
            Text('Revisión de la función: $sourceRevision'),
            if (widget.policy != null &&
                widget.policy!['configuration']['target']['revision'] !=
                    sourceRevision)
              const Text(
                'La función cambió desde la configuración anterior. Revisa sus parámetros antes de guardarla.',
              ),
          ],
          if (selected != null && parameters != null)
            for (final entry in parameters!.entries)
              Padding(
                padding: const EdgeInsets.only(top: 12),
                child: Builder(
                  builder: (context) {
                    final p = PluginParameter(entry.key, entry.value as Object);
                    return p.options.isNotEmpty
                        ? DropdownButtonFormField<Object>(
                            key: keys[p.name],
                            focusNode: focuses[p.name],
                            autovalidateMode:
                                AutovalidateMode.onUserInteraction,
                            initialValue: choices[p.name],
                            isExpanded: true,
                            decoration: InputDecoration(
                              labelText: p.label,
                              helperText: p.description,
                              helperMaxLines: 4,
                            ),
                            items: [
                              if (!p.isRequired)
                                const DropdownMenuItem(
                                  value: null,
                                  child: Text('Sin valor'),
                                ),
                              for (final value in p.options)
                                DropdownMenuItem(
                                  value: value,
                                  child: Text(p.display(value)),
                                ),
                            ],
                            onChanged: busy
                                ? null
                                : (value) =>
                                      setState(() => choices[p.name] = value),
                            validator: p.validate,
                          )
                        : TextFormField(
                            key: keys[p.name],
                            focusNode: focuses[p.name],
                            autovalidateMode:
                                AutovalidateMode.onUserInteraction,
                            controller: fields[p.name],
                            enabled: !busy,
                            decoration: InputDecoration(
                              labelText: p.label,
                              helperText: p.description,
                              helperMaxLines: 4,
                            ),
                            validator: (v) => p.validate(p.parse(v ?? '')),
                          );
                  },
                ),
              ),
          if (selected != null && parameters == null)
            Padding(
              padding: const EdgeInsets.only(top: 12),
              child: TextFormField(
                key: jsonKey,
                focusNode: jsonFocus,
                autovalidateMode: AutovalidateMode.onUserInteraction,
                controller: jsonArguments,
                enabled: !busy,
                minLines: 3,
                maxLines: 8,
                decoration: const InputDecoration(
                  labelText: 'Parámetros (JSON avanzado)',
                ),
                validator: (v) {
                  try {
                    if (jsonDecode(v ?? '') is Map) return null;
                  } catch (_) {}
                  return 'Introduce un objeto JSON.';
                },
              ),
            ),
          const SizedBox(height: 16),
          DropdownButtonFormField<String>(
            initialValue: misfire,
            isExpanded: true,
            decoration: const InputDecoration(
              labelText: 'Si se pierde un horario',
            ),
            items: const [
              DropdownMenuItem(
                value: 'skip_missed',
                child: Text('Omitir pendientes antiguos'),
              ),
              DropdownMenuItem(
                value: 'run_latest',
                child: Text('Ejecutar solo el pendiente más reciente'),
              ),
            ],
            onChanged: busy ? null : (v) => setState(() => misfire = v!),
          ),
          const SizedBox(height: 12),
          TextFormField(
            key: latenessKey,
            focusNode: latenessFocus,
            autovalidateMode: AutovalidateMode.onUserInteraction,
            controller: lateness,
            enabled: !busy,
            keyboardType: TextInputType.number,
            decoration: const InputDecoration(
              labelText: 'Retraso máximo en minutos',
              helperText: 'Pasado este plazo se omite la ocurrencia.',
            ),
            validator: (v) {
              final n = int.tryParse(v ?? '');
              return n == null || n < 1 || n > 1440
                  ? 'Usa entre 1 y 1440 minutos.'
                  : null;
            },
          ),
          const SizedBox(height: 8),
          Wrap(
            spacing: 8,
            children: [
              TextButton(
                onPressed: busy ? null : expiry,
                child: Text(
                  until == null
                      ? 'Elegir vencimiento (opcional)'
                      : 'Vence: ${_date(until!.toIso8601String())} · hora de este equipo',
                ),
              ),
              if (until != null)
                TextButton(
                  onPressed: busy ? null : () => setState(() => until = null),
                  child: const Text('Sin vencimiento'),
                ),
            ],
          ),
          ExpansionTile(
            title: const Text('Cambios de horario de verano'),
            tilePadding: EdgeInsets.zero,
            children: [
              DropdownButtonFormField<String>(
                initialValue: repeated,
                decoration: const InputDecoration(
                  labelText: 'Si una hora se repite',
                ),
                items: const [
                  DropdownMenuItem(
                    value: 'earlier',
                    child: Text('Una vez, en la primera hora'),
                  ),
                  DropdownMenuItem(
                    value: 'later',
                    child: Text('Una vez, en la segunda hora'),
                  ),
                ],
                onChanged: busy ? null : (v) => setState(() => repeated = v!),
              ),
              DropdownButtonFormField<String>(
                initialValue: missing,
                decoration: const InputDecoration(
                  labelText: 'Si una hora no existe',
                ),
                items: const [
                  DropdownMenuItem(
                    value: 'skip',
                    child: Text('Omitir esa ocurrencia'),
                  ),
                  DropdownMenuItem(
                    value: 'shift_forward',
                    child: Text('Desplazarla hacia adelante'),
                  ),
                ],
                onChanged: busy ? null : (v) => setState(() => missing = v!),
              ),
            ],
          ),
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            title: const Text('Recibir resultados en el chat de Gennie'),
            subtitle: Text(
              noticeOrigin == null
                  ? (noticeError ?? 'Consultando el chat de destino…')
                  : contextualNotice &&
                        trigger == 'preparation' &&
                        document['follow_up'] != null
                  ? 'Gennie valorará si conviene avisarte, preguntar o proponer ayuda. Las ejecuciones ya iniciadas pueden informar después de pausar.'
                  : 'Recibirás un aviso por cada ocurrencia. Gennie usará el resultado y el contexto de la conversación de destino con su modelo configurado. Las ejecuciones ya iniciadas pueden informar después de pausar.',
            ),
            value: reportResults,
            onChanged: busy || (!reportResults && noticeOrigin == null)
                ? null
                : (v) => setState(() => reportResults = v),
          ),
          if (trigger == 'preparation' && document['follow_up'] != null)
            SwitchListTile(
              contentPadding: EdgeInsets.zero,
              title: const Text('Valorar cuándo avisarme con la guía remota'),
              subtitle: const Text(
                'Autorizas consultas periódicas a OpenRouter con los criterios del encargo, el resultado, valoraciones anteriores de esta configuración y contexto de la conversación de destino. La guía aconseja y Gennie decide; puede guardar sin avisar. No autoriza acciones nuevas. Si falla, conserva ese resultado en Actividad sin aviso contextual ni otra valoración de ese intento.',
              ),
              value: contextualNotice,
              onChanged: busy || !reportResults
                  ? null
                  : (v) => setState(() => contextualNotice = v),
            ),
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            title: const Text('Habilitar ejecución periódica'),
            subtitle: const Text(
              'Cada ocurrencia usará esta función, parámetros y alcance. Un resultado incierto detendrá las siguientes hasta su revisión.',
            ),
            value: enabled,
            onChanged: busy ? null : (v) => setState(() => enabled = v),
          ),
          if (error != null)
            Text(
              error!,
              key: errorKey,
              style: TextStyle(color: Theme.of(context).colorScheme.error),
            ),
        ],
      ),
    ),
  );
}

class _RunsDialog extends StatefulWidget {
  const _RunsDialog({
    required this.state,
    required this.policy,
    this.onOpenPlugin,
  });
  final DesktopState state;
  final Map<String, dynamic> policy;
  final ValueChanged<String>? onOpenPlugin;
  @override
  State<_RunsDialog> createState() => _RunsDialogState();
}

class _RunsDialogState extends State<_RunsDialog> {
  final runs = <Map<String, dynamic>>[];
  bool busy = false;
  String? error;
  int? before;
  @override
  void initState() {
    super.initState();
    unawaited(load());
  }

  Future<void> load() async {
    if (busy) return;
    setState(() => busy = true);
    try {
      final result = await _call(widget.state, {
        'action': 'runs',
        'policy_id': widget.policy['id'],
        'before': before,
      });
      if (mounted) {
        setState(() {
          runs.addAll(
            (result['runs'] as List).map(
              (r) => Map<String, dynamic>.from(r as Map),
            ),
          );
          before = result['next_cursor'] as int?;
          busy = false;
          error = null;
        });
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          error = 'No se pudo cargar la actividad: $e';
          busy = false;
        });
      }
    }
  }

  Future<void> review(Map<String, dynamic> summary) async {
    await showExecutionRun(
      context,
      widget.state,
      summary['id'] as int,
      onOpenPlugin: widget.onOpenPlugin == null
          ? null
          : (id) {
              Navigator.pop(context);
              widget.onOpenPlugin!(id);
            },
    );
    if (mounted) {
      setState(() {
        runs.clear();
        before = null;
      });
      await load();
    }
  }

  @override
  Widget build(BuildContext context) => AppReviewDialog(
    title: 'Actividad del encargo',
    busy: busy,
    actions: [
      TextButton(
        onPressed: busy ? null : () => Navigator.pop(context),
        child: const Text('Cerrar'),
      ),
    ],
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        if (runs.isEmpty && !busy) const Text('Todavía no hay ejecuciones.'),
        if (busy) const LinearProgressIndicator(),
        if (error != null)
          Text(
            error!,
            style: TextStyle(color: Theme.of(context).colorScheme.error),
          ),
        if (error != null)
          TextButton(
            onPressed: busy ? null : load,
            child: const Text('Reintentar actividad'),
          ),
        for (final run in runs)
          Padding(
            padding: const EdgeInsets.only(bottom: 16),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  '${_date(run['scheduled_at'])} · ${_states[run['state']] ?? run['state']}',
                ),
                TextButton(
                  onPressed: busy ? null : () => review(run),
                  child: Text(
                    run['state'] == 'uncertain' && run['resolution'] == null
                        ? 'Revisar resultado'
                        : 'Ver detalle',
                  ),
                ),
                if (run['resolution'] != null)
                  Text(
                    run['resolution'] == 'applied'
                        ? 'Comprobado: se aplicó'
                        : 'Comprobado: no se aplicó',
                  ),
              ],
            ),
          ),
        if (before != null)
          TextButton(
            onPressed: busy ? null : load,
            child: const Text('Cargar ejecuciones anteriores'),
          ),
      ],
    ),
  );
}

Future<void> showExecutionRun(
  BuildContext context,
  DesktopState state,
  int runId, {
  ExecutionCall? expected,
  ValueChanged<String>? onOpenPlugin,
}) => showDialog<void>(
  context: context,
  barrierDismissible: false,
  builder: (_) => _ExecutionRunDialog(
    state: state,
    runId: runId,
    expected: expected,
    onOpenPlugin: onOpenPlugin,
  ),
);

class _ExecutionRunDialog extends StatefulWidget {
  const _ExecutionRunDialog({
    required this.state,
    required this.runId,
    this.expected,
    this.onOpenPlugin,
  });
  final DesktopState state;
  final int runId;
  final ExecutionCall? expected;
  final ValueChanged<String>? onOpenPlugin;
  @override
  State<_ExecutionRunDialog> createState() => _ExecutionRunDialogState();
}

class _ExecutionRunDialogState extends State<_ExecutionRunDialog> {
  Map<String, dynamic>? run;
  bool loading = true, saving = false;
  String? choice, error;
  final errorKey = GlobalKey();
  int generation = 0;
  @override
  void initState() {
    super.initState();
    unawaited(load());
  }

  Future<void> load() async {
    final request = ++generation;
    setState(() {
      loading = true;
      error = null;
    });
    try {
      final result = await _call(widget.state, {
        'action': 'run_details',
        'run_id': widget.runId,
      });
      final expected = widget.expected;
      final assignment = result['configuration']['assignment'] as Map;
      if (expected != null &&
          (result['id'].toString() != expected.runId ||
              result['policy_id'] != expected.policyId ||
              result['policy_revision'] != expected.policyRevision ||
              assignment['id'].toString() != expected.assignmentId ||
              assignment['instance_id'] != expected.instanceId ||
              result['state'] != expected.state)) {
        throw StateError(
          'La referencia del mensaje no coincide con esta ejecución.',
        );
      }
      if (mounted && request == generation) {
        setState(() {
          run = result;
          loading = false;
        });
      }
    } catch (e) {
      if (mounted && request == generation) {
        setState(() {
          loading = false;
          error = 'No se pudo consultar el intento: $e';
        });
        _reveal(errorKey);
      }
    }
  }

  Future<void> save() async {
    if (saving || choice == null || run == null) return;
    setState(() {
      saving = true;
      error = null;
    });
    try {
      await _call(widget.state, {
        'action': 'reconcile',
        'run_id': run!['id'],
        'resolution': choice,
      });
      if (mounted) Navigator.pop(context);
    } catch (e) {
      if (mounted) {
        setState(() {
          saving = false;
          error = 'No se pudo guardar: $e';
        });
        _reveal(errorKey);
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final current = run;
    final unresolved =
        current?['state'] == 'uncertain' && current?['resolution'] == null;
    final configuration = current?['configuration'] as Map?;
    return AppReviewDialog(
      title: unresolved ? 'Revisar resultado' : 'Detalle de ejecución',
      busy: saving,
      actions: [
        TextButton(
          onPressed: saving ? null : () => Navigator.pop(context),
          child: const Text('Cerrar'),
        ),
        if (unresolved)
          FilledButton(
            onPressed: saving || choice == null ? null : save,
            child: const Text('Guardar comprobación'),
          ),
      ],
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          if (loading) const LinearProgressIndicator(),
          if (current != null && configuration != null) ...[
            Text(
              'Ocurrencia: ${_date(current['scheduled_at'])} · hora de este equipo',
            ),
            Text(
              _states[current['state']] ?? current['state'].toString(),
              style: const TextStyle(fontWeight: FontWeight.w600),
            ),
            Text(
              'Función: ${configuration['target']['tool'] ?? configuration['target']['function']} · revisión ${configuration['target']['revision']}',
            ),
            Text(
              'Alcance: ${configuration['assignment']['document']['authorization']?['scope'] ?? 'Sin definir'}',
            ),
            ExpansionTile(
              title: const Text('Parámetros utilizados'),
              tilePadding: EdgeInsets.zero,
              children: [
                SelectableText(
                  const JsonEncoder.withIndent(
                    '  ',
                  ).convert(configuration['arguments']),
                ),
              ],
            ),
            if (current['output'] != null)
              ExpansionTile(
                title: const Text('Detalles del resultado'),
                tilePadding: EdgeInsets.zero,
                children: [
                  SelectableText(
                    const JsonEncoder.withIndent(
                      '  ',
                    ).convert(current['output']),
                  ),
                ],
              ),
            if (current['output_omitted'] == true)
              const Text(
                'El resultado excedió el tamaño de almacenamiento local.',
              ),
            if (configuration['contextual_notice'] == true)
              _NoticeDecision(
                value: current['notification'] as Map?,
                busy: loading || saving,
                refresh: () => unawaited(load()),
              ),
            if (unresolved)
              const Text(
                'Comprueba el efecto antes de continuar. Guardar esta comprobación mantiene la ejecución pausada.',
              ),
            if (current['output']?['mcp_attempt_id'] != null &&
                widget.onOpenPlugin != null)
              TextButton(
                onPressed: saving
                    ? null
                    : () {
                        Navigator.pop(context);
                        widget.onOpenPlugin!(
                          'mcp:${configuration['target']['server_id']}',
                        );
                      },
                child: const Text('Revisar también el intento en Plugins'),
              ),
            if (current['output']?['mcp_attempt_id'] != null)
              const Text(
                'El intento de Plugins conserva su propia revisión pendiente. Revísalo allí antes de volver a activar esta función.',
              ),
            if (unresolved)
              RadioGroup<String>(
                groupValue: choice,
                onChanged: saving ? (_) {} : (v) => setState(() => choice = v),
                child: Column(
                  children: [
                    RadioListTile<String>(
                      enabled: !saving,
                      value: 'applied',
                      title: const Text('La acción se aplicó'),
                    ),
                    RadioListTile<String>(
                      enabled: !saving,
                      value: 'not_applied',
                      title: const Text('Comprobé que no se aplicó'),
                    ),
                  ],
                ),
              ),
            if (current['resolution'] != null)
              Text(
                current['resolution'] == 'applied'
                    ? 'Comprobado: se aplicó'
                    : 'Comprobado: no se aplicó',
              ),
          ],
          if (error != null)
            Text(
              error!,
              key: errorKey,
              style: TextStyle(color: Theme.of(context).colorScheme.error),
            ),
          if (!loading && current == null)
            TextButton(
              onPressed: load,
              child: const Text('Reintentar detalle'),
            ),
        ],
      ),
    );
  }
}
