part of 'plugins.dart';

const _mcpExpansionDuration = Duration(milliseconds: 180);

class McpPluginIcon extends StatelessWidget {
  const McpPluginIcon({super.key, this.size = 38});
  final double size;
  @override
  Widget build(BuildContext context) => SizedBox(
    width: size,
    height: size,
    child: Icon(
      Icons.hub_outlined,
      size: size * .6,
      color: sidebarInk(context),
    ),
  );
}

/// Configurar no inicia el programa ni concede acceso a sus herramientas.
class McpSourceEditor extends StatefulWidget {
  const McpSourceEditor({
    super.key,
    required this.call,
    required this.onCancel,
    required this.onSaved,
    this.source,
  });
  final PluginCall call;
  final VoidCallback onCancel;
  final Future<void> Function(Map<String, dynamic>) onSaved;
  final Map<String, dynamic>? source;
  @override
  State<McpSourceEditor> createState() => _McpSourceEditorState();
}

class _McpSourceEditorState extends State<McpSourceEditor> {
  final form = GlobalKey<FormState>();
  final fields = <String, TextEditingController>{};
  final focus = <String, FocusNode>{};
  final anchors = <String, GlobalKey>{};
  final validators = <String, String? Function(String)>{};
  final advanced = ExpansibleController();
  late String transport, id;
  bool saving = false;
  String? error;
  @override
  void initState() {
    super.initState();
    final spec = widget.source?['specification'] as Map? ?? {};
    final connection = spec['connection'] as Map? ?? {};
    id =
        spec['id'] as String? ?? 'mcp_${DateTime.now().microsecondsSinceEpoch}';
    transport = connection['type'] as String? ?? 'http';
    final values = <String, String>{
      'name': spec['name'] as String? ?? '',
      'url': connection['url'] as String? ?? '',
      'command': connection['command'] as String? ?? '',
      'args': _pretty.convert(connection['args'] ?? []),
      'cwd': connection['cwd'] as String? ?? '',
      'env': (connection['env_vars'] as List? ?? []).join('\n'),
      'bearer': connection['bearer_token_env_var'] as String? ?? '',
      'timeout': '${spec['timeout_seconds'] ?? 60}',
    };
    for (final e in values.entries) {
      fields[e.key] = TextEditingController(text: e.value);
      focus[e.key] = FocusNode();
      anchors[e.key] = GlobalKey();
    }
  }

  @override
  void dispose() {
    for (final c in fields.values) {
      c.dispose();
    }
    for (final node in focus.values) {
      node.dispose();
    }
    advanced.dispose();
    super.dispose();
  }

  String value(String name) => fields[name]!.text.trim();
  Widget field(
    String name,
    String label, {
    String? help,
    bool required = false,
    int lines = 1,
    String? Function(String)? validate,
  }) {
    validators[name] = (v) => required && v.trim().isEmpty
        ? 'Completa $label.'
        : validate?.call(v.trim());
    return Padding(
      key: anchors[name],
      padding: const EdgeInsets.only(top: 16),
      child: TextFormField(
        key: ValueKey('mcp-$name'),
        focusNode: focus[name],
        controller: fields[name],
        enabled: !saving,
        minLines: lines,
        maxLines: lines == 1 ? 1 : 6,
        autovalidateMode: AutovalidateMode.onUserInteraction,
        decoration: InputDecoration(
          labelText: label,
          helperText: help,
          helperMaxLines: 5,
          errorMaxLines: 4,
        ),
        validator: (v) => validators[name]!(v ?? ''),
      ),
    );
  }

  static bool envName(String v) =>
      RegExp(r'^[A-Za-z_][A-Za-z0-9_]{0,127}$').hasMatch(v);
  Future<void> save() async {
    if (saving) return;
    final active = [
      'name',
      if (transport == 'http') ...[
        'url',
        'bearer',
      ] else ...[
        'command',
        'args',
        'cwd',
        'env',
      ],
      'timeout',
    ];
    final invalid = active
        .where((key) => validators[key]!(value(key)) != null)
        .firstOrNull;
    if (invalid != null) {
      if ({'cwd', 'env', 'timeout'}.contains(invalid)) {
        final delay = MediaQuery.disableAnimationsOf(context)
            ? Duration.zero
            : _mcpExpansionDuration;
        advanced.expand();
        // El recorte animado todavía oculta al campo en el primer frame.
        await Future<void>.delayed(delay);
        await WidgetsBinding.instance.endOfFrame;
      }
      if (!mounted) return;
      form.currentState!.validate();
      focus[invalid]!.requestFocus();
      // Medir después de dibujar el error y de la solicitud de foco del campo.
      await WidgetsBinding.instance.endOfFrame;
      if (!mounted) return;
      final target = anchors[invalid]!.currentContext;
      if (target != null && target.mounted) {
        await Scrollable.ensureVisible(target, alignment: .2);
      }
      return;
    }
    setState(() {
      saving = true;
      error = null;
    });
    try {
      String? optional(String key) => value(key).isEmpty ? null : value(key);
      final connection = transport == 'http'
          ? <String, dynamic>{
              'type': 'http',
              'url': value('url'),
              'bearer_token_env_var': optional('bearer'),
            }
          : <String, dynamic>{
              'type': 'stdio',
              'command': value('command'),
              'args': jsonDecode(fields['args']!.text),
              'cwd': optional('cwd'),
              'env_vars': value(
                'env',
              ).split(RegExp(r'\s+')).where((v) => v.isNotEmpty).toList(),
            };
      final response = await widget.call({
        'action': 'mcp_save',
        'expected_revision': widget.source?['revision'] ?? 0,
        'specification': {
          'id': id,
          'name': value('name'),
          'connection': connection,
          'timeout_seconds': int.parse(value('timeout')),
        },
      });
      if (mounted) {
        await widget.onSaved(
          Map<String, dynamic>.from(response['server'] as Map),
        );
      }
    } catch (e) {
      if (mounted) setState(() => error = '$e');
    } finally {
      if (mounted) setState(() => saving = false);
    }
  }

