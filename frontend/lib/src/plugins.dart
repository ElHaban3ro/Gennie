import 'dart:async';
import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter/semantics.dart';
import 'assignment_logo.dart';
import 'backend.dart';
import 'glass.dart';
import 'notice.dart';
import 'sidebar.dart';
import 'detail_view.dart';

const _pretty = JsonEncoder.withIndent('  ');
const pluginJobStates = {
  'queued': 'En cola',
  'running': 'Construyendo',
  'waiting_user': 'Necesita tu intervención',
  'completed': 'Completado',
  'blocked': 'Bloqueado',
  'cancelled': 'Cancelado',
  'interrupted': 'Interrumpido',
};
typedef PluginCall =
    Future<Map<String, dynamic>> Function(Map<String, dynamic>);

String _jobName(Map<String, dynamic> job) {
  final host = Uri.tryParse(job['origin'] as String? ?? '')?.host;
  return host == null || host.isEmpty ? 'Plugin en preparación' : host;
}

String? _jobUpdatedAt(Map job) {
  final date = DateTime.tryParse(job['updated_at'] as String? ?? '')?.toLocal();
  if (date == null) return null;
  String two(int value) => value.toString().padLeft(2, '0');
  return 'Actualizado ${two(date.day)}/${two(date.month)}/${date.year} · ${two(date.hour)}:${two(date.minute)}';
}

String? _requestAge(Map job) {
  final created = DateTime.tryParse(job['created_at'] as String? ?? '');
  if (created == null) return null;
  final elapsed = DateTime.now().toUtc().difference(created.toUtc());
  if (elapsed.isNegative) return null;
  if (elapsed.inSeconds < 60) return 'Solicitud: hace menos de 1 min';
  if (elapsed.inMinutes < 60) return 'Solicitud: hace ${elapsed.inMinutes} min';
  if (elapsed.inHours < 24) return 'Solicitud: hace ${elapsed.inHours} h';
  return 'Solicitud: hace ${elapsed.inDays} d';
}

String? _buildDuration(Map job) {
  final seconds = job['spent_seconds'];
  if (seconds is! int ||
      seconds < 0 ||
      job['state'] == 'running' ||
      job['state'] == 'queued') {
    return null;
  }
  final hours = seconds ~/ 3600;
  final minutes = (seconds % 3600) ~/ 60;
  final remainder = seconds % 60;
  final parts = <String>[
    if (hours > 0) '$hours h',
    if (minutes > 0) '$minutes min',
    if (remainder > 0 || (hours == 0 && minutes == 0)) '$remainder s',
  ];
  return 'Tiempo de construcción: ${parts.join(' ')}';
}

class _BuildSpinner extends StatelessWidget {
  const _BuildSpinner();
  @override
  Widget build(BuildContext context) => ExcludeSemantics(
    child: SizedBox(
      width: 12,
      height: 12,
      child: CircularProgressIndicator(
        strokeWidth: 1.5,
        value: MediaQuery.disableAnimationsOf(context) ? .75 : null,
        color: sidebarInk(context).withValues(alpha: .7),
      ),
    ),
  );
}

class _JobStatus extends StatelessWidget {
  const _JobStatus({required this.job});
  final Map job;
  @override
  Widget build(BuildContext context) {
    final ink = sidebarInk(context);
    final running = job['state'] == 'running';
    final age = running ? _requestAge(job) : null;
    return Wrap(
      crossAxisAlignment: WrapCrossAlignment.center,
      spacing: 10,
      runSpacing: 6,
      children: [
        Container(
          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
          decoration: BoxDecoration(
            color: ink.withValues(alpha: .07),
            borderRadius: BorderRadius.circular(6),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              if (running) ...[const _BuildSpinner(), const SizedBox(width: 7)],
              Text(
                pluginJobStates[job['state']] ?? '${job['state']}',
                style: TextStyle(
                  fontFamily: 'Inter',
                  fontSize: 11,
                  height: 1.6,
                  color: ink,
                ),
              ),
            ],
          ),
        ),
        if (age != null)
          Text(
            age,
            style: TextStyle(
              fontFamily: 'Inter',
              fontSize: 11,
              color: ink.withValues(alpha: .7),
            ),
          ),
        if (_buildDuration(job) case final duration?)
          Text(
            duration,
            style: TextStyle(
              fontFamily: 'Inter',
              fontSize: 11,
              color: ink.withValues(alpha: .7),
            ),
          ),
      ],
    );
  }
}

class PluginsView extends StatefulWidget {
  const PluginsView({
    super.key,
    required this.state,
    this.sidebarWidth = 260,
    this.initialSelection,
    this.onSelectionChanged,
  });
  final DesktopState state;
  final double sidebarWidth;
  final String? initialSelection;
  final ValueChanged<String?>? onSelectionChanged;
  @override
  State<PluginsView> createState() => _PluginsViewState();
}

