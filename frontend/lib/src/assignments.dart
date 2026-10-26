import 'dart:convert';
import 'dart:math';
import 'package:flutter/material.dart';
import 'package:window_manager/window_manager.dart';
import 'backend.dart';
import 'assignment_logo.dart';
import 'sidebar.dart';

const assignmentKinds = {
  'one_time': 'Puntual',
  'recurring': 'Recurrente',
  'goal': 'Objetivo',
};
const assignmentPriorities = {
  'low': 'Baja',
  'normal': 'Normal',
  'high': 'Alta',
  'critical': 'Crítica',
};
const assignmentStatuses = {
  'recorded': 'Registrado',
  'paused': 'Pausado',
  'completed': 'Completado',
  'cancelled': 'Cancelado',
};

class AssignmentsView extends StatefulWidget {
  const AssignmentsView({
    super.key,
    required this.state,
    this.sidebarWidth = 260,
  });
  final DesktopState state;
  final double sidebarWidth;
  @override
  State<AssignmentsView> createState() => _AssignmentsViewState();
}

class _AssignmentsViewState extends State<AssignmentsView> {
  final search = TextEditingController();
  String? selected;
  bool creating = false;
  bool editing = false;
  int generation = 0;
  @override
  void dispose() {
    search.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final records = widget.state.snapshot?.assignments ?? <AssignmentRecord>[];
    final query = search.text.trim().toLowerCase();
    final filtered = records
        .where(
          (r) => (jsonDecode(r.document)['title'] as String)
              .toLowerCase()
              .contains(query),
        )
        .toList();
    final matches = records.where((r) => r.id == selected);
    final record = matches.isEmpty ? null : matches.first;
    final enabled = widget.state.snapshot?.assignmentsEnabled == true;
    final hasSelection = creating || record != null;
    final ink = sidebarInk(context);
    final dark = Theme.of(context).brightness == Brightness.dark;
    final divider = BorderSide(color: ink.withValues(alpha: .10));
    return LayoutBuilder(
      builder: (context, size) {
        final width = widget.sidebarWidth
            .clamp(216.0, (size.maxWidth - 320).clamp(216.0, 420.0))
            .toDouble();
        return Row(
          children: [
            AppSidebar(
              width: width,
              title: 'Encargos',
              actionLabel: 'Nuevo encargo',
              onAction: !enabled
                  ? null
                  : () => setState(() {
                      selected = null;
                      creating = true;
                      editing = true;
                      generation++;
                    }),
              searchController: search,
              searchHint: 'Buscar encargos…',
              searchKey: const ValueKey('assignments-search'),
              onSearch: (_) => setState(() {}),
              child: !enabled
                  ? const SidebarEmpty(message: 'Encargos desactivados')
                  : filtered.isEmpty
                  ? SidebarEmpty(
                      message: query.isEmpty
                          ? 'No hay contenido disponible'
                          : 'No se encontraron encargos',
                    )
                  : ListView.builder(
                      padding: const EdgeInsets.symmetric(horizontal: 8),
                      itemCount: filtered.length,
                      itemBuilder: (context, i) {
                        final r = filtered[i];
                        final d =
                            jsonDecode(r.document) as Map<String, dynamic>;
                        return Padding(
                          padding: const EdgeInsets.only(bottom: 2),
                          child: Semantics(
                            selected: selected == r.id,
                            child: SidebarTile(
                              leading: AssignmentLogo(document: d),
                              title: d['title'] as String,
                              subtitle:
                                  '${assignmentKinds[d['kind']]} · ${assignmentStatuses[d['status']]}',
                              selected: selected == r.id,
                              onSelect: () => setState(() {
                                selected = r.id;
                                creating = false;
                                editing = false;
                                generation++;
                              }),
                            ),
                          ),
                        );
                      },
                    ),
            ),
            Expanded(
              child: Column(
                children: [
                  Container(
                    key: const ValueKey('assignment-content-header'),
                    height: 56,
                    decoration: BoxDecoration(
                      color: Colors.white.withValues(alpha: dark ? .025 : .26),
                      border: Border(bottom: divider),
                    ),
                    padding: const EdgeInsets.symmetric(horizontal: 24),
                    child: DragToMoveArea(
                      child: Align(
                        alignment: Alignment.centerLeft,
                        child: Text(
                          creating
                              ? 'Nuevo encargo'
                              : record == null
                              ? 'Encargos'
                              : jsonDecode(record.document)['title'] as String,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: TextStyle(
                            fontFamily: 'Inter',
                            fontSize: 14,
                            fontWeight: FontWeight.w500,
                            color: ink,
                          ),
                        ),
                      ),
                    ),
                  ),
                  Expanded(
                    child: hasSelection && enabled
                        ? !editing && record != null
                              ? AssignmentProfile(
                                  key: ValueKey(record.id),
                                  record: record,
                                  onEdit: () => setState(() => editing = true),
                                )
                              : AssignmentEditor(
                                  key: ValueKey('$selected-$generation'),
                                  state: widget.state,
                                  record: record,
                                  onBack: () => setState(() {
                                    creating = false;
                                    editing = false;
                                  }),
                                  onReload: () => setState(() => generation++),
                                  onSaved: (id) => setState(() {
                                    creating = false;
                                    editing = false;
                                    selected = id;
                                  }),
                                )
                        : ContentEmpty(
                            title: 'Encargos',
                            message: enabled
                                ? 'No hay contenido disponible'
                                : 'El módulo de encargos está desactivado.',
                          ),
                  ),
                ],
              ),
            ),
          ],
        );
      },
    );
  }
}

