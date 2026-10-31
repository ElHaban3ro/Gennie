import 'dart:convert';
import 'package:flutter/material.dart';
import 'backend.dart';
import 'detail_view.dart';
import 'glass.dart';
import 'sidebar.dart';

class ArchitectsView extends StatelessWidget {
  const ArchitectsView({
    super.key,
    required this.state,
    required this.sidebarWidth,
    required this.onSaving,
  });
  final DesktopState state;
  final double sidebarWidth;
  final ValueChanged<bool> onSaving;
  @override
  Widget build(BuildContext context) {
    final backend = state.backend;
    if (backend is ArchitectBackend) {
      return ArchitectSettingsPanel(
        backend: backend as ArchitectBackend,
        sidebarWidth: sidebarWidth,
        onSaving: onSaving,
      );
    }
    return Row(
      children: [
        AppSidebar(
          width: sidebarWidth,
          title: 'Arquitectos',
          child: const SidebarEmpty(message: 'Catálogo no disponible'),
        ),
        const Expanded(
          child: ContentEmpty(
            title: 'Arquitectos',
            message: 'No se pudieron cargar los arquitectos',
          ),
        ),
      ],
    );
  }
}

class ArchitectSettingsPanel extends StatefulWidget {
  const ArchitectSettingsPanel({
    super.key,
    required this.backend,
    required this.onSaving,
    this.sidebarWidth = 260,
  });
  final ArchitectBackend backend;
  final ValueChanged<bool> onSaving;
  final double sidebarWidth;
  @override
  State<ArchitectSettingsPanel> createState() => _ArchitectSettingsPanelState();
}

class _Draft {
  _Draft(this.profile)
    : name = TextEditingController(
        text:
            (profile['name'] as String?) ??
            _defaultName(profile['role'] as String),
      ),
      type = TextEditingController(text: profile['role'] as String),
      model = TextEditingController(text: profile['model'] as String),
      personality = TextEditingController(
        text: profile['personality'] as String,
      ),
      instructions = TextEditingController(
        text: profile['instructions'] as String,
      );
  final Map<String, dynamic> profile;
  final TextEditingController name, type, model, personality, instructions;
  final form = GlobalKey<FormState>();
  final expansion = ExpansibleController();
  final modelFocus = FocusNode(),
      personalityFocus = FocusNode(),
      instructionsFocus = FocusNode();
  bool advanced = false;
  bool isNew = false;
  bool get isBase =>
      const ['gennie', 'plugin_builder', 'proactivity_guide'].contains(id);
  bool get isGuide => profile['provider'] == 'openrouter_decisions';
  String get role => profile['role'] as String;
  String get id => profile['id'] as String;
  void dispose() {
    name.dispose();
    type.dispose();
    model.dispose();
    personality.dispose();
    instructions.dispose();
    expansion.dispose();
    modelFocus.dispose();
    personalityFocus.dispose();
    instructionsFocus.dispose();
  }

  Map<String, dynamic> get value => {
    ...profile,
    'name': name.text.trim(),
    'role': type.text.trim(),
    'model': model.text.trim(),
    'personality': personality.text,
    'instructions': instructions.text,
  };
}

String _defaultName(String type) => switch (type) {
  'bot' => 'Gennie',
  'helper' => 'Constructor de plugins',
  'guide' => 'Guía de proactividad',
  _ => type,
};

class _ArchitectSettingsPanelState extends State<ArchitectSettingsPanel> {
  final scroll = ScrollController();
  final search = TextEditingController();
  List<_Draft> drafts = [];
  int selected = 0;
  bool loading = true, saving = false;
  String? error, confirmation;
  @override
  void initState() {
    super.initState();
    _load();
  }

  @override
  void dispose() {
    for (final d in drafts) {
      d.dispose();
    }
    scroll.dispose();
    search.dispose();
    super.dispose();
  }