class _PluginsViewState extends State<PluginsView> {
  final search = TextEditingController();
  List<Map<String, dynamic>> records = [], jobs = [];
  String? selected, error, deliveryError;
  bool loading = true,
      fetching = false,
      editing = false,
      creating = false,
      updating = false;
  Timer? timer;
  final Map<int, String> retryKeys = {};
  bool rebuilding = false;
  @override
  void initState() {
    super.initState();
    selected = widget.initialSelection;
    widget.state.addListener(_changed);
    unawaited(refresh());
  }

  void _changed() {
    if (!fetching) unawaited(refresh(quiet: true));
  }

  @override
  void dispose() {
    timer?.cancel();
    widget.state.removeListener(_changed);
    search.dispose();
    super.dispose();
  }

  Future<Map<String, dynamic>> call(Map<String, dynamic> request) async {
    final backend = widget.state.backend;
    if (backend is! PluginBackend) {
      if (request['action'] == 'list') {
        return {'plugins': <dynamic>[], 'jobs': <dynamic>[]};
      }
      throw StateError('Backend de plugins no disponible');
    }
    final result = await (backend as PluginBackend).pluginRequest(
      jsonEncode(request),
    );
    return Map<String, dynamic>.from(jsonDecode(result) as Map);
  }

  Future<void> refresh({bool quiet = false}) async {
    if (fetching) return;
    fetching = true;
    try {
      final value = await call({'action': 'list'});
      if (!mounted) return;
      setState(() {
        records = (value['plugins'] as List? ?? [])
            .map((e) => Map<String, dynamic>.from(e as Map))
            .toList();
        jobs = (value['jobs'] as List? ?? [])
            .map((e) => Map<String, dynamic>.from(e as Map))
            .toList();
        deliveryError = value['delivery_error'] as String?;
        error = null;
      });
    } catch (e) {
      if (mounted && !quiet) setState(() => error = '$e');
    } finally {
      fetching = false;
      if (mounted) {
        setState(() => loading = false);
        timer?.cancel();
        if (jobs.any((j) => ['queued', 'running'].contains(j['state']))) {
          timer = Timer(const Duration(seconds: 2), () => refresh(quiet: true));
        }
      }
    }
  }

  Future<void> jobAction(String action, Map<String, dynamic> job) async {
    try {
      await call({'action': action, 'job_id': job['id']});
      await refresh();
      if (mounted &&
          action == 'cancel' &&
          selected == job['plugin_id'] &&
          !records.any((r) => (r['manifest'] as Map)['id'] == selected) &&
          !jobs.any(
            (j) => j['plugin_id'] == selected && j['state'] != 'cancelled',
          )) {
        setState(() => selected = null);
        widget.onSelectionChanged?.call(null);
      }
    } catch (e) {
      if (mounted) setState(() => error = '$e');
    }
  }

  Future<void> rebuild(Map<String, dynamic> job) async {
    if (rebuilding) return;
    final id = job['id'] as int;
    final key = retryKeys.putIfAbsent(
      id,
      () => 'plugin-rebuild-$id-${DateTime.now().microsecondsSinceEpoch}',
    );
    setState(() {
      rebuilding = true;
      error = null;
    });
    try {
      await call({
        'action': 'build',
        'trigger_conversation_id': job['trigger_conversation_id'],
        'request': {
          'request_key': key,
          'retry_of': id,
          'plugin_id': job['plugin_id'],
          'origin': job['origin'],
          'objective': job['objective'],
          'max_attempts': job['max_attempts'],
          'max_seconds': job['max_seconds'],
        },
      });
      retryKeys.remove(id);
      await refresh();
    } catch (e) {
      if (mounted) setState(() => error = '$e');
    } finally {
      if (mounted) setState(() => rebuilding = false);
    }
  }