/// Ficha de consulta; los datos guardados solo cambian desde el editor.
class AssignmentProfile extends StatelessWidget {
  const AssignmentProfile({
    super.key,
    required this.record,
    required this.onEdit,
  });
  final AssignmentRecord record;
  final VoidCallback onEdit;

  String schedule(BuildContext context, Map<String, dynamic> d) {
    final recurrence = d['recurrence'] as Map<String, dynamic>?;
    if (d['kind'] == 'recurring' && recurrence != null) {
      const days = ['Lun', 'Mar', 'Mié', 'Jue', 'Vie', 'Sáb', 'Dom'];
      final weekdays = (recurrence['weekdays'] as List? ?? [])
          .whereType<int>()
          .where((day) => day >= 1 && day <= 7)
          .map((day) => days[day - 1])
          .join(', ');
      final frequency = recurrence['frequency'] == 'daily'
          ? 'Todos los días'
          : weekdays.isEmpty
          ? 'Cada semana'
          : weekdays;
      return '$frequency · ${recurrence['local_time']}';
    }
    final date = DateTime.tryParse(d['notify_at'] as String? ?? '')?.toLocal();
    if (date == null) return 'Sin fecha límite';
    return '${MaterialLocalizations.of(context).formatMediumDate(date)} · ${TimeOfDay.fromDateTime(date).format(context)}';
  }