  @override
  Widget build(BuildContext context) => Column(
    children: [
      Expanded(
        child: AppViewScroll(
          child: Form(
            key: form,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Text(
                  widget.source == null ? 'Conexión MCP' : 'Editar conexión',
                  style: const TextStyle(
                    fontSize: 20,
                    fontWeight: FontWeight.w600,
                  ),
                ),
                const SizedBox(height: 12),
                Text(
                  widget.source == null
                      ? 'Guarda la conexión. Después descubre y elige sus herramientas.'
                      : 'Guardar restablece permisos y catálogo.',
                ),
                field(
                  'name',
                  'Nombre',
                  required: true,
                  validate: (v) =>
                      v.runes.length > 200 || v.contains(RegExp(r'[\r\n]'))
                      ? 'Usa un nombre de una línea, hasta 200 caracteres.'
                      : null,
                ),
                const SizedBox(height: 16),
                DropdownButtonFormField<String>(
                  key: const ValueKey('mcp-transport'),
                  initialValue: transport,
                  isExpanded: true,
                  decoration: const InputDecoration(
                    labelText: 'Tipo de conexión',
                  ),
                  items: const [
                    DropdownMenuItem(
                      value: 'http',
                      child: Text('Streamable HTTP'),
                    ),
                    DropdownMenuItem(
                      value: 'stdio',
                      child: Text('Programa local · stdio'),
                    ),
                  ],
                  onChanged: saving
                      ? null
                      : (v) => setState(() => transport = v!),
                ),
                if (transport == 'http') ...[
                  field(
                    'url',
                    'Dirección del servidor',
                    required: true,
                    help:
                        'HTTPS. HTTP solo para localhost o una dirección loopback.',
                    validate: (v) {
                      final uri = Uri.tryParse(v);
                      final host =
                          uri?.host.replaceAll(RegExp(r'[\[\]]'), '') ?? '';
                      final local =
                          host == 'localhost' ||
                          host == '::1' ||
                          RegExp(
                            r'^127\.(\d{1,3}\.){2}\d{1,3}$',
                          ).hasMatch(host);
                      return uri == null ||
                              host.isEmpty ||
                              !(uri.scheme == 'https' ||
                                  uri.scheme == 'http' && local) ||
                              uri.userInfo.isNotEmpty ||
                              uri.hasQuery ||
                              uri.hasFragment
                          ? 'Usa una dirección admitida sin credenciales, parámetros ni fragmento.'
                          : null;
                    },
                  ),
                  field(
                    'bearer',
                    'Variable de autenticación (opcional)',
                    help:
                        'Nombre de una variable del entorno de Gennie. No pegues su valor aquí.',
                    validate: (v) => v.isNotEmpty && !envName(v)
                        ? 'Escribe solo el nombre de la variable.'
                        : null,
                  ),
                ] else ...[
                  field(
                    'command',
                    'Programa',
                    required: true,
                    help:
                        'Ruta o nombre del ejecutable. Los argumentos se introducen por separado.',
                  ),
                  field(
                    'args',
                    'Argumentos (lista JSON)',
                    lines: 2,
                    help:
                        'Ejemplo: ["--port", "3000"]. No se interpreta una línea de shell.',
                    validate: (v) {
                      try {
                        final args = jsonDecode(v);
                        if (args is List &&
                            args.length <= 64 &&
                            args.every(
                              (e) =>
                                  e is String &&
                                  utf8.encode(e).length <= 4000 &&
                                  !e.contains('\u0000'),
                            )) {
                          return null;
                        }
                      } catch (_) {
                        /* Error junto al campo. */
                      }
                      return 'Usa una lista JSON de hasta 64 textos.';
                    },
                  ),
                  const SizedBox(height: 12),
                  const Text(
                    'Este programa se ejecutará con los permisos de tu usuario. Para conectarlo, Gennie requiere los cuatro permisos del asistente.',
                  ),
                ],
                ExpansionTile(
                  controller: advanced,
                  maintainState: true,
                  expansionAnimationStyle: AnimationStyle(
                    duration: MediaQuery.disableAnimationsOf(context)
                        ? Duration.zero
                        : _mcpExpansionDuration,
                    reverseDuration: MediaQuery.disableAnimationsOf(context)
                        ? Duration.zero
                        : _mcpExpansionDuration,
                  ),
                  tilePadding: EdgeInsets.zero,
                  title: const Text('Opciones avanzadas'),
                  children: [
                    if (transport == 'stdio') ...[
                      field(
                        'cwd',
                        'Carpeta de trabajo (opcional)',
                        validate: (v) => v.isNotEmpty && !v.startsWith('/')
                            ? 'Usa una ruta absoluta.'
                            : null,
                      ),
                      field(
                        'env',
                        'Variables compartidas (opcional)',
                        lines: 2,
                        help:
                            'Nombres separados por espacios o saltos de línea. No introduzcas valores.',
                        validate: (v) =>
                            v.isNotEmpty &&
                                (v.split(RegExp(r'\s+')).length > 40 ||
                                    !v.split(RegExp(r'\s+')).every(envName))
                            ? 'Usa hasta 40 nombres de variables válidos.'
                            : null,
                      ),
                    ],
                    field(
                      'timeout',
                      'Plazo por herramienta (segundos)',
                      validate: (v) {
                        final n = int.tryParse(v);
                        return n == null || n < 1 || n > 720
                            ? 'Elige entre 1 y 720 segundos.'
                            : null;
                      },
                    ),
                  ],
                ),
                if (error != null)
                  Padding(
                    padding: const EdgeInsets.only(top: 16),
                    child: Text(
                      error!,
                      style: TextStyle(
                        color: Theme.of(context).colorScheme.error,
                      ),
                    ),
                  ),
              ],
            ),
          ),
        ),
      ),
      AppViewContent(
        footer: true,
        child: Wrap(
          spacing: 8,
          runSpacing: 8,
          children: [
            FilledButton(
              onPressed: saving ? null : save,
              child: Text(saving ? 'Guardando…' : 'Guardar conexión'),
            ),
            TextButton(
              onPressed: saving ? null : widget.onCancel,
              child: const Text('Cancelar'),
            ),
          ],
        ),
      ),
    ],
  );
}