  Future<void> remove(Map<String, dynamic> record) async {
    final manifest = record['manifest'] as Map;
    final confirm = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Eliminar plugin'),
        content: Text(
          '¿Eliminar ${manifest['name']}? Se cancelarán sus trabajos pendientes. La sesión del navegador se conserva.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: const Text('Cancelar'),
          ),
          TextButton(
            onPressed: () => Navigator.pop(context, true),
            child: const Text('Eliminar'),
          ),
        ],
      ),
    );
    if (confirm != true) return;
    try {
      final response = await call({
        'action': 'delete',
        'plugin_id': manifest['id'],
        'expected_revision': record['revision'],
      });
      if (response['status'] != 'deleted') {
        throw StateError('El plugin cambió. Recarga antes de eliminarlo.');
      }
      if (!mounted) return;
      setState(() {
        if (selected == manifest['id']) {
          selected = null;
          widget.onSelectionChanged?.call(null);
          editing = false;
          updating = false;
        }
      });
      await refresh();
    } catch (e) {
      if (mounted) setState(() => error = '$e');
    }
  }

  Future<void> _menu(Map<String, dynamic> record, Offset position) async {
    final overlay =
        Overlay.of(context).context.findRenderObject()! as RenderBox;
    final action = await showMenu<String>(
      context: context,
      position: RelativeRect.fromRect(
        position & const Size(1, 1),
        Offset.zero & overlay.size,
      ),
      items: const [
        PopupMenuItem(value: 'delete', child: Text('Eliminar plugin')),
      ],
    );
    if (mounted && action == 'delete') await remove(record);
  }

  @override
  Widget build(BuildContext context) {
    final query = search.text.trim().toLowerCase();
    final entries = <String, Map<String, dynamic>>{};
    final latestJobs = <String, Map<String, dynamic>>{};
    for (final job in jobs) {
      final id = job['plugin_id'] as String;
      final previous = latestJobs[id];
      if (previous == null || (job['id'] as int) > (previous['id'] as int)) {
        latestJobs[id] = job;
      }
    }
    for (final job in latestJobs.values) {
      if (job['state'] == 'cancelled') continue;
      final id = job['plugin_id'] as String;
      entries.putIfAbsent(
        id,
        () => {'id': id, 'name': _jobName(job), 'job': job},
      );
    }
    for (final record in records) {
      final manifest = record['manifest'] as Map;
      final id = manifest['id'] as String;
      entries[id] = {
        'id': id,
        'name': manifest['name'],
        'record': record,
        if (latestJobs[id] != null) 'job': latestJobs[id],
      };
    }
    final filtered = entries.values
        .where(
          (entry) =>
              '${entry['name']} ${entry['id']} ${entry['job']?['origin'] ?? ''} ${jsonEncode(entry['record']?['manifest'])}'
                  .toLowerCase()
                  .contains(query),
        )
        .toList();
    final chosen = records.where(
      (r) => (r['manifest'] as Map)['id'] == selected,
    );
    final record = chosen.isEmpty ? null : chosen.first;
    final selectedName = selected == null
        ? null
        : entries[selected]?['name'] as String?;
    final latest = selected == null ? null : latestJobs[selected];
    final associated = latest == null ? <Map<String, dynamic>>[] : [latest];
    return Row(
      children: [
        AppSidebar(
          width: widget.sidebarWidth,
          title: 'Plugins',
          actionLabel: 'Nuevo plugin',
          onAction: () => setState(() {
            creating = true;
            editing = false;
            updating = false;
            selected = null;
            widget.onSelectionChanged?.call(null);
          }),
          searchController: search,
          searchHint: 'Buscar plugins…',
          searchKey: const ValueKey('plugins-search'),
          onSearch: (_) => setState(() {}),
          child: loading
              ? const Center(child: CircularProgressIndicator(strokeWidth: 2))
              : filtered.isEmpty
              ? SidebarEmpty(
                  message: query.isEmpty
                      ? 'No hay contenido disponible'
                      : 'No se encontraron plugins',
                )
              : ListView(
                  padding: const EdgeInsets.symmetric(horizontal: 8),
                  children: [
                    for (final entry in filtered)
                      Padding(
                        padding: const EdgeInsets.only(bottom: 2),
                        child: Builder(
                          builder: (tileContext) {
                            final itemRecord =
                                entry['record'] as Map<String, dynamic>?;
                            final origin =
                                (itemRecord?['manifest'] as Map?)?['origin'] ??
                                (entry['job'] as Map?)?['origin'] ??
                                '';
                            void openMenu() {
                              if (itemRecord == null) return;
                              final box =
                                  tileContext.findRenderObject()! as RenderBox;
                              _menu(
                                itemRecord,
                                box.localToGlobal(box.size.center(Offset.zero)),
                              );
                            }

                            return CallbackShortcuts(
                              bindings: {
                                const SingleActivator(
                                  LogicalKeyboardKey.f10,
                                  shift: true,
                                ): openMenu,
                                const SingleActivator(
                                  LogicalKeyboardKey.contextMenu,
                                ): openMenu,
                              },
                              child: Semantics(
                                customSemanticsActions: itemRecord == null
                                    ? null
                                    : {
                                        const CustomSemanticsAction(
                                          label: 'Eliminar plugin',
                                        ): () =>
                                            remove(itemRecord),
                                      },
                                child: GestureDetector(
                                  onSecondaryTapUp: itemRecord == null
                                      ? null
                                      : (details) => _menu(
                                          itemRecord,
                                          details.globalPosition,
                                        ),
                                  child: SidebarTile(
                                    trailing:
                                        (entry['job'] as Map?)?['state'] ==
                                            'running'
                                        ? const _BuildSpinner()
                                        : null,
                                    leading: AssignmentLogo(
                                      size: 38,
                                      document: {
                                        'applications': [
                                          {
                                            'name': entry['name'],
                                            'kind': 'web',
                                            'url': origin,
                                          },
                                        ],
                                      },
                                    ),
                                    title: entry['name'] as String,
                                    subtitle: entry['job'] != null
                                        ? pluginJobStates[(entry['job']
                                                  as Map)['state']] ??
                                              'En preparación'
                                        : '${(((entry['record'] as Map)['manifest'] as Map)['functions'] as List).length} funciones · revisión ${(entry['record'] as Map)['revision']}',
                                    selected: entry['id'] == selected,
                                    onSelect: () => setState(() {
                                      selected = entry['id'] as String;
                                      widget.onSelectionChanged?.call(selected);
                                      creating = false;
                                      editing = false;
                                      updating = false;
                                    }),
                                  ),
                                ),
                              ),
                            );
                          },
                        ),
                      ),
                  ],
                ),
        ),
        Expanded(
          child: AppDetailView(
            title: creating ? 'Nuevo plugin' : selectedName ?? 'Plugins',
            actions: [
              AppTooltip(
                label: 'Recargar plugins',
                child: IconButton(
                  onPressed: () => refresh(),
                  icon: const Icon(Icons.refresh, size: 18),
                ),
              ),
            ],
            notice: error != null || deliveryError != null
                ? Padding(
                    padding: const EdgeInsets.all(12),
                    child: Text(
                      error ?? deliveryError!,
                      style: TextStyle(
                        color: Theme.of(context).colorScheme.error,
                        fontSize: 12,
                      ),
                    ),
                  )
                : null,
            child: creating || updating
                ? PluginBuildForm(
                    key: ValueKey(
                      'build-${record?['revision']}-${selected ?? 'new'}',
                    ),
                    record: updating ? record : null,
                    conversation: widget.state.snapshot?.selectedId,
                    call: call,
                    onCancel: () => setState(() {
                      creating = false;
                      updating = false;
                    }),
                    onCreated: () async {
                      setState(() {
                        creating = false;
                        updating = false;
                      });
                      await refresh();
                    },
                  )
                : editing && record != null
                ? PluginEditor(
                    key: ValueKey('edit-$selected-${record['revision']}'),
                    record: record,
                    call: call,
                    onCancel: () => setState(() => editing = false),
                    onSaved: () async {
                      setState(() => editing = false);
                      await refresh();
                    },
                  )
                : record == null && associated.isEmpty
                ? const ContentEmpty(title: 'Plugins')
                : AppViewScroll(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        if (record != null)
                          PluginProfile(
                            key: ValueKey('$selected-${record['revision']}'),
                            record: record,
                            job: associated.isNotEmpty
                                ? associated.first
                                : null,
                            call: call,
                            onEdit: () => setState(() => editing = true),
                            onDelete: () => remove(record),
                            onUpdate: () => setState(() => updating = true),
                          ),
                        if (record == null &&
                            selected != null &&
                            associated.isNotEmpty) ...[
                          Builder(
                            builder: (context) {
                              final job = associated.first;
                              final ink = sidebarInk(context);
                              final body = TextStyle(
                                fontFamily: 'Inter',
                                fontSize: 13,
                                height: 1.6,
                                color: ink,
                              );
                              return Column(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: [
                                  AppDetailHeading(
                                    icon: AssignmentLogo(
                                      size: 72,
                                      document: {
                                        'applications': [
                                          {
                                            'name':
                                                selectedName ?? _jobName(job),
                                            'kind': 'web',
                                            'url': job['origin'] ?? '',
                                          },
                                        ],
                                      },
                                    ),
                                    label: 'Plugin',
                                    title: selectedName ?? _jobName(job),
                                    subtitle: SelectableText(
                                      job['origin'] as String? ?? '',
                                    ),
                                  ),
                                  const SizedBox(height: 16),
                                  _JobStatus(job: job),
                                  if ((job['objective'] as String? ?? '')
                                      .trim()
                                      .isNotEmpty) ...[
                                    const SizedBox(height: 24),
                                    Text(
                                      'Para qué',
                                      style: body.copyWith(
                                        fontSize: 12,
                                        fontWeight: FontWeight.w600,
                                      ),
                                    ),
                                    const SizedBox(height: 4),
                                    SelectableText(
                                      job['objective'] as String,
                                      style: body,
                                    ),
                                  ],
                                  const SizedBox(height: 24),
                                  Text(
                                    (job['events'] as List? ?? []).isNotEmpty &&
                                            ((job['events'] as List).last
                                                    as Map)['message'] ==
                                                job['summary']
                                        ? 'Resultado del último paso'
                                        : 'Resultado de la construcción',
                                    style: body.copyWith(
                                      fontSize: 12,
                                      fontWeight: FontWeight.w600,
                                    ),
                                  ),
                                  const SizedBox(height: 4),
                                  SelectableText(
                                    job['summary'] as String,
                                    style: body,
                                  ),
                                  if (job['state'] == 'blocked' &&
                                      !jobs.any(
                                        (j) =>
                                            j['plugin_id'] ==
                                                job['plugin_id'] &&
                                            [
                                              'queued',
                                              'running',
                                              'waiting_user',
                                              'interrupted',
                                            ].contains(j['state']),
                                      )) ...[
                                    const SizedBox(height: 8),
                                    Align(
                                      alignment: Alignment.centerLeft,
                                      child: TextButton(
                                        onPressed: rebuilding
                                            ? null
                                            : () => rebuild(job),
                                        child: const Text('Volver a generar'),
                                      ),
                                    ),
                                  ],
                                ],
                              );
                            },
                          ),
                        ],
                        if (associated.isNotEmpty) ...[
                          const SizedBox(height: 24),
                          Row(
                            children: [
                              Icon(
                                Icons.build_outlined,
                                size: 18,
                                color: sidebarInk(
                                  context,
                                ).withValues(alpha: .7),
                              ),
                              const SizedBox(width: 10),
                              const Text(
                                'Construcción de plugins',
                                style: TextStyle(
                                  fontSize: 13,
                                  fontWeight: FontWeight.w600,
                                ),
                              ),
                            ],
                          ),
                          const SizedBox(height: 12),
                          for (final (jobIndex, job) in associated.indexed)
                            Padding(
                              padding: const EdgeInsets.only(bottom: 16),
                              child: Column(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: [
                                  if (jobIndex > 0) ...[
                                    Divider(
                                      height: 24,
                                      color: sidebarInk(
                                        context,
                                      ).withValues(alpha: .1),
                                    ),
                                  ],
                                  Text(
                                    record == null && jobIndex == 0
                                        ? 'Trabajo actual'
                                        : '${(entries[job['plugin_id']]?['name'] as String?) ?? _jobName(job)} · ${pluginJobStates[job['state']] ?? job['state']}',
                                    style: const TextStyle(
                                      fontSize: 13,
                                      fontWeight: FontWeight.w600,
                                    ),
                                  ),
                                  const SizedBox(height: 6),
                                  if (!(record == null &&
                                      selected != null &&
                                      identical(job, associated.first)))
                                    SelectableText(
                                      job['summary'] as String,
                                      style: const TextStyle(
                                        fontFamily: 'Inter',
                                        fontSize: 13,
                                        height: 1.6,
                                      ),
                                    ),
                                  Text(
                                    'Chat ${job['trigger_conversation_id']}',
                                    style: TextStyle(
                                      fontSize: 11,
                                      color: sidebarInk(
                                        context,
                                      ).withValues(alpha: .7),
                                    ),
                                  ),
                                  if (_jobUpdatedAt(job) case final updated?)
                                    Text(
                                      updated,
                                      style: TextStyle(
                                        fontSize: 11,
                                        color: sidebarInk(
                                          context,
                                        ).withValues(alpha: .7),
                                      ),
                                    ),
                                  if ((job['events'] as List? ?? [])
                                          .isNotEmpty &&
                                      (record != null ||
                                          ((job['events'] as List).last
                                                  as Map)['message'] !=
                                              job['summary'])) ...[
                                    const SizedBox(height: 16),
                                    const Text(
                                      'Último paso',
                                      style: TextStyle(
                                        fontSize: 12,
                                        fontWeight: FontWeight.w600,
                                      ),
                                    ),
                                    const SizedBox(height: 4),
                                    SelectableText(
                                      ((job['events'] as List).last
                                              as Map)['message']
                                          as String,
                                      style: const TextStyle(
                                        fontFamily: 'Inter',
                                        fontSize: 13,
                                        height: 1.6,
                                      ),
                                    ),
                                  ],
                                  if ((job['events'] as List? ?? []).isNotEmpty)
                                    ExpansionTile(
                                      shape: const Border(),
                                      collapsedShape: const Border(),
                                      tilePadding: EdgeInsets.zero,
                                      iconColor: sidebarInk(context),
                                      collapsedIconColor: sidebarInk(context),
                                      title: const Text(
                                        'Pasos de investigación',
                                        style: TextStyle(
                                          fontSize: 13,
                                          fontWeight: FontWeight.w500,
                                        ),
                                      ),
                                      children: [
                                        for (final (index, event)
                                            in (job['events'] as List).indexed)
                                          Padding(
                                            padding: const EdgeInsets.only(
                                              bottom: 10,
                                            ),
                                            child: Align(
                                              alignment: Alignment.centerLeft,
                                              child: SelectableText(
                                                '${index + 1}. ${event['message']}\n${event['created_at']}',
                                                style: const TextStyle(
                                                  fontSize: 12,
                                                ),
                                              ),
                                            ),
                                          ),
                                      ],
                                    ),
                                  if (job['state'] == 'waiting_user')
                                    const Padding(
                                      padding: EdgeInsets.only(bottom: 4),
                                      child: Text(
                                        'Abre el sitio y completa la acción solicitada. Después, reanuda la construcción.',
                                        style: TextStyle(fontSize: 12),
                                      ),
                                    ),
                                  Wrap(
                                    spacing: 8,
                                    children: [
                                      if ((job['state'] == 'cancelled' ||
                                              (job['state'] == 'blocked' &&
                                                  record != null)) &&
                                          !jobs.any(
                                            (j) =>
                                                j['plugin_id'] ==
                                                    job['plugin_id'] &&
                                                [
                                                  'queued',
                                                  'running',
                                                  'waiting_user',
                                                  'interrupted',
                                                ].contains(j['state']),
                                          ))
                                        TextButton(
                                          onPressed: rebuilding
                                              ? null
                                              : () => rebuild(job),
                                          child: const Text('Volver a generar'),
                                        ),
                                      if (job['state'] == 'waiting_user')
                                        TextButton(
                                          onPressed: () =>
                                              jobAction('connect', job),
                                          child: const Text(
                                            'Abrir sitio en Gennie Web',
                                          ),
                                        ),
                                      if ([
                                        'waiting_user',
                                        'interrupted',
                                      ].contains(job['state']))
                                        TextButton(
                                          onPressed: () =>
                                              jobAction('resume', job),
                                          child: const Text(
                                            'Reanudar construcción',
                                          ),
                                        ),
                                      if ([
                                        'queued',
                                        'running',
                                        'waiting_user',
                                        'interrupted',
                                      ].contains(job['state']))
                                        TextButton(
                                          onPressed: () =>
                                              jobAction('cancel', job),
                                          child: const Text('Cancelar trabajo'),
                                        ),
                                    ],
                                  ),
                                ],
                              ),
                            ),
                        ],
                      ],
                    ),
                  ),
          ),
        ),
      ],
    );
  }
}