  @override
  Widget build(BuildContext context) {
    final d = jsonDecode(record.document) as Map<String, dynamic>;
    final apps = d['applications'] as List? ?? [];
    final preparation = d['preparation'] as Map<String, dynamic>? ?? {};
    final followUp = d['follow_up'] as Map<String, dynamic>?;
    final ink = sidebarInk(context);
    final body = TextStyle(
      fontFamily: 'Inter',
      fontSize: 13,
      height: 1.6,
      color: ink,
    );
    Widget detail(String label, Object? value) {
      final text = value is List ? value.join('\n') : value?.toString() ?? '';
      if (text.trim().isEmpty) return const SizedBox.shrink();
      return Padding(
        padding: const EdgeInsets.only(bottom: 20),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              label,
              style: body.copyWith(fontSize: 12, fontWeight: FontWeight.w600),
            ),
            const SizedBox(height: 4),
            SelectableText(text, style: body),
          ],
        ),
      );
    }

    return DefaultTextStyle(
      style: body,
      child: Align(
        alignment: Alignment.topCenter,
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 760),
          child: ListView(
            padding: const EdgeInsets.all(24),
            children: [
              Align(
                alignment: Alignment.centerRight,
                child: OutlinedButton.icon(
                  onPressed: onEdit,
                  style: OutlinedButton.styleFrom(
                    foregroundColor: ink,
                    side: BorderSide(color: ink.withValues(alpha: .2)),
                  ),
                  icon: const Icon(Icons.edit_outlined, size: 16),
                  label: const Text('Editar'),
                ),
              ),
              const SizedBox(height: 16),
              Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  AssignmentLogo(document: d, size: 56),
                  const SizedBox(width: 16),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          apps.isEmpty
                              ? 'Encargo'
                              : apps.map((a) => a['name']).join(' · '),
                          style: body.copyWith(
                            fontSize: 12,
                            color: ink.withValues(alpha: .7),
                          ),
                        ),
                        const SizedBox(height: 4),
                        Semantics(
                          header: true,
                          child: Text(
                            d['title'] as String,
                            style: body.copyWith(
                              fontSize: 22,
                              height: 1.25,
                              fontWeight: FontWeight.w600,
                            ),
                          ),
                        ),
                      ],
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 16),
              Wrap(
                spacing: 8,
                runSpacing: 8,
                children: [
                  for (final label in [
                    assignmentStatuses[d['status']],
                    assignmentKinds[d['kind']],
                    'Importancia ${assignmentPriorities[d['priority']]?.toLowerCase()}',
                  ])
                    Container(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 10,
                        vertical: 4,
                      ),
                      decoration: BoxDecoration(
                        color: ink.withValues(alpha: .07),
                        borderRadius: BorderRadius.circular(6),
                      ),
                      child: Text(
                        label ?? '',
                        style: body.copyWith(fontSize: 11),
                      ),
                    ),
                ],
              ),
              const SizedBox(height: 24),
              SelectableText(d['description'] as String, style: body),
              const SizedBox(height: 24),
              Container(
                padding: const EdgeInsets.all(16),
                decoration: BoxDecoration(
                  border: Border.all(color: ink.withValues(alpha: .12)),
                  borderRadius: BorderRadius.circular(12),
                ),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Icon(Icons.schedule_outlined, size: 20, color: ink),
                    const SizedBox(width: 12),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            'Cuándo recibir la información',
                            style: body.copyWith(
                              fontSize: 12,
                              color: ink.withValues(alpha: .7),
                            ),
                          ),
                          const SizedBox(height: 4),
                          Text(
                            schedule(context, d),
                            style: body.copyWith(fontWeight: FontWeight.w500),
                          ),
                          if (d['kind'] == 'recurring')
                            Text(
                              d['timezone'] as String? ?? '',
                              style: body.copyWith(
                                fontSize: 11,
                                color: ink.withValues(alpha: .7),
                              ),
                            ),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
              const SizedBox(height: 24),
              detail('Para qué', d['reason']),
              Theme(
                data: Theme.of(
                  context,
                ).copyWith(dividerColor: Colors.transparent),
                child: ExpansionTile(
                  tilePadding: EdgeInsets.zero,
                  childrenPadding: const EdgeInsets.only(top: 12),
                  iconColor: ink,
                  collapsedIconColor: ink,
                  title: Text(
                    'Detalles del encargo',
                    style: body.copyWith(fontWeight: FontWeight.w500),
                  ),
                  children: [
                    Align(
                      alignment: Alignment.centerLeft,
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          detail('Contexto', d['context']),
                          if (apps.isNotEmpty)
                            detail(
                              'Aplicaciones',
                              apps
                                  .map(
                                    (a) =>
                                        '${a['name']}${a['url'] == null ? '' : ' · ${a['url']}'}',
                                  )
                                  .toList(),
                            ),
                          if (preparation['interval_minutes'] != null)
                            detail(
                              'Preparación interna',
                              'Cada ${preparation['interval_minutes']} minutos. Esta cadencia no cambia la hora del aviso.',
                            ),
                          detail(
                            'Motivo de la cadencia',
                            preparation['rationale'],
                          ),
                          detail('Pasos de preparación', preparation['steps']),
                          if (followUp != null) ...[
                            detail('Qué observar', followUp['scope']),
                            detail('Aspectos a valorar', followUp['signals']),
                            detail(
                              'Cómo decidir si intervenir',
                              followUp['decision_guidance'],
                            ),
                            detail(
                              'Acciones delegadas',
                              followUp['action_scope'],
                            ),
                          ],
                        ],
                      ),
                    ),
                  ],
                ),
              ),
              const SizedBox(height: 16),
            ],
          ),
        ),
      ),
    );
  }
}