class McpSourceView extends StatefulWidget {
  const McpSourceView({
    super.key,
    required this.serverId,
    required this.call,
    required this.onChanged,
    required this.onDeleted,
    this.observedActivity,
  });
  final String serverId;
  final String? observedActivity;
  final PluginCall call;
  final Future<void> Function() onChanged, onDeleted;
  @override
  State<McpSourceView> createState() => _McpSourceViewState();
}

class _McpSourceViewState extends State<McpSourceView> {
  Map<String, dynamic>? source;
  String? error, selectedTool;
  bool loading = true,
      busy = false,
      editing = false,
      enabled = false,
      runningTool = false;
  Set<String> allowed = {};
  int toolGeneration = 0, metadataGeneration = 0;
  @override
  void initState() {
    super.initState();
    unawaited(load());
  }

  @override
  void didUpdateWidget(covariant McpSourceView oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.observedActivity != widget.observedActivity &&
        source != null &&
        !loading &&
        !busy &&
        !editing) {
      unawaited(refreshConnectionStatus());
    }
  }

  void accept(Map<String, dynamic> value) {
    metadataGeneration++;
    if (selectedTool != null &&
        jsonEncode(source?['tools']?[selectedTool]) !=
            jsonEncode(value['tools']?[selectedTool])) {
      toolGeneration++;
    }
    source = value;
    allowed = Set<String>.from(value['allowed_tools'] as List);
    enabled = value['enabled'] == true;
    if (!(value['tools'] as Map).containsKey(selectedTool)) selectedTool = null;
  }

  Future<void> load() async {
    if (busy) return;
    setState(() {
      loading = true;
      error = null;
    });
    try {
      final response = await widget.call({
        'action': 'mcp_get',
        'server_id': widget.serverId,
      });
      if (mounted) {
        setState(
          () => accept(Map<String, dynamic>.from(response['server'] as Map)),
        );
      }
    } catch (e) {
      if (mounted) setState(() => error = '$e');
    } finally {
      if (mounted) setState(() => loading = false);
    }
  }

  Future<void> mutate(
    String action, [
    Map<String, dynamic> extra = const {},
  ]) async {
    if (busy) return;
    setState(() {
      busy = true;
      error = null;
    });
    try {
      final result = await widget.call({
        'action': action,
        'server_id': widget.serverId,
        'expected_revision': source!['revision'],
        ...extra,
      });
      if (!mounted) return;
      if (result['server'] is Map) {
        setState(
          () => accept(Map<String, dynamic>.from(result['server'] as Map)),
        );
      } else if (action == 'mcp_disconnect') {
        setState(() => source!['connection_status'] = 'disconnected');
      }
      await widget.onChanged();
    } catch (e) {
      if (mounted) setState(() => error = '$e');
    } finally {
      if (mounted) setState(() => busy = false);
    }
  }

  Future<void> refreshConnectionStatus() async {
    final generation = ++metadataGeneration;
    try {
      final response = await widget.call({
        'action': 'mcp_get',
        'server_id': widget.serverId,
      });
      if (mounted && generation == metadataGeneration && source != null) {
        setState(() {
          final value = response['server'] as Map;
          for (final key in [
            'connection_status',
            'pending_attempts',
            'pending_count',
            'pending_next_cursor',
            'recent_reviews',
          ]) {
            source![key] = value[key];
          }
          if (value['revision'] != source!['revision']) {
            error =
                'La conexión cambió. Recarga la ficha antes de ejecutar o revisar.';
          }
        });
      }
    } catch (_) {
      if (mounted && generation == metadataGeneration) {
        setState(() => source!['connection_status'] = 'unavailable');
      }
    }
  }

  Future<void> loadMoreAttempts() async {
    if (busy || source?['pending_next_cursor'] == null) return;
    metadataGeneration++;
    setState(() => busy = true);
    try {
      final page = await widget.call({
        'action': 'mcp_attempts',
        'server_id': widget.serverId,
        'after': source!['pending_next_cursor'],
      });
      if (mounted) {
        setState(() {
          final values = <int, dynamic>{
            for (final a in source!['pending_attempts'] as List)
              a['id'] as int: a,
          };
          for (final a in page['attempts'] as List) {
            values[a['id'] as int] = a;
          }
          source!['pending_attempts'] = values.values.toList();
          source!['pending_count'] = page['total'];
          source!['pending_next_cursor'] = page['next_cursor'];
        });
      }
    } catch (e) {
      if (mounted) setState(() => error = '$e');
    } finally {
      if (mounted) setState(() => busy = false);
    }
  }

  Future<void> reviewAttempt(Map<String, dynamic> attempt) async {
    final revision = source!['revision'];
    await showDialog<void>(
      context: context,
      barrierDismissible: false,
      builder: (_) => McpAttemptReviewDialog(
        attempt: attempt,
        onConfirm: (resolution) async {
          final response = await widget.call({
            'action': 'mcp_reconcile',
            'server_id': widget.serverId,
            'expected_revision': revision,
            'attempt_id': attempt['id'],
            'resolution': resolution,
          });
          if (!mounted) return;
          // La revisión no modifica el borrador de permisos ni los argumentos del probador.
          final draft = Set<String>.of(allowed);
          final draftEnabled = enabled;
          final dirty =
              enabled != source!['enabled'] ||
              !setEquals(
                allowed,
                Set<String>.from(source!['allowed_tools'] as List),
              );
          setState(() {
            accept(Map<String, dynamic>.from(response['server'] as Map));
            if (dirty) {
              allowed = draft;
              enabled = draftEnabled;
            }
          });
          await widget.onChanged();
        },
      ),
    );
  }

  Future<void> remove() async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Eliminar conexión MCP'),
        content: Text(
          'Se retirarán la conexión y los permisos de ${source!['specification']['name']}. El historial de mensajes se conserva.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: const Text('Cancelar'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(context, true),
            child: const Text('Eliminar'),
          ),
        ],
      ),
    );
    if (confirmed != true || !mounted || busy) return;
    setState(() => busy = true);
    try {
      final result = await widget.call({
        'action': 'mcp_delete',
        'server_id': widget.serverId,
        'expected_revision': source!['revision'],
      });
      if (result['status'] != 'deleted') {
        throw StateError('No se confirmó la eliminación.');
      }
      if (mounted) await widget.onDeleted();
    } catch (e) {
      if (mounted) {
        setState(() {
          error = '$e';
          busy = false;
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    if (loading) {
      return const Center(child: CircularProgressIndicator(strokeWidth: 2));
    }
    if (source == null) {
      return AppViewScroll(
        child: Column(
          children: [
            Text(error ?? 'No se pudo leer la conexión.'),
            TextButton(onPressed: load, child: const Text('Reintentar')),
          ],
        ),
      );
    }
    if (editing) {
      return McpSourceEditor(
        source: source,
        call: widget.call,
        onCancel: () => setState(() => editing = false),
        onSaved: (value) async {
          if (mounted) {
            setState(() {
              accept(value);
              editing = false;
            });
          }
          await widget.onChanged();
        },
      );
    }
    final spec = source!['specification'] as Map;
    final tools = source!['tools'] as Map;
    final schemas = source!['schema_status'] as Map? ?? {};
    final state = runningTool ? 'working' : source!['connection_status'];
    final locked = busy || runningTool;
    final dirty =
        enabled != source!['enabled'] ||
        !setEquals(allowed, Set<String>.from(source!['allowed_tools'] as List));
    final ready =
        !dirty &&
        source!['enabled'] == true &&
        allowed.contains(selectedTool) &&
        schemas[selectedTool] == 'ready';
    final pending = (source!['pending_attempts'] as List? ?? []);
    final blocked = pending.any((a) => a['tool'] == selectedTool);
    return AppViewScroll(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          LayoutBuilder(
            builder: (context, constraints) => AppDetailHeading(
              icon: constraints.maxWidth < 400
                  ? const SizedBox.shrink()
                  : const McpPluginIcon(size: 72),
              label: 'Plugin · MCP',
              title: spec['name'] as String,
              subtitle: Text(
                '${spec['connection']['type'] == 'http' ? 'Streamable HTTP' : 'Programa local · stdio'} · revisión ${source!['revision']}',
              ),
              onEdit: locked ? null : () => setState(() => editing = true),
              editLabel: 'Editar conexión MCP',
            ),
          ),
          const SizedBox(height: 20),
          Text(switch (state) {
            'connected' => 'Conectado',
            'working' => 'Trabajando',
            'unavailable' => 'Estado no disponible',
            _ => 'Desconectado',
          }, key: const ValueKey('mcp-connection-status')),
          const SizedBox(height: 8),
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: [
              OutlinedButton(
                onPressed: locked ? null : () => mutate('mcp_connect'),
                child: Text(busy ? 'Esperando…' : 'Descubrir herramientas'),
              ),
              TextButton(
                onPressed: busy ? null : () => mutate('mcp_disconnect'),
                child: const Text('Desconectar'),
              ),
              TextButton(
                onPressed: locked ? null : load,
                child: const Text('Recargar ficha'),
              ),
              TextButton(
                onPressed: locked ? null : remove,
                child: const Text('Eliminar conexión'),
              ),
            ],
          ),
          if (busy) const LinearProgressIndicator(),
          if (error != null)
            Padding(
              padding: const EdgeInsets.only(top: 12),
              child: Text(
                error!,
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            ),
          if ((source!['pending_count'] as int? ?? pending.length) > 0) ...[
            const Divider(height: 32),
            Semantics(
              header: true,
              child: Text(
                'Resultados pendientes · ${source!['pending_count'] ?? pending.length}',
                style: const TextStyle(
                  fontSize: 16,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ),
            const SizedBox(height: 8),
            const Text(
              'Una llamada sin confirmar bloquea otra ejecución de esa herramienta, incluso después de reiniciar. Comprueba su efecto antes de revisarla.',
            ),
            for (final attempt in pending)
              Padding(
                padding: const EdgeInsets.only(top: 16),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    SelectableText(
                      attempt['tool'] as String,
                      style: const TextStyle(fontWeight: FontWeight.w600),
                    ),
                    Text(
                      attempt['active'] == true ? 'En curso' : 'Sin confirmar',
                    ),
                    Text(
                      mcpAttemptDate(attempt['created_at'] as String?),
                      style: const TextStyle(fontSize: 12),
                    ),
                    TextButton(
                      onPressed: locked || attempt['active'] == true
                          ? null
                          : () => reviewAttempt(
                              Map<String, dynamic>.from(attempt as Map),
                            ),
                      child: const Text('Revisar resultado'),
                    ),
                  ],
                ),
              ),
            if (source!['pending_next_cursor'] != null)
              Align(
                alignment: Alignment.centerLeft,
                child: TextButton(
                  onPressed: locked ? null : loadMoreAttempts,
                  child: const Text('Cargar más intentos'),
                ),
              ),
          ],
          const SizedBox(height: 24),
          const Text(
            'Permisos',
            style: TextStyle(fontSize: 16, fontWeight: FontWeight.w600),
          ),
          const SizedBox(height: 8),
          const Text(
            'Elige las herramientas que Gennie podrá usar cuando se lo pidas. Descubrirlas no las ejecuta ni las autoriza.',
          ),
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            title: const Text('Habilitar este plugin'),
            value: enabled,
            onChanged: locked ? null : (v) => setState(() => enabled = v),
          ),
          if (tools.isEmpty)
            const Text(
              'Aún no hay herramientas guardadas. Descubre el catálogo para elegir permisos.',
            ),
          for (final name in tools.keys.cast<String>())
            CheckboxListTile(
              contentPadding: EdgeInsets.zero,
              controlAffinity: ListTileControlAffinity.leading,
              title: Text(name),
              subtitle: Text(
                schemas[name] == 'ready'
                    ? (tools[name]['description'] as String? ??
                          'Sin descripción.')
                    : 'Esquema no admitido: esta herramienta no se puede habilitar.',
              ),
              value: allowed.contains(name),
              onChanged: locked || schemas[name] != 'ready'
                  ? null
                  : (v) => setState(() {
                      if (v == true) {
                        allowed.add(name);
                      } else {
                        allowed.remove(name);
                      }
                    }),
            ),
          Align(
            alignment: Alignment.centerLeft,
            child: FilledButton(
              onPressed: !locked && dirty
                  ? () => mutate('mcp_permissions', {
                      'enabled': enabled,
                      'allowed_tools': allowed.toList(),
                    })
                  : null,
              child: const Text('Guardar permisos'),
            ),
          ),
          if (dirty)
            const Padding(
              padding: EdgeInsets.only(top: 8),
              child: Text('Guarda los permisos antes de ejecutar.'),
            ),
          if (tools.isNotEmpty) ...[
            const Divider(height: 40),
            const Text(
              'Probar una herramienta',
              style: TextStyle(fontSize: 16, fontWeight: FontWeight.w600),
            ),
            const SizedBox(height: 12),
            DropdownButtonFormField<String>(
              key: const ValueKey('mcp-tool'),
              initialValue: selectedTool,
              isExpanded: true,
              decoration: const InputDecoration(labelText: 'Herramienta'),
              items: [
                for (final name in tools.keys.cast<String>())
                  DropdownMenuItem(
                    value: name,
                    child: Text(name, overflow: TextOverflow.ellipsis),
                  ),
              ],
              onChanged: locked
                  ? null
                  : (v) => setState(() => selectedTool = v),
            ),
            if (selectedTool != null) ...[
              const SizedBox(height: 16),
              if (!ready)
                const Text(
                  'Para ejecutar, guarda la habilitación y el permiso de esta herramienta. Su esquema debe ser compatible.',
                ),
              if (ready)
                McpToolProbe(
                  key: ValueKey(
                    '${widget.serverId}:$selectedTool:$toolGeneration',
                  ),
                  definition: Map<String, dynamic>.from(
                    tools[selectedTool] as Map,
                  ),
                  blocked: blocked,
                  pendingAttempt:
                      pending
                              .where((a) => a['tool'] == selectedTool)
                              .firstOrNull
                          as Map<String, dynamic>?,
                  onReview: reviewAttempt,
                  reviewedAttemptId:
                      (source!['recent_reviews'] as List? ?? [])
                              .where((a) => a['tool'] == selectedTool)
                              .firstOrNull?['id']
                          as int?,
                  onRunningChanged: (v) {
                    if (mounted) setState(() => runningTool = v);
                    if (!v && mounted) unawaited(refreshConnectionStatus());
                  },
                  call: (arguments) async => widget.call({
                    'action': 'mcp_run',
                    'server_id': widget.serverId,
                    'tool': selectedTool,
                    'expected_revision': source!['revision'],
                    'arguments': arguments,
                  }),
                ),
            ],
          ],
        ],
      ),
    );
  }
}