class PluginProfile extends StatefulWidget {
  const PluginProfile({
    super.key,
    required this.record,
    required this.call,
    required this.onEdit,
    required this.onDelete,
    required this.onUpdate,
    this.job,
  });
  final Map<String, dynamic> record;
  final Map<String, dynamic>? job;
  final PluginCall call;
  final VoidCallback onEdit, onDelete, onUpdate;
  @override
  State<PluginProfile> createState() => _PluginProfileState();
}

class _PluginProfileState extends State<PluginProfile> {
  int index = 0;
  @override
  Widget build(BuildContext context) {
    final m = widget.record['manifest'] as Map;
    final ink = sidebarInk(context);
    final body = TextStyle(
      fontFamily: 'Inter',
      fontSize: 13,
      height: 1.6,
      color: ink,
    );
    final functions = m['functions'] as List;
    final fn = functions[index] as Map;
    Widget section(String title, IconData icon, List<dynamic> values) =>
        Padding(
          padding: const EdgeInsets.only(top: 20),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  Icon(icon, size: 18, color: ink.withValues(alpha: .7)),
                  const SizedBox(width: 10),
                  Text(
                    title,
                    style: body.copyWith(fontWeight: FontWeight.w600),
                  ),
                ],
              ),
              const SizedBox(height: 12),
              for (final value in values)
                Padding(
                  padding: const EdgeInsets.only(bottom: 4),
                  child: SelectableText('• $value', style: body),
                ),
            ],
          ),
        );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        AppDetailHeading(
          icon: AssignmentLogo(
            size: 72,
            document: {
              'applications': [
                {'name': m['name'], 'kind': 'web', 'url': m['origin']},
              ],
            },
          ),
          label: 'Plugin',
          title: m['name'] as String,
          subtitle: SelectableText(m['origin'] as String),
          onEdit: widget.onEdit,
          editLabel: 'Editar plugin',
          editKey: const ValueKey('plugin-edit'),
        ),
        if (widget.job != null) ...[
          const SizedBox(height: 16),
          _JobStatus(job: widget.job!),
        ],
        const SizedBox(height: 20),
        SelectableText(
          m['description'] as String? ?? 'Sin descripción de cobertura.',
          style: body,
        ),
        section(
          'Capacidades',
          Icons.check_circle_outline,
          (m['capabilities'] as List?) ??
              functions.map((f) => f['description']).toList(),
        ),
        section(
          'Limitaciones',
          Icons.info_outline,
          (m['limitations'] as List?) ??
              ['No se han documentado limitaciones específicas.'],
        ),
        Wrap(
          spacing: 8,
          children: [
            TextButton(
              onPressed: widget.onUpdate,
              child: const Text('Actualizar con el asistente'),
            ),
            TextButton(
              onPressed: widget.onDelete,
              child: const Text('Eliminar'),
            ),
          ],
        ),
        const Divider(height: 32),
        const Text(
          'Probar funciones',
          style: TextStyle(fontSize: 16, fontWeight: FontWeight.w600),
        ),
        const SizedBox(height: 12),
        DropdownButtonFormField<int>(
          initialValue: index,
          isExpanded: true,
          decoration: const InputDecoration(labelText: 'Función'),
          items: [
            for (var i = 0; i < functions.length; i++)
              DropdownMenuItem(
                value: i,
                child: Text(
                  functions[i]['name'] as String,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
          ],
          onChanged: (value) {
            if (value != null) setState(() => index = value);
          },
        ),
        const SizedBox(height: 14),
        FunctionProbe(
          key: ValueKey(
            '${m['id']}-${widget.record['revision']}-${fn['name']}',
          ),
          record: widget.record,
          function: Map<String, dynamic>.from(fn),
          call: widget.call,
        ),
      ],
    );
  }
}

