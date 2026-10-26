import 'dart:convert';
import 'dart:math';
import 'package:flutter/material.dart';
import 'package:window_manager/window_manager.dart';
import 'backend.dart';
import 'glass.dart';
import 'notice.dart';
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
  'active': 'Activo',
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
    final guidance = (followUp?['decision_guidance'] as String? ?? '').trim();
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

    bool hasValue(Object? value) => value is List
        ? value.isNotEmpty
        : value != null && value.toString().trim().isNotEmpty;
    Widget section(String title, IconData icon, List<Widget> children) =>
        Padding(
          padding: const EdgeInsets.only(bottom: 20),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  Icon(icon, size: 18, color: ink.withValues(alpha: .7)),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Semantics(
                      header: true,
                      child: Text(
                        title,
                        style: body.copyWith(fontWeight: FontWeight.w600),
                      ),
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 12),
              ...children,
            ],
          ),
        );

    return DefaultTextStyle(
      style: body,
      child: Align(
        alignment: Alignment.topCenter,
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 760),
          child: ListView(
            padding: const EdgeInsets.all(24),
            children: [
              Stack(
                children: [
                  Padding(
                    padding: const EdgeInsets.only(right: 44),
                    child: Row(
                      crossAxisAlignment: CrossAxisAlignment.center,
                      children: [
                        AssignmentLogo(document: d, size: 72),
                        const SizedBox(width: 16),
                        Expanded(
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Text(
                                'Encargo',
                                key: const ValueKey('assignment-profile-label'),
                                style: body.copyWith(
                                  fontSize: 12,
                                  fontWeight: FontWeight.w600,
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
                              if (apps.isNotEmpty) ...[
                                const SizedBox(height: 6),
                                Text(
                                  apps.map((a) => a['name']).join(' · '),
                                  style: body.copyWith(
                                    fontSize: 12,
                                    color: ink.withValues(alpha: .7),
                                  ),
                                ),
                              ],
                            ],
                          ),
                        ),
                      ],
                    ),
                  ),
                  Positioned(
                    top: 0,
                    right: 0,
                    child: AppTooltip(
                      label: 'Editar encargo',
                      child: Semantics(
                        label: 'Editar encargo',
                        child: IconButton(
                          key: const ValueKey('assignment-edit'),
                          onPressed: onEdit,
                          constraints: const BoxConstraints.tightFor(
                            width: 32,
                            height: 32,
                          ),
                          padding: EdgeInsets.zero,
                          color: ink,
                          icon: const Icon(Icons.edit_outlined, size: 16),
                        ),
                      ),
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
              detail('Para qué', d['reason']),
              Container(
                key: const ValueKey('assignment-delivery'),
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
                          if (guidance.isNotEmpty) ...[
                            Padding(
                              padding: const EdgeInsets.symmetric(vertical: 12),
                              child: Divider(
                                height: 1,
                                color: ink.withValues(alpha: .12),
                              ),
                            ),
                            Text(
                              'Criterio para avisarte entre entregas',
                              style: body.copyWith(
                                fontSize: 12,
                                fontWeight: FontWeight.w600,
                              ),
                            ),
                            const SizedBox(height: 4),
                            _ExpandableGuidance(text: guidance, style: body),
                          ],
                        ],
                      ),
                    ),
                  ],
                ),
              ),
              const SizedBox(height: 24),
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
                    if (hasValue(d['context']))
                      section('Contexto', Icons.notes_outlined, [
                        Padding(
                          padding: const EdgeInsets.only(bottom: 16),
                          child: SelectableText(
                            d['context'] as String,
                            style: body,
                          ),
                        ),
                      ]),
                    if (apps.isNotEmpty)
                      section('Aplicaciones', Icons.apps_outlined, [
                        for (final app in apps)
                          Padding(
                            padding: const EdgeInsets.only(bottom: 16),
                            child: Row(
                              crossAxisAlignment: CrossAxisAlignment.center,
                              children: [
                                AssignmentLogo(
                                  document: {
                                    'applications': [app],
                                  },
                                  size: 28,
                                ),
                                const SizedBox(width: 10),
                                Expanded(
                                  child: Column(
                                    crossAxisAlignment:
                                        CrossAxisAlignment.start,
                                    children: [
                                      SelectableText(
                                        app['name'] as String,
                                        style: body.copyWith(
                                          fontWeight: FontWeight.w500,
                                        ),
                                      ),
                                      if (hasValue(app['url']))
                                        SelectableText(
                                          app['url'] as String,
                                          style: body.copyWith(
                                            fontSize: 12,
                                            color: ink.withValues(alpha: .7),
                                          ),
                                        ),
                                    ],
                                  ),
                                ),
                              ],
                            ),
                          ),
                      ]),
                    if (preparation.values.any(hasValue))
                      _AssignmentDetailDisclosure(
                        title: 'Preparación interna',
                        icon: Icons.checklist_outlined,
                        summary: [
                          if (preparation['interval_minutes'] != null)
                            'Cada ${preparation['interval_minutes']} minutos',
                          if (hasValue(preparation['steps']))
                            '${(preparation['steps'] as List).length} pasos de preparación',
                        ].join(' · '),
                        style: body,
                        children: [
                          if (preparation['interval_minutes'] != null)
                            detail(
                              'Cadencia',
                              'Cada ${preparation['interval_minutes']} minutos. Esta cadencia no cambia la hora del aviso.',
                            ),
                          detail(
                            'Motivo de la cadencia',
                            preparation['rationale'],
                          ),
                          if (hasValue(preparation['steps'])) ...[
                            Text(
                              'Pasos de preparación',
                              style: body.copyWith(
                                fontSize: 12,
                                fontWeight: FontWeight.w600,
                              ),
                            ),
                            const SizedBox(height: 8),
                            for (final (index, step)
                                in (preparation['steps'] as List).indexed)
                              Padding(
                                padding: const EdgeInsets.only(bottom: 12),
                                child: Row(
                                  crossAxisAlignment: CrossAxisAlignment.start,
                                  children: [
                                    SizedBox(
                                      width: 24,
                                      child: Text(
                                        '${index + 1}.',
                                        style: body.copyWith(
                                          color: ink.withValues(alpha: .65),
                                        ),
                                      ),
                                    ),
                                    Expanded(
                                      child: SelectableText(
                                        step as String,
                                        style: body,
                                      ),
                                    ),
                                  ],
                                ),
                              ),
                            const SizedBox(height: 4),
                          ],
                        ],
                      ),
                    if (followUp != null &&
                        [
                          followUp['scope'],
                          followUp['signals'],
                          followUp['action_scope'],
                        ].any(hasValue))
                      _AssignmentDetailDisclosure(
                        title: 'Seguimiento',
                        icon: Icons.visibility_outlined,
                        summary: followUp['scope'] as String? ?? '',
                        style: body,
                        children: [
                          detail('Qué observar', followUp['scope']),
                          if (hasValue(followUp['signals'])) ...[
                            Text(
                              'Aspectos a valorar',
                              style: body.copyWith(
                                fontSize: 12,
                                fontWeight: FontWeight.w600,
                              ),
                            ),
                            const SizedBox(height: 8),
                            for (final signal in followUp['signals'] as List)
                              Padding(
                                padding: const EdgeInsets.only(bottom: 12),
                                child: Row(
                                  crossAxisAlignment: CrossAxisAlignment.start,
                                  children: [
                                    Text(
                                      '•',
                                      style: body.copyWith(
                                        color: ink.withValues(alpha: .65),
                                      ),
                                    ),
                                    const SizedBox(width: 10),
                                    Expanded(
                                      child: SelectableText(
                                        signal as String,
                                        style: body,
                                      ),
                                    ),
                                  ],
                                ),
                              ),
                            const SizedBox(height: 4),
                          ],
                          detail(
                            'Acciones delegadas',
                            followUp['action_scope'],
                          ),
                        ],
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

/// Despliegue de información secundaria sin superficies adicionales.
class _AssignmentDetailDisclosure extends StatefulWidget {
  const _AssignmentDetailDisclosure({
    required this.title,
    required this.icon,
    required this.summary,
    required this.style,
    required this.children,
  });
  final String title, summary;
  final IconData icon;
  final TextStyle style;
  final List<Widget> children;
  @override
  State<_AssignmentDetailDisclosure> createState() =>
      _AssignmentDetailDisclosureState();
}

class _AssignmentDetailDisclosureState
    extends State<_AssignmentDetailDisclosure> {
  bool expanded = false;
  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.only(bottom: 24),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Semantics(
          expanded: expanded,
          child: TextButton(
            onPressed: () => setState(() => expanded = !expanded),
            style: TextButton.styleFrom(
              foregroundColor: widget.style.color,
              overlayColor: Colors.transparent,
              splashFactory: NoSplash.splashFactory,
              padding: EdgeInsets.zero,
              minimumSize: const Size(0, 32),
              tapTargetSize: MaterialTapTargetSize.shrinkWrap,
            ),
            child: Row(
              children: [
                Icon(
                  widget.icon,
                  size: 18,
                  color: widget.style.color?.withValues(alpha: .7),
                ),
                const SizedBox(width: 10),
                Expanded(
                  child: Text(
                    widget.title,
                    style: widget.style.copyWith(fontWeight: FontWeight.w600),
                  ),
                ),
                Icon(
                  expanded ? Icons.expand_less : Icons.expand_more,
                  size: 18,
                ),
              ],
            ),
          ),
        ),
        if (!expanded && widget.summary.isNotEmpty)
          Padding(
            padding: const EdgeInsets.only(left: 28, top: 4),
            child: Text(
              widget.summary,
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
              style: widget.style.copyWith(
                fontSize: 12,
                color: widget.style.color?.withValues(alpha: .7),
              ),
            ),
          ),
        if (expanded)
          Padding(
            padding: const EdgeInsets.only(left: 28, top: 12),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: widget.children,
            ),
          ),
      ],
    ),
  );
}