  Future<void> _load() async {
    setState(() {
      loading = true;
      error = null;
      confirmation = null;
    });
    try {
      final response =
          jsonDecode(
                await widget.backend.architectRequest(
                  jsonEncode({'action': 'list'}),
                ),
              )
              as Map<String, dynamic>;
      final profiles = (response['profiles'] as List)
          .cast<Map<String, dynamic>>();
      if (!const [
            'gennie',
            'plugin_builder',
            'proactivity_guide',
          ].every((id) => profiles.any((p) => p['id'] == id)) ||
          profiles.map((p) => p['id']).toSet().length != profiles.length) {
        throw const FormatException('Configuración de arquitectos incompleta');
      }
      if (!mounted) return;
      setState(() {
        for (final d in drafts) {
          d.dispose();
        }
        drafts = profiles.map((p) => _Draft(Map.of(p))).toList();
        selected = 0;
        loading = false;
      });
    } catch (e) {
      if (mounted) {
        setState(() {
          loading = false;
          error = '$e';
        });
      }
    }
  }

  Future<void> _save() async {
    if (saving) return;
    final draft = drafts[selected];
    if (!draft.form.currentState!.validate()) {
      setState(() {
        draft.advanced = true;
        error = 'Revisa los campos indicados.';
        confirmation = null;
      });
      final delay = MediaQuery.disableAnimationsOf(context)
          ? Duration.zero
          : const Duration(milliseconds: 180);
      draft.expansion.expand();
      await Future<void>.delayed(delay);
      // Los validadores también cubren campos avanzados cuando están plegados.
      await WidgetsBinding.instance.endOfFrame;
      if (!mounted || loading || drafts[selected] != draft) return;
      final focus = _textError(draft.personality.text, 1000) != null
          ? draft.personalityFocus
          : !draft.isGuide && _modelError(draft.model.text) != null
          ? draft.modelFocus
          : draft.instructionsFocus;
      focus.requestFocus();
      await WidgetsBinding.instance.endOfFrame;
      if (!mounted) return;
      final field =
          focus.context
              ?.findAncestorStateOfType<FormFieldState<String>>()
              ?.context ??
          focus.context;
      if (field != null && field.mounted) {
        await Scrollable.ensureVisible(
          field,
          alignment: .2,
          duration: Duration.zero,
        );
      }
      return;
    }
    setState(() {
      saving = true;
      error = null;
      confirmation = null;
    });
    widget.onSaving(true);
    try {
      final response =
          jsonDecode(
                await widget.backend.architectRequest(
                  jsonEncode({
                    'action': draft.isNew ? 'create' : 'save',
                    'profile': draft.value,
                  }),
                ),
              )
              as Map<String, dynamic>;
      final profile = response['profile'] as Map<String, dynamic>;
      if ((!draft.isNew && profile['id'] != draft.id) ||
          profile['id'] is! String ||
          profile['revision'] is! int ||
          (profile['revision'] as int) <= (draft.profile['revision'] as int)) {
        throw const FormatException('No se recibió confirmación del guardado');
      }
      if (!mounted) return;
      setState(() {
        draft.profile.addAll(profile);
        draft.isNew = false;
        confirmation = 'Arquitecto guardado';
      });
    } catch (e) {
      if (mounted) {
        setState(() => error = '$e');
      }
    } finally {
      if (mounted) {
        setState(() => saving = false);
        widget.onSaving(false);
      }
    }
  }

  String _label(String role) => switch (role) {
    'bot' => 'Bot',
    'helper' => 'Helper',
    'guide' => 'Guía',
    _ => role,
  };

  void _newDraft({bool duplicate = false}) {
    if (saving) return;
    if (drafts.any((d) => d.isNew)) {
      setState(() => selected = drafts.indexWhere((d) => d.isNew));
      return;
    }
    final source = duplicate
        ? drafts[selected].value
        : {
            'role': 'helper',
            'provider': 'codex_chatgpt',
            'model': 'gpt-6-luna',
            'effort': 'low',
            'personality': '',
            'instructions': '',
            'read_files': false,
            'write_files': false,
            'run_commands': false,
            'local_actions': false,
          };
    setState(() {
      search.clear();
      drafts.add(
        _Draft({
          ...source,
          'id': 'new-architect',
          'name': duplicate ? '${source['name']} · copia' : '',
          'revision': 0,
          'enabled': false,
        })..isNew = true,
      );
      selected = drafts.length - 1;
      error = confirmation = null;
    });
    if (scroll.hasClients) scroll.jumpTo(0);
  }