/// Campos simples; el modo JSON conserva las entradas avanzadas del contrato.
Map<String, dynamic>? mcpSimpleParameters(Map schema) {
  const rootKeys = {
    'type',
    'properties',
    'required',
    'additionalProperties',
    'description',
    'title',
    r'$schema',
  };
  const fieldKeys = {
    'type',
    'title',
    'description',
    'enum',
    'minLength',
    'maxLength',
    'minimum',
    'maximum',
  };
  if (schema['type'] != 'object' ||
      !schema.keys.every(rootKeys.contains) ||
      schema['additionalProperties'] is Map) {
    return null;
  }
  final properties = schema['properties'] as Map? ?? {};
  final required = schema['required'] as List? ?? [];
  final result = <String, dynamic>{};
  for (final e in properties.entries) {
    if (e.value is! Map) return null;
    final field = e.value as Map;
    if (!field.keys.every(fieldKeys.contains) ||
        !{'string', 'integer', 'number', 'boolean'}.contains(field['type'])) {
      return null;
    }
    final type = field['type'];
    if (field['enum'] is List &&
        (field['enum'] as List).any(
          (v) =>
              v == null ||
              (type == 'string'
                  ? v is! String
                  : type == 'boolean'
                  ? v is! bool
                  : v is! num),
        )) {
      return null;
    }
    result[e.key as String] = {
      ...Map<String, dynamic>.from(field),
      'label': field['title'] ?? e.key,
      'required': required.contains(e.key),
      if (type == 'string') 'minLength': field['minLength'] ?? 0,
      if (type == 'string') 'maxLength': field['maxLength'] ?? 60000,
      if (type == 'integer' || type == 'number')
        'minimum': field['minimum'] ?? -1.7976931348623157e308,
      if (type == 'integer' || type == 'number')
        'maximum': field['maximum'] ?? 1.7976931348623157e308,
    };
  }
  return result;
}