class AssignmentEditor extends StatefulWidget {
  const AssignmentEditor({
    super.key,
    required this.state,
    this.record,
    required this.onBack,
    required this.onReload,
    required this.onSaved,
  });
  final DesktopState state;
  final AssignmentRecord? record;
  final VoidCallback onBack, onReload;
  final ValueChanged<String?> onSaved;
  @override
  State<AssignmentEditor> createState() => _AssignmentEditorState();
}

class _AssignmentEditorState extends State<AssignmentEditor> {
  final fields = <String, TextEditingController>{};
  final form = GlobalKey<FormState>();
  final applications = <Map<String, dynamic>>[];
  late String kind, priority, status, frequency;
  late String requestKey;
  late final int? originalRevision;
  final weekdays = <int>{};
  DateTime? notifyAt;
  bool saving = false;
  bool followUpEnabled = false;
  String? error;
  @override
  void initState() {
    super.initState();
    final d = widget.record == null
        ? <String, dynamic>{}
        : jsonDecode(widget.record!.document) as Map<String, dynamic>;
    final recurrence = d['recurrence'] as Map<String, dynamic>? ?? {};
    final preparation = d['preparation'] as Map<String, dynamic>? ?? {};
    final followUp = d['follow_up'] as Map<String, dynamic>?;
    followUpEnabled = followUp != null;
    kind = d['kind'] as String? ?? 'recurring';
    priority = d['priority'] as String? ?? 'normal';
    status = d['status'] as String? ?? 'recorded';
    frequency = recurrence['frequency'] as String? ?? 'daily';
    weekdays.addAll((recurrence['weekdays'] as List? ?? []).cast<int>());
    notifyAt = DateTime.tryParse(d['notify_at'] as String? ?? '')?.toLocal();
    originalRevision = widget.record?.revision;
    requestKey =
        'ui-${DateTime.now().microsecondsSinceEpoch}-${Random.secure().nextInt(1 << 32)}';
    final values = <String, String>{
      'title': d['title'] as String? ?? '',
      'description': d['description'] as String? ?? '',
      'context': d['context'] as String? ?? '',
      'reason': d['reason'] as String? ?? '',
      'timezone':
          d['timezone'] as String? ?? widget.state.snapshot?.timezone ?? '',
      'time': recurrence['local_time'] as String? ?? '08:00',
      'interval': preparation['interval_minutes']?.toString() ?? '',
      'rationale': preparation['rationale'] as String? ?? '',
      'steps': (preparation['steps'] as List? ?? []).join('\n'),
      'followUpScope': followUp?['scope'] as String? ?? '',
      'signals': (followUp?['signals'] as List? ?? []).join('\n'),
      'guidance': followUp?['decision_guidance'] as String? ?? '',
      'actionScope': followUp?['action_scope'] as String? ?? '',
    };
    for (final e in values.entries) {
      fields[e.key] = TextEditingController(text: e.value);
    }
    for (final a in d['applications'] as List? ?? []) {
      applications.add(Map<String, dynamic>.from(a as Map));
    }
  }

  @override
  void dispose() {
    for (final c in fields.values) {
      c.dispose();
    }
    super.dispose();
  }

  String? optional(String key) {
    final v = fields[key]!.text.trim();
    return v.isEmpty ? null : v;
  }