  Future<void> _remove() async {
    if (saving) return;
    final d = drafts[selected];
    if (d.isBase) return;
    if (!d.isNew) {
      final confirmed = await showDialog<bool>(
        context: context,
        builder: (context) => AlertDialog(
          title: const Text('Eliminar arquitecto'),
          content: Text(
            '¿Eliminar ${d.name.text}? Los arquitectos base se conservan.',
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
      if (confirmed != true || !mounted) return;
      setState(() {
        saving = true;
        error = confirmation = null;
      });
      widget.onSaving(true);
      try {
        final response =
            jsonDecode(
                  await widget.backend.architectRequest(
                    jsonEncode({
                      'action': 'remove',
                      'id': d.id,
                      'revision': d.profile['revision'],
                    }),
                  ),
                )
                as Map<String, dynamic>;
        if (response['removed'] != d.id) {
          throw const FormatException('No se confirmó la eliminación');
        }
      } catch (e) {
        if (mounted) setState(() => error = '$e');
        return;
      } finally {
        if (mounted) {
          setState(() => saving = false);
          widget.onSaving(false);
        }
      }
    }
    if (!mounted) return;
    setState(() {
      drafts.remove(d);
      selected = 0;
      confirmation = 'Arquitecto retirado';
    });
    // Liberar controladores después de desmontar el formulario anterior.
    WidgetsBinding.instance.addPostFrameCallback((_) => d.dispose());
  }

  String? _modelError(String? text) {
    final model = text?.trim() ?? '';
    if (model.isEmpty ||
        model.length > 100 ||
        !RegExp(r'^[A-Za-z0-9_.-]+$').hasMatch(model)) {
      return 'Introduce un identificador de modelo válido.';
    }
    return null;
  }

  String? _textError(String? text, int max) {
    if ((text ?? '').runes.length > max) {
      return 'Máximo $max caracteres.';
    }
    if (RegExp(r'[\x00-\x08\x0b\x0c\x0e-\x1f\x7f-\x9f]').hasMatch(text ?? '')) {
      return 'Retira los caracteres de control.';
    }
    return null;
  }

  Widget _toggle(_Draft d, String key, String label) => SwitchListTile.adaptive(
    contentPadding: EdgeInsets.zero,
    title: Text(label, style: const TextStyle(fontSize: 13)),
    value: d.profile[key] as bool,
    onChanged: saving
        ? null
        : (v) => setState(() {
            d.profile[key] = v;
            if (key == 'read_files' && !v) d.profile['write_files'] = false;
            if (key == 'write_files' && v) d.profile['read_files'] = true;
            if (key == 'run_commands' && !v) d.profile['local_actions'] = false;
            if (key == 'local_actions' && v) d.profile['run_commands'] = true;
            confirmation = null;
          }),
  );
  @override
  Widget build(BuildContext context) {
    final query = search.text.trim().toLowerCase();
    return Row(
      children: [
        AppSidebar(
          width: widget.sidebarWidth,
          title: 'Arquitectos',
          actionLabel: 'Nuevo arquitecto',
          onAction: saving || loading || drafts.isEmpty ? null : _newDraft,
          searchController: search,
          searchHint: 'Buscar arquitectos…',
          onSearch: (_) => setState(() {}),
          child: loading
              ? const Center(child: CircularProgressIndicator(strokeWidth: 2))
              : ListView(
                  padding: const EdgeInsets.symmetric(horizontal: 8),
                  children: [
                    for (final (index, d) in drafts.indexed)
                      if ('${d.name.text} ${d.type.text}'
                          .toLowerCase()
                          .contains(query))
                        SidebarTile(
                          title: d.name.text.isEmpty
                              ? 'Nuevo arquitecto'
                              : d.name.text,
                          subtitle:
                              '${_label(d.type.text)}${d.isBase
                                  ? ' · Base'
                                  : d.isNew
                                  ? ' · Borrador'
                                  : ''}',
                          selected: selected == index,
                          onSelect: () {
                            if (!saving) {
                              setState(() {
                                selected = index;
                                error = confirmation = null;
                                if (scroll.hasClients) scroll.jumpTo(0);
                              });
                            }
                          },
                          leading: const Icon(
                            Icons.account_tree_outlined,
                            size: 20,
                          ),
                        ),
                    if (drafts.isNotEmpty &&
                        !drafts.any(
                          (d) => '${d.name.text} ${d.type.text}'
                              .toLowerCase()
                              .contains(query),
                        ))
                      const SidebarEmpty(
                        message: 'No se encontraron arquitectos',
                      ),
                  ],
                ),
        ),
        Expanded(
          child: AppDetailView(
            title: drafts.isEmpty
                ? 'Arquitectos'
                : drafts[selected].name.text.isEmpty
                ? 'Nuevo arquitecto'
                : drafts[selected].name.text,
            actions: drafts.isEmpty
                ? const []
                : [
                    TextButton(
                      onPressed: saving || drafts[selected].isNew
                          ? null
                          : () => _newDraft(duplicate: true),
                      child: const Text('Duplicar'),
                    ),
                    if (!drafts[selected].isBase)
                      AppTooltip(
                        label: drafts[selected].isNew
                            ? 'Descartar borrador'
                            : 'Eliminar arquitecto',
                        child: IconButton(
                          onPressed: saving ? null : _remove,
                          icon: const Icon(Icons.delete_outline, size: 18),
                        ),
                      ),
                    const SizedBox(width: 12),
                  ],
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 16),
              child: Align(
                alignment: Alignment.topCenter,
                child: ConstrainedBox(
                  constraints: const BoxConstraints(maxWidth: 712),
                  child: _editor(context),
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }

  Widget _editor(BuildContext context) {
    if (loading) {
      return const Center(child: CircularProgressIndicator());
    }
    if (drafts.isEmpty) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(error ?? 'No se pudieron leer los arquitectos.'),
            TextButton(onPressed: _load, child: const Text('Reintentar')),
          ],
        ),
      );
    }
    final d = drafts[selected];
    final guide = d.isGuide;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Expanded(
          child: Scrollbar(
            controller: scroll,
            thumbVisibility: true,
            child: SingleChildScrollView(
              controller: scroll,
              child: FocusTraversalGroup(
                child: Form(
                  key: d.form,
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      const SizedBox(height: 8),
                      TextFormField(
                        key: ValueKey('${d.id}-name'),
                        controller: d.name,
                        enabled: !saving,
                        decoration: const InputDecoration(labelText: 'Nombre'),
                        validator: (v) =>
                            v == null ||
                                v.trim().isEmpty ||
                                v.trim().runes.length > 200 ||
                                v.runes.any((c) => c < 32 || c == 127)
                            ? 'Introduce un nombre de una línea (máximo 200 caracteres).'
                            : null,
                        onChanged: (_) => setState(() => confirmation = null),
                      ),
                      const SizedBox(height: 16),
                      TextFormField(
                        key: ValueKey('${d.id}-type'),
                        controller: d.type,
                        enabled: !saving && !d.isBase,
                        decoration: InputDecoration(
                          labelText: 'Tipo',
                          helperText: d.isBase
                              ? 'Arquitecto base de Gennie.'
                              : 'bot, helper, guide o un tipo propio.',
                        ),
                        validator: (v) =>
                            v == null ||
                                v.trim().isEmpty ||
                                v.trim().runes.length > 80 ||
                                v.runes.any((c) => c < 32 || c == 127)
                            ? 'Introduce un tipo de una línea (máximo 80 caracteres).'
                            : null,
                        onChanged: (_) => setState(() => confirmation = null),
                      ),
                      const SizedBox(height: 12),
                      Text(switch (d.id) {
                        'gennie' =>
                          'Conversa contigo y coordina las delegaciones.',
                        'plugin_builder' =>
                          'Construye y verifica plugins en su propia sesión técnica.',
                        'proactivity_guide' =>
                          'Clasifica información para preparar propuestas al bot.',
                        _ =>
                          'Instancia reutilizable en los flujos que la referencien. Crear o guardar no inicia ejecuciones.',
                      }, style: const TextStyle(fontSize: 13)),
                      if (d.id == 'plugin_builder')
                        _toggle(d, 'enabled', 'Permitir construir plugins'),
                      if (!d.isBase && !guide)
                        _toggle(d, 'enabled', 'Habilitar arquitecto'),
                      if (guide) ...[
                        _toggle(
                          d,
                          'enabled',
                          'Permitir consultas remotas bajo demanda',
                        ),
                        const Text(
                          'Usa OpenRouter y requiere su clave configurada. Activarla no inicia consultas. Las revisiones periódicas requieren consentimiento propio en cada configuración de Encargos.',
                          style: TextStyle(fontSize: 12),
                        ),
                      ],
                      const SizedBox(height: 16),
                      TextFormField(
                        key: ValueKey('${d.id}-personality'),
                        controller: d.personality,
                        focusNode: d.personalityFocus,
                        enabled: !saving,
                        minLines: 2,
                        maxLines: 4,
                        decoration: const InputDecoration(
                          labelText: 'Personalidad',
                          helperText: 'Tono y forma de responder. Opcional.',
                        ),
                        validator: (v) => _textError(v, 1000),
                        onChanged: (_) => setState(() => confirmation = null),
                      ),
                      const SizedBox(height: 12),
                      const Text(
                        'La personalización respeta el rol, los permisos y lo que hayas autorizado.',
                        style: TextStyle(fontSize: 12),
                      ),
                      ExpansionTile(
                        key: ValueKey('${d.id}-advanced'),
                        controller: d.expansion,
                        expansionAnimationStyle: AnimationStyle(
                          duration: MediaQuery.disableAnimationsOf(context)
                              ? Duration.zero
                              : const Duration(milliseconds: 180),
                          reverseDuration:
                              MediaQuery.disableAnimationsOf(context)
                              ? Duration.zero
                              : const Duration(milliseconds: 180),
                        ),
                        initiallyExpanded: d.advanced,
                        maintainState: true,
                        tilePadding: EdgeInsets.zero,
                        childrenPadding: EdgeInsets.zero,
                        onExpansionChanged: (v) => d.advanced = v,
                        title: const Text(
                          'Opciones avanzadas',
                          style: TextStyle(fontSize: 13),
                        ),
                        children: [
                          const SizedBox(height: 8),
                          if (!d.isBase) ...[
                            DropdownButtonFormField<String>(
                              key: ValueKey('${d.id}-provider'),
                              initialValue: d.profile['provider'] as String,
                              decoration: const InputDecoration(
                                labelText: 'Proveedor',
                              ),
                              items: const [
                                DropdownMenuItem(
                                  value: 'codex_chatgpt',
                                  child: Text('Codex · ChatGPT'),
                                ),
                                DropdownMenuItem(
                                  value: 'openrouter_decisions',
                                  child: Text('OpenRouter Decisions'),
                                ),
                              ],
                              onChanged: saving
                                  ? null
                                  : (v) => setState(() {
                                      d.profile['provider'] = v!;
                                      d.model.text = v == 'codex_chatgpt'
                                          ? 'gpt-6-luna'
                                          : 'typesafe/jev-1.14';
                                      d.profile['model'] = d.model.text;
                                      d.profile['effort'] = v == 'codex_chatgpt'
                                          ? 'low'
                                          : null;
                                      if (v == 'openrouter_decisions') {
                                        for (final k in [
                                          'read_files',
                                          'write_files',
                                          'run_commands',
                                          'local_actions',
                                        ]) {
                                          d.profile[k] = false;
                                        }
                                      }
                                      confirmation = null;
                                    }),
                            ),
                            const SizedBox(height: 16),
                          ],
                          Text(
                            guide
                                ? 'Proveedor: OpenRouter Decisions'
                                : 'Proveedor: Codex · sesión de ChatGPT',
                            style: const TextStyle(fontSize: 12),
                          ),
                          const SizedBox(height: 16),
                          if (guide)
                            DropdownButtonFormField<String>(
                              key: ValueKey('${d.id}-model'),
                              initialValue: d.profile['model'] as String,
                              isExpanded: true,
                              decoration: const InputDecoration(
                                labelText: 'Modelo',
                              ),
                              items:
                                  [
                                        'typesafe/jev-1.14',
                                        'openai/gpt-6-luna-decisions',
                                      ]
                                      .map(
                                        (v) => DropdownMenuItem(
                                          value: v,
                                          child: Text(
                                            v,
                                            overflow: TextOverflow.ellipsis,
                                          ),
                                        ),
                                      )
                                      .toList(),
                              onChanged: saving
                                  ? null
                                  : (v) => setState(() {
                                      d.profile['model'] = v!;
                                      d.model.text = v;
                                      confirmation = null;
                                    }),
                            )
                          else ...[
                            TextFormField(
                              key: ValueKey('${d.id}-model'),
                              controller: d.model,
                              focusNode: d.modelFocus,
                              enabled: !saving,
                              decoration: const InputDecoration(
                                labelText: 'Modelo',
                                helperText:
                                    'Se comprueba su disponibilidad al conectar.',
                                helperMaxLines: 2,
                              ),
                              validator: _modelError,
                              onChanged: (_) =>
                                  setState(() => confirmation = null),
                            ),
                            const SizedBox(height: 16),
                            DropdownButtonFormField<String>(
                              key: ValueKey('${d.id}-effort'),
                              initialValue: d.profile['effort'] as String,
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
                              onChanged: saving
                                  ? null
                                  : (v) => setState(() {
                                      d.profile['effort'] = v;
                                      confirmation = null;
                                    }),
                            ),
                            const SizedBox(height: 16),
                            Text(
                              d.id == 'gennie'
                                  ? 'Estos permisos también limitan las delegaciones.'
                                  : 'Permisos de esta instancia. Al reutilizarla se aplican dentro de los límites del bot.',
                              style: const TextStyle(fontSize: 12),
                            ),
                            _toggle(d, 'read_files', 'Leer archivos'),
                            _toggle(d, 'write_files', 'Escribir archivos'),
                            _toggle(d, 'run_commands', 'Ejecutar comandos'),
                            _toggle(
                              d,
                              'local_actions',
                              'Controlar aplicaciones locales',
                            ),
                          ],
                          const SizedBox(height: 16),
                          TextFormField(
                            key: ValueKey('${d.id}-instructions'),
                            controller: d.instructions,
                            focusNode: d.instructionsFocus,
                            enabled: !saving,
                            minLines: 3,
                            maxLines: 8,
                            decoration: const InputDecoration(
                              labelText: 'Instrucciones adicionales',
                              helperText:
                                  'Preferencias para trabajar dentro de este rol.',
                              helperMaxLines: 2,
                            ),
                            validator: (v) => _textError(v, 6000),
                            onChanged: (_) =>
                                setState(() => confirmation = null),
                          ),
                          const SizedBox(height: 16),
                        ],
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ),
        ),
        if (error != null)
          ConstrainedBox(
            constraints: const BoxConstraints(maxHeight: 80),
            child: SingleChildScrollView(
              child: Text(
                error!,
                style: TextStyle(
                  fontSize: 12,
                  color: Theme.of(context).colorScheme.error,
                ),
              ),
            ),
          ),
        if (confirmation != null)
          Semantics(
            liveRegion: true,
            child: Text(confirmation!, style: const TextStyle(fontSize: 12)),
          ),
        const SizedBox(height: 12),
        Align(
          alignment: Alignment.centerRight,
          child: Wrap(
            alignment: WrapAlignment.end,
            spacing: 8,
            runSpacing: 8,
            children: [
              if (error != null)
                TextButton(
                  onPressed: saving ? null : _load,
                  child: const Text('Descartar y recargar'),
                ),
              FilledButton(
                onPressed: saving ? null : _save,
                child: Text(
                  saving
                      ? 'Guardando…'
                      : d.isNew
                      ? 'Crear arquitecto'
                      : 'Guardar cambios',
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }
}