class McpToolProbe extends StatefulWidget {
  const McpToolProbe({
    super.key,
    required this.definition,
    required this.call,
    this.onRunningChanged,
    this.blocked = false,
    this.reviewedAttemptId,
    this.pendingAttempt,
    this.onReview,
  });
  final Map<String, dynamic> definition;
  final ValueChanged<bool>? onRunningChanged;
  final bool blocked;
  final int? reviewedAttemptId;
  final Map<String, dynamic>? pendingAttempt;
  final Future<void> Function(Map<String, dynamic>)? onReview;
  final Future<Map<String, dynamic>> Function(Map<String, dynamic>) call;
  @override
  State<McpToolProbe> createState() => _McpToolProbeState();
}

class _McpToolProbeState extends State<McpToolProbe> {
  final arguments = TextEditingController(text: '{}');
  final form = GlobalKey<FormState>();
  Map<String, dynamic>? result;
  String? error;
  bool running = false, uncertain = false, raw = false;
  bool get blocked => uncertain || widget.blocked;
  @override
  void didUpdateWidget(covariant McpToolProbe oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!widget.blocked &&
        widget.reviewedAttemptId != null &&
        oldWidget.reviewedAttemptId != widget.reviewedAttemptId) {
      uncertain = false;
    }
  }

  @override
  void dispose() {
    arguments.dispose();
    super.dispose();
  }

  Future<Map<String, dynamic>> invoke(Map<String, dynamic> args) async {
    if (running || blocked) {
      throw StateError('Comprueba el resultado anterior antes de repetir.');
    }
    setState(() {
      running = true;
      error = null;
      result = null;
    });
    widget.onRunningChanged?.call(true);
    Map<String, dynamic> response;
    try {
      response = await widget.call(args);
    } catch (_) {
      response = {
        'status': 'result_uncertain',
        'message':
            'No se recibió un resultado. Comprueba el efecto antes de repetir.',
      };
    }
    if (mounted) widget.onRunningChanged?.call(false);
    if (mounted) {
      setState(() {
        running = false;
        result = response;
        uncertain =
            response['status'] == 'result_uncertain' ||
            response['status'] == 'review_required' ||
            response['review_required'] == true;
      });
    }
    return response;
  }

  Future<void> runJson() async {
    if (running || blocked || !form.currentState!.validate()) return;
    await invoke(Map<String, dynamic>.from(jsonDecode(arguments.text) as Map));
  }

  @override
  Widget build(BuildContext context) {
    final definition = widget.definition;
    final schema = definition['inputSchema'] as Map? ?? {};
    final parameters = mcpSimpleParameters(schema);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        const Text(
          'La prueba ejecuta la herramienta y puede producir efectos en el servicio conectado. Revisa los datos antes de continuar.',
        ),
        const SizedBox(height: 12),
        if (parameters != null) ...[
          Align(
            alignment: Alignment.centerLeft,
            child: TextButton(
              onPressed: running || blocked
                  ? null
                  : () => setState(() => raw = !raw),
              child: Text(
                raw ? 'Usar campos del formulario' : 'Introducir JSON avanzado',
              ),
            ),
          ),
        ],
        if (parameters != null && !raw)
          FunctionProbe(
            enabled: !blocked,
            record: {
              'manifest': {'id': 'mcp'},
              'revision': 1,
              'verified': {definition['name']: true},
            },
            function: {
              'name': definition['name'],
              'description': definition['description'] ?? '',
              'transport': 'mcp',
              'path': definition['name'],
              'parameters': parameters,
            },
            call: (request) =>
                invoke(Map<String, dynamic>.from(request['arguments'] as Map)),
          )
        else
          Form(
            key: form,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Text(definition['description'] as String? ?? ''),
                ExpansionTile(
                  tilePadding: EdgeInsets.zero,
                  title: const Text('Ver esquema de entrada'),
                  children: [
                    SelectableText(
                      _pretty.convert(schema),
                      style: const TextStyle(fontSize: 12),
                    ),
                  ],
                ),
                TextFormField(
                  key: const ValueKey('mcp-arguments'),
                  controller: arguments,
                  enabled: !running && !blocked,
                  minLines: 4,
                  maxLines: 12,
                  decoration: const InputDecoration(
                    labelText: 'Argumentos (objeto JSON)',
                    helperText:
                        'Este esquema contiene estructuras complejas. Gennie valida todos los campos antes de llamar.',
                    helperMaxLines: 4,
                    errorMaxLines: 4,
                  ),
                  validator: (v) {
                    try {
                      if (jsonDecode(v ?? '') is Map) return null;
                    } catch (_) {
                      /* Error de formato. */
                    }
                    return 'Introduce un objeto JSON válido.';
                  },
                ),
                const SizedBox(height: 12),
                Align(
                  alignment: Alignment.centerLeft,
                  child: FilledButton(
                    onPressed: running || blocked ? null : runJson,
                    child: Text(
                      running ? 'Ejecutando…' : 'Ejecutar herramienta',
                    ),
                  ),
                ),
                if (result != null)
                  Padding(
                    padding: const EdgeInsets.only(top: 12),
                    child: SelectableText(_pretty.convert(result)),
                  ),
              ],
            ),
          ),
        if (blocked) ...[
          const SizedBox(height: 12),
          const Text(
            'Resultado pendiente. Comprueba el efecto y usa Revisar resultado en la ficha. Si no aparece, recarga la ficha.',
          ),
          if (widget.onReview != null &&
              (result?['attempt'] ?? widget.pendingAttempt) is Map)
            TextButton(
              onPressed:
                  running ||
                      (result?['attempt'] ??
                              widget.pendingAttempt)?['active'] ==
                          true
                  ? null
                  : () => widget.onReview!(
                      Map<String, dynamic>.from(
                        (result?['attempt'] ?? widget.pendingAttempt) as Map,
                      ),
                    ),
              child: const Text('Revisar resultado'),
            ),
        ],
        if (error != null)
          Text(
            error!,
            style: TextStyle(color: Theme.of(context).colorScheme.error),
          ),
      ],
    );
  }
}