  Widget field(
    String key,
    String label, {
    int lines = 1,
    bool required = false,
    String? hint,
  }) => Padding(
    padding: const EdgeInsets.only(bottom: 16),
    child: TextFormField(
      controller: fields[key],
      enabled: !saving,
      maxLines: lines,
      decoration: InputDecoration(
        labelText: label,
        helperText: hint,
        helperMaxLines: 4,
      ),
      validator: required
          ? (v) => v == null || v.trim().isEmpty ? 'Completa este campo' : null
          : null,
    ),
  );
  Widget choice(
    String label,
    String value,
    Map<String, String> values,
    ValueChanged<String> change,
  ) => Padding(
    padding: const EdgeInsets.only(bottom: 16),
    child: DropdownButtonFormField<String>(
      key: ValueKey('$label-$value'),
      initialValue: value,
      isExpanded: true,
      decoration: InputDecoration(labelText: label),
      items: [
        for (final e in values.entries)
          DropdownMenuItem(value: e.key, child: Text(e.value)),
      ],
      onChanged: saving ? null : (v) => setState(() => change(v!)),
    ),
  );
  Future<void> pickDate() async {
    final initial = notifyAt ?? DateTime.now();
    final date = await showDatePicker(
      context: context,
      initialDate: initial,
      firstDate: DateTime(2000),
      lastDate: DateTime(2200),
    );
    if (date == null || !mounted) return;
    final time = await showTimePicker(
      context: context,
      initialTime: TimeOfDay.fromDateTime(initial),
    );
    if (time != null && mounted) {
      setState(() {
        notifyAt = DateTime(
          date.year,
          date.month,
          date.day,
          time.hour,
          time.minute,
        );
        fields['timezone']!.text = widget.state.snapshot?.timezone ?? 'UTC';
      });
    }
  }