class FunctionProbe extends StatefulWidget {
  const FunctionProbe({
    super.key,
    required this.record,
    required this.function,
    required this.call,
  });
  final Map<String, dynamic> record, function;
  final PluginCall call;
  @override
  State<FunctionProbe> createState() => _FunctionProbeState();
}

class _FunctionProbeState extends State<FunctionProbe> {
  final fields = <String, TextEditingController>{};
  Map<String, dynamic>? result;
  String? error;
  bool running = false;
  int? milliseconds;
  @override
  void initState() {
    super.initState();
    for (final key in (widget.function['parameters'] as Map).keys) {
      fields[key as String] = TextEditingController();
    }
  }

  @override
  void dispose() {
    for (final c in fields.values) {
      c.dispose();
    }
    super.dispose();
  }

  Future<void> run() async {
    final arguments = <String, dynamic>{};
    final parameters = widget.function['parameters'] as Map;
    for (final entry in fields.entries) {
      final value = entry.value.text;
      if (value.isEmpty) {
        setState(() => error = 'Completa ${entry.key}.');
        return;
      }
      if (parameters[entry.key] == 'integer') {
        final number = int.tryParse(value);
        if (number == null) {
          setState(() => error = '${entry.key} debe ser un entero.');
          return;
        }
        arguments[entry.key] = number;
      } else {
        arguments[entry.key] = value;
      }
    }
    setState(() {
      running = true;
      error = null;
      result = null;
    });
    final clock = Stopwatch()..start();
    try {
      final verified = widget.record['verified'] as Map? ?? {};
      final response = await widget.call({
        'action': verified.containsKey(widget.function['name'])
            ? 'run'
            : 'verify',
        'plugin_id': (widget.record['manifest'] as Map)['id'],
        'function_name': widget.function['name'],
        'expected_revision': widget.record['revision'],
        'arguments': arguments,
      });
      if (mounted) setState(() => result = response);
    } catch (e) {
      if (mounted) setState(() => error = '$e');
    } finally {
      if (mounted) {
        setState(() {
          running = false;
          milliseconds = clock.elapsedMilliseconds;
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final fn = widget.function;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(fn['description'] as String, style: const TextStyle(fontSize: 13)),
        const SizedBox(height: 6),
        SelectableText(
          '${fn['transport'] == 'http' ? 'GET' : 'Página'} ${fn['path']}',
          style: const TextStyle(fontSize: 12),
        ),
        for (final entry in fields.entries)
          Padding(
            padding: const EdgeInsets.only(top: 12),
            child: TextField(
              key: ValueKey('plugin-param-${entry.key}'),
              controller: entry.value,
              enabled: !running,
              decoration: InputDecoration(
                labelText:
                    '${entry.key} (${(fn['parameters'] as Map)[entry.key]})',
              ),
            ),
          ),
        const SizedBox(height: 16),
        Align(
          alignment: Alignment.centerLeft,
          child: FilledButton(
            onPressed: running ? null : run,
            child: Text(running ? 'Ejecutando…' : 'Ejecutar función'),
          ),
        ),
        if (running)
          const Padding(
            padding: EdgeInsets.only(top: 12),
            child: LinearProgressIndicator(),
          ),
        if (error != null)
          Padding(
            padding: const EdgeInsets.only(top: 12),
            child: SelectableText(
              error!,
              style: TextStyle(
                color: Theme.of(context).colorScheme.error,
                fontSize: 12,
              ),
            ),
          ),
        if (result != null) ...[
          const SizedBox(height: 16),
          Text(
            'Resultado · ${result!['status']} · ${milliseconds ?? 0} ms',
            key: const ValueKey('plugin-result-status'),
            style: const TextStyle(fontSize: 12, fontWeight: FontWeight.w600),
          ),
          const SizedBox(height: 8),
          SelectableText(
            _pretty.convert(result),
            key: const ValueKey('plugin-result'),
            style: const TextStyle(fontSize: 12, height: 1.5),
          ),
          if (result!['status'] == 'authentication_required')
            TextButton(
              onPressed: () async {
                try {
                  await widget.call({
                    'action': 'connect',
                    'plugin_id': (widget.record['manifest'] as Map)['id'],
                  });
                } catch (e) {
                  if (mounted) setState(() => error = '$e');
                }
              },
              child: const Text('Iniciar sesión en Gennie Web'),
            ),
        ],
      ],
    );
  }
}

class PluginEditor extends StatefulWidget {
  const PluginEditor({
    super.key,
    required this.record,
    required this.call,
    required this.onCancel,
    required this.onSaved,
  });
  final Map<String, dynamic> record;
  final PluginCall call;
  final VoidCallback onCancel;
  final Future<void> Function() onSaved;
  @override
  State<PluginEditor> createState() => _PluginEditorState();
}

class _PluginEditorState extends State<PluginEditor> {
  late final TextEditingController text;
  String? error;
  bool saving = false;
  @override
  void initState() {
    super.initState();
    text = TextEditingController(
      text: _pretty.convert(widget.record['manifest']),
    );
  }

  @override
  void dispose() {
    text.dispose();
    super.dispose();
  }

  Future<void> save() async {
    setState(() {
      saving = true;
      error = null;
    });
    try {
      final manifest = jsonDecode(text.text);
      if (manifest is! Map ||
          manifest['id'] != (widget.record['manifest'] as Map)['id']) {
        throw const FormatException('Conserva el identificador del plugin.');
      }
      final result = await widget.call({
        'action': 'save',
        'manifest': manifest,
        'expected_revision': widget.record['revision'],
      });
      if (result['status'] != 'saved') {
        throw StateError(
          'El plugin cambió. Cancela y recarga antes de editar la nueva revisión.',
        );
      }
      if (!mounted) return;
      showAppNotice(context, 'Plugin guardado');
      await widget.onSaved();
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
          child: TextField(
            key: const ValueKey('plugin-editor'),
            controller: text,
            enabled: !saving,
            minLines: 16,
            maxLines: null,
            style: const TextStyle(fontSize: 12, height: 1.5),
            decoration: const InputDecoration(
              labelText: 'Definición del plugin (JSON)',
              helperText:
                  'Edita cobertura, capacidades, limitaciones y funciones. Guardar exige verificar de nuevo las funciones.',
              helperMaxLines: 3,
            ),
          ),
        ),
      ),
      if (error != null)
        Padding(
          padding: const EdgeInsets.all(12),
          child: Text(
            error!,
            style: TextStyle(
              color: Theme.of(context).colorScheme.error,
              fontSize: 12,
            ),
          ),
        ),
      AppViewContent(
        footer: true,
        child: Row(
          children: [
            FilledButton(
              onPressed: saving ? null : save,
              child: Text(saving ? 'Guardando…' : 'Guardar'),
            ),
            const SizedBox(width: 8),
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

class PluginBuildForm extends StatefulWidget {
  const PluginBuildForm({
    super.key,
    this.record,
    required this.conversation,
    required this.call,
    required this.onCancel,
    required this.onCreated,
  });
  final Map<String, dynamic>? record;
  final String? conversation;
  final PluginCall call;
  final VoidCallback onCancel;
  final Future<void> Function() onCreated;
  @override
  State<PluginBuildForm> createState() => _PluginBuildFormState();
}

class _PluginBuildFormState extends State<PluginBuildForm> {
  final origin = TextEditingController(), objective = TextEditingController();
  String? error;
  bool saving = false;
  final requestKey = 'plugin-ui-${DateTime.now().microsecondsSinceEpoch}';
  @override
  void initState() {
    super.initState();
    origin.text =
        (widget.record?['manifest'] as Map?)?['origin'] as String? ?? '';
  }

  @override
  void dispose() {
    origin.dispose();
    objective.dispose();
    super.dispose();
  }

  Future<void> submit() async {
    final uri = Uri.tryParse(origin.text.trim());
    final chat = int.tryParse(widget.conversation ?? '');
    if (uri == null ||
        !['https', 'http'].contains(uri.scheme) ||
        uri.host.isEmpty ||
        uri.userInfo.isNotEmpty ||
        objective.text.trim().isEmpty) {
      setState(
        () => error = 'Indica una web válida y las funciones que necesitas.',
      );
      return;
    }
    if (chat == null) {
      setState(
        () => error =
            'Selecciona primero un chat de origen en Historial para recibir los avances.',
      );
      return;
    }
    final generated =
        'web_${uri.host.toLowerCase().replaceAll(RegExp('[^a-z0-9]'), '_')}';
    final id =
        (widget.record?['manifest'] as Map?)?['id'] as String? ??
        generated.substring(0, generated.length.clamp(0, 64));
    setState(() {
      saving = true;
      error = null;
    });
    try {
      await widget.call({
        'action': 'build',
        'trigger_conversation_id': chat,
        'request': {
          'request_key': requestKey,
          'plugin_id': id,
          'origin': uri.origin,
          'objective': objective.text.trim(),
          'max_attempts': 8,
          'max_seconds': 600,
        },
      });
      if (mounted) await widget.onCreated();
    } catch (e) {
      if (mounted) setState(() => error = '$e');
    } finally {
      if (mounted) setState(() => saving = false);
    }
  }

  @override
  Widget build(BuildContext context) => AppViewScroll(
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          widget.record == null ? 'Crear plugin web' : 'Actualizar plugin',
          style: const TextStyle(fontSize: 22, fontWeight: FontWeight.w600),
        ),
        const SizedBox(height: 12),
        const Text(
          'El constructor trabajará por separado y enviará avances al chat de origen. Te pedirá iniciar sesión solo si hace falta.',
          style: TextStyle(fontSize: 13),
        ),
        const SizedBox(height: 20),
        TextField(
          controller: origin,
          enabled: !saving && widget.record == null,
          decoration: const InputDecoration(
            labelText: 'Web',
            hintText: 'https://sitio.com',
          ),
        ),
        const SizedBox(height: 16),
        TextField(
          controller: objective,
          enabled: !saving,
          minLines: 3,
          maxLines: 8,
          decoration: const InputDecoration(
            labelText: 'Qué funciones necesitas y qué deben cubrir',
          ),
        ),
        if (error != null)
          Padding(
            padding: const EdgeInsets.only(top: 12),
            child: Text(
              error!,
              style: TextStyle(
                color: Theme.of(context).colorScheme.error,
                fontSize: 12,
              ),
            ),
          ),
        const SizedBox(height: 20),
        Wrap(
          spacing: 8,
          children: [
            FilledButton(
              onPressed: saving ? null : submit,
              child: Text(saving ? 'Preparando…' : 'Iniciar construcción'),
            ),
            TextButton(
              onPressed: saving ? null : widget.onCancel,
              child: const Text('Cancelar'),
            ),
          ],
        ),
      ],
    ),
  );
}