String mcpAttemptDate(String? value) {
  final date = DateTime.tryParse(value ?? '')?.toLocal();
  if (date == null) return 'Fecha no disponible';
  String two(int n) => n.toString().padLeft(2, '0');
  return '${two(date.day)}/${two(date.month)}/${date.year} · ${two(date.hour)}:${two(date.minute)}';
}

class McpAttemptReviewDialog extends StatefulWidget {
  const McpAttemptReviewDialog({
    super.key,
    required this.attempt,
    required this.onConfirm,
  });
  final Map<String, dynamic> attempt;
  final Future<void> Function(String) onConfirm;
  @override
  State<McpAttemptReviewDialog> createState() => _McpAttemptReviewDialogState();
}

class _McpAttemptReviewDialogState extends State<McpAttemptReviewDialog> {
  final scroll = ScrollController();
  String? resolution, error;
  bool saving = false;
  @override
  void dispose() {
    scroll.dispose();
    super.dispose();
  }

  Future<void> confirm() async {
    if (saving || resolution == null) return;
    setState(() {
      saving = true;
      error = null;
    });
    try {
      await widget.onConfirm(resolution!);
      if (mounted) Navigator.pop(context);
    } catch (e) {
      if (mounted) {
        setState(() {
          saving = false;
          error = '$e';
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) => PopScope(
    canPop: !saving,
    child: Actions(
      actions: <Type, Action<Intent>>{
        DismissIntent: CallbackAction<DismissIntent>(
          onInvoke: (_) {
            if (!saving) Navigator.of(context).pop();
            return null;
          },
        ),
      },
      child: FocusScope(
        autofocus: true,
        child: Dialog(
          backgroundColor:
              (Theme.of(context).brightness == Brightness.dark
                      ? const Color(0xff26282c)
                      : const Color(0xfff5f5f4))
                  .withValues(
                    alpha: MediaQuery.highContrastOf(context) ? 1 : .94,
                  ),
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(12),
          ),
          child: Glass(
            padding: const EdgeInsets.all(24),
            child: SizedBox(
              width: 420,
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  const Text(
                    'Revisar resultado',
                    style: TextStyle(fontSize: 18, fontWeight: FontWeight.w600),
                  ),
                  const SizedBox(height: 12),
                  Flexible(
                    child: Scrollbar(
                      controller: scroll,
                      thumbVisibility: true,
                      child: SingleChildScrollView(
                        controller: scroll,
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(
                              '${widget.attempt['server_name']} · ${widget.attempt['tool']}',
                              style: const TextStyle(
                                fontWeight: FontWeight.w600,
                              ),
                            ),
                            const SizedBox(height: 8),
                            Text(
                              '${mcpAttemptDate(widget.attempt['created_at'] as String?)} · revisión ${widget.attempt['source_revision']}',
                            ),
                            const SizedBox(height: 16),
                            const Text(
                              'Comprueba el efecto en el servicio conectado antes de elegir. Esta revisión no ejecuta la herramienta.',
                            ),
                            const SizedBox(height: 12),
                            RadioGroup<String>(
                              groupValue: resolution,
                              onChanged: saving
                                  ? (_) {}
                                  : (v) => setState(() => resolution = v),
                              child: Column(
                                children: [
                                  RadioListTile<String>(
                                    contentPadding: EdgeInsets.zero,
                                    value: 'applied',
                                    enabled: !saving,
                                    title: const Text('La acción se aplicó'),
                                    subtitle: const Text(
                                      'Cerrar el intento. Gennie no debe repetirlo.',
                                    ),
                                  ),
                                  RadioListTile<String>(
                                    contentPadding: EdgeInsets.zero,
                                    value: 'not_applied',
                                    enabled: !saving,
                                    title: const Text(
                                      'Comprobé que no se aplicó',
                                    ),
                                    subtitle: const Text(
                                      'Permitir otra llamada dentro de la autorización vigente.',
                                    ),
                                  ),
                                ],
                              ),
                            ),
                            if (error != null)
                              Padding(
                                padding: const EdgeInsets.only(top: 12),
                                child: Text(
                                  error!,
                                  style: TextStyle(
                                    color: Theme.of(context).colorScheme.error,
                                  ),
                                ),
                              ),
                          ],
                        ),
                      ),
                    ),
                  ),
                  const SizedBox(height: 16),
                  Wrap(
                    spacing: 8,
                    runSpacing: 8,
                    alignment: WrapAlignment.end,
                    children: [
                      TextButton(
                        onPressed: saving ? null : () => Navigator.pop(context),
                        child: const Text('Cancelar'),
                      ),
                      FilledButton(
                        onPressed: saving || resolution == null
                            ? null
                            : confirm,
                        child: Text(saving ? 'Guardando…' : 'Guardar revisión'),
                      ),
                    ],
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    ),
  );
}