  Future<void> save() async {
    if (followUpEnabled &&
        [
          'followUpScope',
          'guidance',
          'interval',
          'rationale',
        ].any((key) => optional(key) == null)) {
      setState(
        () => error =
            'Completa qué observar, cómo decidir y la cadencia con su motivo.',
      );
      return;
    }
    if (!form.currentState!.validate()) return;
    setState(() {
      saving = true;
      error = null;
    });
    final interval = optional('interval');
    if (interval != null && int.tryParse(interval) == null) {
      setState(() {
        saving = false;
        error =
            'La cadencia de preparación debe ser un número entero de minutos.';
      });
      return;
    }
    final at = notifyAt?.toUtc().toIso8601String().replaceFirst('.000Z', 'Z');
    final document = {
      'title': fields['title']!.text.trim(),
      'description': fields['description']!.text.trim(),
      'context': optional('context'),
      'reason': optional('reason'),
      'kind': kind,
      'priority': priority,
      'status': status,
      'timezone': fields['timezone']!.text.trim(),
      'notify_at': kind == 'recurring' ? null : at,
      'recurrence': kind != 'recurring'
          ? null
          : {
              'frequency': frequency,
              'local_time': fields['time']!.text.trim(),
              'weekdays': frequency == 'daily'
                  ? <int>[]
                  : (weekdays.toList()..sort()),
            },
      'preparation': {
        'interval_minutes': interval == null ? null : int.parse(interval),
        'rationale': optional('rationale'),
        'steps': fields['steps']!.text
            .split('\n')
            .map((s) => s.trim())
            .where((s) => s.isNotEmpty)
            .toList(),
      },
      if (followUpEnabled)
        'follow_up': {
          'scope': fields['followUpScope']!.text.trim(),
          'signals': fields['signals']!.text
              .split('\n')
              .map((s) => s.trim())
              .where((s) => s.isNotEmpty)
              .toList(),
          'decision_guidance': fields['guidance']!.text.trim(),
          'action_scope': optional('actionScope'),
        },
      'applications': applications,
    };
    try {
      final previousIds =
          widget.state.snapshot?.assignments.map((r) => r.id).toSet() ??
          <String>{};
      final snapshot = await widget.state.backend.saveAssignment(
        widget.record?.id,
        originalRevision,
        requestKey,
        jsonEncode(document),
      );
      widget.state.accept(snapshot);
      if (!mounted) return;
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(const SnackBar(content: Text('Encargo guardado.')));
      final created = snapshot.assignments.where(
        (r) => !previousIds.contains(r.id),
      );
      widget.onSaved(
        widget.record?.id ?? (created.length == 1 ? created.single.id : null),
      );
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
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final ink = sidebarInk(context);
    final body = TextStyle(
      fontFamily: 'Inter',
      fontSize: 13,
      fontWeight: FontWeight.w400,
      height: 1.5,
      color: ink,
    );
    final border = OutlineInputBorder(
      borderRadius: BorderRadius.circular(8),
      borderSide: BorderSide(color: ink.withValues(alpha: .16)),
    );
    return Theme(
      data: theme.copyWith(
        colorScheme: theme.colorScheme.copyWith(
          primary: ink,
          onPrimary: theme.colorScheme.surface,
        ),
        textTheme: theme.textTheme.copyWith(
          bodyLarge: body,
          bodyMedium: body,
          bodySmall: body.copyWith(fontSize: 11),
          titleMedium: body,
          labelLarge: body.copyWith(fontSize: 12),
        ),
        inputDecorationTheme: InputDecorationTheme(
          filled: false,
          isDense: true,
          contentPadding: const EdgeInsets.symmetric(
            horizontal: 12,
            vertical: 12,
          ),
          border: border,
          enabledBorder: border,
          disabledBorder: border,
          focusedBorder: border.copyWith(
            borderSide: BorderSide(color: ink.withValues(alpha: .5)),
          ),
          labelStyle: body.copyWith(
            fontSize: 12,
            color: ink.withValues(alpha: .65),
          ),
          helperStyle: body.copyWith(
            fontSize: 11,
            color: ink.withValues(alpha: .62),
          ),
        ),
      ),
      child: DefaultTextStyle.merge(
        style: body,
        child: Form(
          key: form,
          child: ListView(
            padding: const EdgeInsets.all(24),
            children: [
              Row(
                children: [
                  Expanded(
                    child: Text(
                      widget.record == null
                          ? 'Nuevo encargo'
                          : 'Editar encargo',
                      style: const TextStyle(
                        fontSize: 14,
                        fontWeight: FontWeight.w500,
                      ),
                    ),
                  ),
                  TextButton(
                    onPressed: saving ? null : widget.onBack,
                    child: const Text('Cancelar'),
                  ),
                ],
              ),
              const SizedBox(height: 16),
              const Text(
                'Define qué necesitas revisar y cuándo recibir el resultado.',
                style: TextStyle(
                  fontFamily: 'Inter',
                  fontSize: 12,
                  color: Color(0xff737373),
                ),
              ),
              const SizedBox(height: 24),
              field('title', 'Título', required: true),
              field(
                'description',
                'Qué debe hacer Gennie',
                lines: 3,
                required: true,
              ),
              choice('Tipo', kind, assignmentKinds, (v) => kind = v),
              choice(
                'Importancia',
                priority,
                assignmentPriorities,
                (v) => priority = v,
              ),
              choice(
                'Estado del registro',
                status,
                assignmentStatuses,
                (v) => status = v,
              ),
              field('reason', 'Para qué lo necesitas (opcional)', lines: 2),
              field('context', 'Contexto (opcional)', lines: 3),
              const Text(
                'Cuándo recibir la información',
                style: TextStyle(
                  fontFamily: 'Inter',
                  fontSize: 13,
                  fontWeight: FontWeight.w500,
                ),
              ),
              const SizedBox(height: 16),
              if (kind == 'recurring') ...[
                field(
                  'timezone',
                  'Zona horaria',
                  required: true,
                  hint: 'Ejemplo: America/Bogota',
                ),
                choice('Repetición', frequency, const {
                  'daily': 'Todos los días',
                  'weekly': 'Cada semana',
                }, (v) => frequency = v),
                field(
                  'time',
                  'Hora del aviso',
                  required: true,
                  hint: 'Formato de 24 horas: 08:00',
                ),
                if (frequency == 'weekly')
                  Padding(
                    padding: const EdgeInsets.only(bottom: 16),
                    child: Wrap(
                      spacing: 4,
                      children: [
                        for (var i = 1; i <= 7; i++)
                          FilterChip(
                            label: Text(
                              const [
                                'Lun',
                                'Mar',
                                'Mié',
                                'Jue',
                                'Vie',
                                'Sáb',
                                'Dom',
                              ][i - 1],
                            ),
                            selected: weekdays.contains(i),
                            onSelected: saving
                                ? null
                                : (v) => setState(() {
                                    if (v) {
                                      weekdays.add(i);
                                    } else {
                                      weekdays.remove(i);
                                    }
                                  }),
                          ),
                      ],
                    ),
                  ),
              ] else ...[
                Text(
                  kind == 'goal'
                      ? 'Fecha límite opcional. Se elige en la hora local del sistema.'
                      : 'Elige fecha y hora de aviso en la hora local del sistema.',
                ),
                Wrap(
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: [
                    TextButton(
                      onPressed: saving ? null : pickDate,
                      child: Text(
                        notifyAt == null
                            ? 'Elegir fecha y hora'
                            : '${MaterialLocalizations.of(context).formatMediumDate(notifyAt!)} · ${TimeOfDay.fromDateTime(notifyAt!).format(context)}',
                      ),
                    ),
                    if (notifyAt != null && kind == 'goal')
                      TextButton(
                        onPressed: saving
                            ? null
                            : () => setState(() => notifyAt = null),
                        child: const Text('Sin fecha límite'),
                      ),
                  ],
                ),
                const SizedBox(height: 16),
              ],
              const Text(
                'Preparación interna',
                style: TextStyle(
                  fontFamily: 'Inter',
                  fontSize: 13,
                  fontWeight: FontWeight.w500,
                ),
              ),
              const SizedBox(height: 8),
              const Text(
                'Esta cadencia indica cuándo reunir información. No cambia la hora del aviso.',
              ),
              const SizedBox(height: 16),
              field(
                'interval',
                'Revisar cada cuántos minutos (opcional)',
                required: followUpEnabled,
              ),
              field(
                'rationale',
                'Por qué usar esa cadencia',
                lines: 2,
                required: followUpEnabled,
              ),
              field('steps', 'Pasos de preparación (uno por línea)', lines: 4),
              const Text(
                'Seguimiento',
                style: TextStyle(fontSize: 13, fontWeight: FontWeight.w500),
              ),
              const SizedBox(height: 8),
              SwitchListTile.adaptive(
                key: const ValueKey('follow-up-policy'),
                contentPadding: EdgeInsets.zero,
                title: const Text(
                  'Seguimiento entre entregas',
                  style: TextStyle(fontSize: 13),
                ),
                subtitle: const Text(
                  'Define qué observar y cómo acompañarte.',
                  style: TextStyle(fontSize: 11),
                ),
                value: followUpEnabled,
                onChanged: saving
                    ? null
                    : (value) => setState(() {
                        followUpEnabled = value;
                        if (value) {
                          if (fields['interval']!.text.trim().isEmpty) {
                            fields['interval']!.text = '10';
                          }
                          if (fields['rationale']!.text.trim().isEmpty) {
                            fields['rationale']!.text =
                                'Detectar cambios y valorar si conviene intervenir entre entregas.';
                          }
                          if (fields['guidance']!.text.trim().isEmpty) {
                            fields['guidance']!.text =
                                'Valora los cambios, el contexto y el historial. Decide si basta con registrar, si conviene preguntar, avisar o proponer ayuda, o si corresponde una acción delegada. Conserva los cambios aunque no justifiquen interrumpir; evita repetir preguntas o avisos sin novedades relevantes.';
                          }
                        }
                      }),
              ),
              const SizedBox(height: 16),
              if (followUpEnabled) ...[
                field(
                  'followUpScope',
                  'Qué observar',
                  lines: 3,
                  required: true,
                  hint:
                      'Describe las fuentes y los cambios que interesa seguir y conservar.',
                ),
                const Text(
                  'Criterio de intervención',
                  style: TextStyle(fontSize: 13, fontWeight: FontWeight.w500),
                ),
                const SizedBox(height: 8),
                const Text(
                  'Gennie valora cada revisión según su contexto. La frecuencia de revisión no obliga a avisarte.',
                ),
                const SizedBox(height: 16),
                field(
                  'signals',
                  'Aspectos a valorar (uno por línea)',
                  lines: 4,
                  hint:
                      'Preguntas propias de este encargo. Puedes dejarlo vacío si basta con la orientación general.',
                ),
                field(
                  'guidance',
                  'Cómo decidir si intervenir',
                  lines: 5,
                  required: true,
                  hint:
                      'Cuándo guardar silencio, preguntar, avisar o proponer ayuda. Los criterios y plazos pueden adaptarse a la situación.',
                ),
                field(
                  'actionScope',
                  'Acciones delegadas (opcional)',
                  lines: 3,
                  hint:
                      'Solo acciones que hayas encargado expresamente, con sus límites. Vacío si no has delegado ejecución.',
                ),
              ],
              const Text(
                'Aplicaciones necesarias',
                style: TextStyle(
                  fontFamily: 'Inter',
                  fontSize: 13,
                  fontWeight: FontWeight.w500,
                ),
              ),
              const SizedBox(height: 8),
              const Text(
                'Indica las aplicaciones y fuentes que necesita esta tarea.',
              ),
              const SizedBox(height: 16),
              for (var i = 0; i < applications.length; i++) ...[
                TextFormField(
                  key: ValueKey('name-${identityHashCode(applications[i])}'),
                  initialValue: applications[i]['name'] as String? ?? '',
                  enabled: !saving,
                  decoration: const InputDecoration(
                    labelText: 'Nombre de la aplicación',
                  ),
                  onChanged: (v) => applications[i]['name'] = v,
                ),
                const SizedBox(height: 12),
                choice(
                  'Dónde se utiliza',
                  applications[i]['kind'] as String? ?? 'web',
                  const {'web': 'Web', 'desktop': 'En el ordenador'},
                  (v) {
                    applications[i]['kind'] = v;
                    if (v == 'desktop') applications[i]['url'] = null;
                  },
                ),
                if (applications[i]['kind'] == 'web')
                  TextFormField(
                    key: ValueKey('url-${identityHashCode(applications[i])}'),
                    initialValue: applications[i]['url'] as String? ?? '',
                    enabled: !saving,
                    decoration: const InputDecoration(
                      labelText: 'Dirección web (opcional)',
                    ),
                    onChanged: (v) => applications[i]['url'] = v.trim().isEmpty
                        ? null
                        : v.trim(),
                  ),
                Align(
                  alignment: Alignment.centerRight,
                  child: TextButton(
                    onPressed: saving
                        ? null
                        : () => setState(() => applications.removeAt(i)),
                    child: const Text('Quitar aplicación'),
                  ),
                ),
                const Divider(),
              ],
              Align(
                alignment: Alignment.centerLeft,
                child: TextButton.icon(
                  onPressed: saving
                      ? null
                      : () => setState(
                          () => applications.add({
                            'name': '',
                            'kind': 'web',
                            'url': null,
                            'plugin_available': false,
                            'manual_required': true,
                          }),
                        ),
                  icon: const Icon(Icons.add, size: 18),
                  label: const Text('Añadir aplicación'),
                ),
              ),
              if (error != null) ...[
                const SizedBox(height: 16),
                SelectableText(
                  error!,
                  style: TextStyle(color: Theme.of(context).colorScheme.error),
                ),
                if (widget.record != null)
                  Align(
                    alignment: Alignment.centerLeft,
                    child: TextButton(
                      onPressed: saving ? null : widget.onReload,
                      child: const Text('Descartar borrador y recargar'),
                    ),
                  ),
              ],
              const SizedBox(height: 24),
              Align(
                alignment: Alignment.centerRight,
                child: FilledButton(
                  onPressed: saving ? null : save,
                  child: Text(saving ? 'Guardando…' : 'Guardar encargo'),
                ),
              ),
              const SizedBox(height: 24),
            ],
          ),
        ),
      ),
    );
  }
}