/// Conserva la lectura compacta y permite consultar la política completa.
class _ExpandableGuidance extends StatefulWidget {
  const _ExpandableGuidance({required this.text, required this.style});
  final String text;
  final TextStyle style;

  @override
  State<_ExpandableGuidance> createState() => _ExpandableGuidanceState();
}

class _ExpandableGuidanceState extends State<_ExpandableGuidance> {
  bool expanded = false;

  @override
  void didUpdateWidget(covariant _ExpandableGuidance oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.text != widget.text) expanded = false;
  }

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, constraints) {
      final painter = TextPainter(
        text: TextSpan(text: widget.text, style: widget.style),
        maxLines: 3,
        textDirection: Directionality.of(context),
        textScaler: MediaQuery.textScalerOf(context),
        locale: Localizations.localeOf(context),
      )..layout(maxWidth: constraints.maxWidth);
      final overflows = painter.didExceedMaxLines;
      painter.dispose();
      return Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          if (expanded || !overflows)
            SelectableText(widget.text, style: widget.style)
          else
            Text(
              widget.text,
              style: widget.style,
              maxLines: 3,
              overflow: TextOverflow.ellipsis,
            ),
          if (overflows) ...[
            const SizedBox(height: 8),
            Semantics(
              expanded: expanded,
              child: TextButton(
                onPressed: () => setState(() => expanded = !expanded),
                style: ButtonStyle(
                  foregroundColor: WidgetStateProperty.resolveWith(
                    (states) => widget.style.color?.withValues(
                      alpha:
                          states.contains(WidgetState.hovered) ||
                              states.contains(WidgetState.focused)
                          ? 1
                          : .72,
                    ),
                  ),
                  overlayColor: const WidgetStatePropertyAll(
                    Colors.transparent,
                  ),
                  backgroundColor: const WidgetStatePropertyAll(
                    Colors.transparent,
                  ),
                  splashFactory: NoSplash.splashFactory,
                  padding: const WidgetStatePropertyAll(EdgeInsets.zero),
                  minimumSize: const WidgetStatePropertyAll(Size(0, 24)),
                  tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                  textStyle: WidgetStatePropertyAll(
                    widget.style.copyWith(
                      fontSize: 11,
                      fontWeight: FontWeight.w500,
                      decoration: TextDecoration.underline,
                    ),
                  ),
                ),
                child: Text(expanded ? 'Ver menos' : 'Ver más'),
              ),
            ),
          ],
        ],
      );
    },
  );
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
  final opened = <String>{};
  final fieldKeys = <String, GlobalKey>{};
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
    status = d['status'] as String? ?? 'active';
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
      key: fieldKeys.putIfAbsent(key, () => GlobalKey()),
      controller: fields[key],
      style: key == 'title'
          ? const TextStyle(fontSize: 18, fontWeight: FontWeight.w600)
          : null,
      enabled: !saving,
      minLines: 1,
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

  Future<void> revealField(String name) async {
    if (['interval', 'rationale', 'steps'].contains(name)) {
      opened.add('preparation');
    }
    if (['followUpScope', 'guidance'].contains(name)) opened.add('followup');
    setState(() {});
    await WidgetsBinding.instance.endOfFrame;
    if (!mounted) return;
    final target = fieldKeys[name]?.currentContext;
    if (target != null && target.mounted) {
      await Scrollable.ensureVisible(target, alignment: .15);
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
      final missing = [
        'followUpScope',
        'guidance',
        'interval',
        'rationale',
      ].firstWhere((name) => optional(name) == null);
      await revealField(missing);
      form.currentState?.validate();
      return;
    }
    if (!form.currentState!.validate()) {
      setState(
        () => error = 'Completa los campos obligatorios antes de guardar.',
      );
      final missing = [
        'title',
        'description',
        if (kind == 'recurring') ...['timezone', 'time'],
      ].where((name) => optional(name) == null);
      if (missing.isNotEmpty) await revealField(missing.first);
      return;
    }
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
      await revealField('interval');
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
      showAppNotice(context, 'Encargo guardado.');
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
    Widget sectionDivider() => Padding(
      padding: const EdgeInsets.only(bottom: 16),
      child: Divider(height: 1, color: ink.withValues(alpha: .10)),
    );
    Widget heading(String title, IconData icon) => Padding(
      padding: const EdgeInsets.only(top: 12, bottom: 20),
      child: Row(
        children: [
          Icon(icon, size: 18, color: ink.withValues(alpha: .7)),
          const SizedBox(width: 10),
          Expanded(
            child: Semantics(
              header: true,
              child: Text(
                title,
                style: body.copyWith(fontWeight: FontWeight.w600),
              ),
            ),
          ),
        ],
      ),
    );
    Widget pair(Widget first, Widget second) => LayoutBuilder(
      builder: (context, size) => size.maxWidth < 480
          ? Column(children: [first, second])
          : Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Expanded(child: first),
                const SizedBox(width: 24),
                Expanded(child: second),
              ],
            ),
    );
    Widget disclosure(
      String id,
      String title,
      IconData icon,
      String summary,
      List<Widget> children,
    ) => Padding(
      padding: const EdgeInsets.only(bottom: 20),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Semantics(
            expanded: opened.contains(id),
            child: TextButton(
              key: ValueKey('assignment-editor-$id'),
              onPressed: saving
                  ? null
                  : () => setState(() {
                      FocusScope.of(context).unfocus();
                      if (!opened.add(id)) opened.remove(id);
                    }),
              style: TextButton.styleFrom(
                foregroundColor: ink,
                overlayColor: Colors.transparent,
                splashFactory: NoSplash.splashFactory,
                padding: EdgeInsets.zero,
                minimumSize: const Size(0, 36),
                tapTargetSize: MaterialTapTargetSize.shrinkWrap,
              ),
              child: Row(
                children: [
                  Icon(icon, size: 18, color: ink.withValues(alpha: .7)),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Text(
                      title,
                      style: body.copyWith(fontWeight: FontWeight.w600),
                    ),
                  ),
                  Icon(
                    opened.contains(id) ? Icons.expand_less : Icons.expand_more,
                    size: 18,
                  ),
                ],
              ),
            ),
          ),
          if (!opened.contains(id) && summary.isNotEmpty)
            Padding(
              padding: const EdgeInsets.only(left: 28, top: 4),
              child: Text(
                summary,
                maxLines: 2,
                overflow: TextOverflow.ellipsis,
                style: body.copyWith(
                  fontSize: 12,
                  color: ink.withValues(alpha: .7),
                ),
              ),
            ),
          Visibility(
            visible: opened.contains(id),
            maintainState: true,
            child: Padding(
              padding: const EdgeInsets.only(top: 16),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: children,
              ),
            ),
          ),
        ],
      ),
    );
    final border = UnderlineInputBorder(
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
          floatingLabelBehavior: FloatingLabelBehavior.always,
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
          child: Column(
            children: [
              Expanded(
                child: SingleChildScrollView(
                  key: const ValueKey('assignment-editor-scroll'),
                  child: Center(
                    child: ConstrainedBox(
                      constraints: const BoxConstraints(maxWidth: 760),
                      child: Padding(
                        padding: const EdgeInsets.all(24),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Row(
                              children: [
                                AssignmentLogo(
                                  document: {
                                    'title': fields['title']!.text,
                                    'applications': applications,
                                  },
                                  size: 56,
                                ),
                                const SizedBox(width: 16),
                                Expanded(
                                  child: Column(
                                    crossAxisAlignment:
                                        CrossAxisAlignment.start,
                                    children: [
                                      Text(
                                        widget.record == null
                                            ? 'Nuevo encargo'
                                            : 'Editar encargo',
                                        style: body.copyWith(
                                          fontSize: 22,
                                          height: 1.25,
                                          fontWeight: FontWeight.w600,
                                        ),
                                      ),
                                      const SizedBox(height: 6),
                                      Text(
                                        'La tarea, sus fuentes y cuándo recibirla.',
                                        style: body.copyWith(
                                          fontSize: 12,
                                          color: ink.withValues(alpha: .7),
                                        ),
                                      ),
                                    ],
                                  ),
                                ),
                              ],
                            ),
                            const SizedBox(height: 32),
                            field('title', 'Título', required: true),
                            field(
                              'description',
                              'Qué debe hacer Gennie',
                              lines: 3,
                              required: true,
                            ),
                            field(
                              'reason',
                              'Para qué lo necesitas (opcional)',
                              lines: 2,
                            ),
                            const SizedBox(height: 12),
                            heading(
                              'Cuándo recibir la información',
                              Icons.schedule_outlined,
                            ),
                            choice(
                              'Tipo',
                              kind,
                              assignmentKinds,
                              (v) => kind = v,
                            ),
                            if (kind == 'recurring') ...[
                              pair(
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
                              ),
                              field(
                                'timezone',
                                'Zona horaria',
                                required: true,
                                hint: 'Ejemplo: America/Bogota',
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
                                          : () =>
                                                setState(() => notifyAt = null),
                                      child: const Text('Sin fecha límite'),
                                    ),
                                ],
                              ),
                              const SizedBox(height: 16),
                            ],

                            const SizedBox(height: 12),
                            sectionDivider(),
                            heading('Aplicaciones', Icons.apps_outlined),
                            if (applications.isEmpty)
                              Text(
                                'Ninguna aplicación añadida',
                                style: body.copyWith(
                                  fontSize: 12,
                                  color: ink.withValues(alpha: .7),
                                ),
                              ),
                            for (var i = 0; i < applications.length; i++) ...[
                              Row(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: [
                                  Padding(
                                    padding: const EdgeInsets.only(top: 12),
                                    child: AssignmentLogo(
                                      document: {
                                        'applications': [applications[i]],
                                      },
                                      size: 32,
                                    ),
                                  ),
                                  const SizedBox(width: 12),
                                  Expanded(
                                    child: TextFormField(
                                      key: ValueKey(
                                        'name-${identityHashCode(applications[i])}',
                                      ),
                                      initialValue:
                                          applications[i]['name'] as String? ??
                                          '',
                                      enabled: !saving,
                                      decoration: const InputDecoration(
                                        labelText: 'Nombre de la aplicación',
                                      ),
                                      onChanged: (v) =>
                                          applications[i]['name'] = v,
                                    ),
                                  ),
                                  AppTooltip(
                                    label: 'Quitar aplicación',
                                    child: Semantics(
                                      label: 'Quitar aplicación',
                                      child: IconButton(
                                        onPressed: saving
                                            ? null
                                            : () => setState(
                                                () => applications.removeAt(i),
                                              ),
                                        icon: const Icon(Icons.close, size: 16),
                                      ),
                                    ),
                                  ),
                                ],
                              ),
                              const SizedBox(height: 12),
                              disclosure(
                                'source-${identityHashCode(applications[i])}',
                                'Origen de la aplicación',
                                Icons.link_outlined,
                                applications[i]['kind'] == 'desktop'
                                    ? 'En el ordenador'
                                    : applications[i]['url'] as String? ??
                                          'Web',
                                [
                                  choice(
                                    'Dónde se utiliza',
                                    applications[i]['kind'] as String? ?? 'web',
                                    const {
                                      'web': 'Web',
                                      'desktop': 'En el ordenador',
                                    },
                                    (v) {
                                      applications[i]['kind'] = v;
                                      if (v == 'desktop') {
                                        applications[i]['url'] = null;
                                      }
                                    },
                                  ),
                                  if (applications[i]['kind'] == 'web')
                                    TextFormField(
                                      key: ValueKey(
                                        'url-${identityHashCode(applications[i])}',
                                      ),
                                      initialValue:
                                          applications[i]['url'] as String? ??
                                          '',
                                      enabled: !saving,
                                      decoration: const InputDecoration(
                                        labelText: 'Dirección web (opcional)',
                                      ),
                                      onChanged: (v) => applications[i]['url'] =
                                          v.trim().isEmpty ? null : v.trim(),
                                    ),
                                ],
                              ),
                            ],
                            TextButton.icon(
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
                              style: TextButton.styleFrom(
                                padding: EdgeInsets.zero,
                                overlayColor: Colors.transparent,
                              ),
                              icon: const Icon(Icons.add, size: 16),
                              label: const Text('Añadir aplicación'),
                            ),
                            const SizedBox(height: 20),
                            sectionDivider(),
                            disclosure(
                              'context',
                              'Contexto',
                              Icons.notes_outlined,
                              optional('context') ??
                                  'Información adicional para interpretar la tarea.',
                              [
                                field(
                                  'context',
                                  'Contexto (opcional)',
                                  lines: 3,
                                ),
                              ],
                            ),
                            sectionDivider(),
                            disclosure(
                              'preparation',
                              'Preparación interna',
                              Icons.checklist_outlined,
                              optional('interval') == null
                                  ? 'Cadencia y pasos para reunir la información.'
                                  : 'Cada ${optional('interval')} minutos',
                              [
                                Padding(
                                  padding: const EdgeInsets.only(bottom: 16),
                                  child: Text(
                                    'Reunir información no cambia la hora del aviso.',
                                    style: body.copyWith(
                                      fontSize: 12,
                                      color: ink.withValues(alpha: .7),
                                    ),
                                  ),
                                ),
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
                                field(
                                  'steps',
                                  'Pasos de preparación (uno por línea)',
                                  lines: 4,
                                ),
                              ],
                            ),
                            sectionDivider(),
                            Row(
                              children: [
                                const Icon(Icons.visibility_outlined, size: 18),
                                const SizedBox(width: 10),
                                Expanded(
                                  child: Text(
                                    'Seguimiento',
                                    style: body.copyWith(
                                      fontWeight: FontWeight.w600,
                                    ),
                                  ),
                                ),
                              ],
                            ),
                            SwitchListTile.adaptive(
                              key: const ValueKey('follow-up-policy'),
                              contentPadding: EdgeInsets.zero,
                              title: const Text(
                                'Seguimiento entre entregas',
                                style: TextStyle(fontSize: 13),
                              ),
                              subtitle: const Text(
                                'Valorar cambios además de la entrega programada.',
                                style: TextStyle(fontSize: 11),
                              ),
                              value: followUpEnabled,
                              onChanged: saving
                                  ? null
                                  : (value) => setState(() {
                                      followUpEnabled = value;
                                      if (value) {
                                        opened.add('followup');
                                        if (optional('interval') == null) {
                                          fields['interval']!.text = '10';
                                        }
                                        if (optional('rationale') == null) {
                                          fields['rationale']!.text =
                                              'Detectar cambios y valorar si conviene intervenir entre entregas.';
                                        }
                                        if (optional('guidance') == null) {
                                          fields['guidance']!.text =
                                              'Valora los cambios, el contexto y el historial. Decide si basta con registrar, si conviene preguntar, avisar o proponer ayuda, o si corresponde una acción delegada. Conserva los cambios aunque no justifiquen interrumpir; evita repetir preguntas o avisos sin novedades relevantes.';
                                        }
                                      }
                                    }),
                            ),
                            if (followUpEnabled)
                              disclosure(
                                'followup',
                                'Criterios de seguimiento',
                                Icons.tune_outlined,
                                optional('followUpScope') ??
                                    'Define qué observar y cuándo intervenir.',
                                [
                                  field(
                                    'followUpScope',
                                    'Qué observar',
                                    lines: 3,
                                    required: true,
                                  ),
                                  field(
                                    'guidance',
                                    'Cómo decidir si intervenir',
                                    lines: 4,
                                    required: true,
                                    hint:
                                        'Cuándo guardar silencio, preguntar, avisar o proponer ayuda.',
                                  ),
                                  disclosure(
                                    'actions',
                                    'Aspectos y acciones',
                                    Icons.rule_outlined,
                                    'Preguntas específicas y límites de las acciones delegadas.',
                                    [
                                      field(
                                        'signals',
                                        'Aspectos a valorar (uno por línea)',
                                        lines: 3,
                                      ),
                                      field(
                                        'actionScope',
                                        'Acciones delegadas (opcional)',
                                        lines: 3,
                                        hint:
                                            'Solo acciones que hayas encargado expresamente, con sus límites.',
                                      ),
                                    ],
                                  ),
                                ],
                              ),
                            const SizedBox(height: 16),
                            sectionDivider(),
                            disclosure(
                              'settings',
                              'Estado e importancia',
                              Icons.flag_outlined,
                              '${assignmentStatuses[status]} · Importancia ${assignmentPriorities[priority]?.toLowerCase()}',
                              [
                                pair(
                                  choice(
                                    'Estado del encargo',
                                    status,
                                    assignmentStatuses,
                                    (v) => status = v,
                                  ),
                                  choice(
                                    'Importancia',
                                    priority,
                                    assignmentPriorities,
                                    (v) => priority = v,
                                  ),
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
              Container(
                key: const ValueKey('assignment-editor-footer'),
                decoration: BoxDecoration(
                  border: Border(
                    top: BorderSide(color: ink.withValues(alpha: .1)),
                  ),
                ),
                child: Center(
                  child: ConstrainedBox(
                    constraints: const BoxConstraints(maxWidth: 760),
                    child: Padding(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 24,
                        vertical: 12,
                      ),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [
                          if (error != null) ...[
                            ConstrainedBox(
                              constraints: const BoxConstraints(maxHeight: 80),
                              child: SingleChildScrollView(
                                child: SelectableText(
                                  error!,
                                  style: body.copyWith(
                                    color: theme.colorScheme.error,
                                  ),
                                ),
                              ),
                            ),
                            if (widget.record != null)
                              Align(
                                alignment: Alignment.centerLeft,
                                child: TextButton(
                                  onPressed: saving ? null : widget.onReload,
                                  child: const Text(
                                    'Descartar borrador y recargar',
                                  ),
                                ),
                              ),
                            const SizedBox(height: 8),
                          ],
                          Row(
                            mainAxisAlignment: MainAxisAlignment.end,
                            children: [
                              TextButton(
                                onPressed: saving ? null : widget.onBack,
                                child: const Text('Cancelar'),
                              ),
                              const SizedBox(width: 12),
                              FilledButton(
                                onPressed: saving ? null : save,
                                child: Text(
                                  saving ? 'Guardando…' : 'Guardar encargo',
                                ),
                              ),
                            ],
                          ),
                        ],
                      ),
                    ),
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
