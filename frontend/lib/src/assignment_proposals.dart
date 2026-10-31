import 'review_dialog.dart';
import 'dart:async';
import 'dart:convert';
import 'package:flutter/material.dart';
import 'backend.dart';

Future<Map<String, dynamic>> _request(
  DesktopState state,
  Map<String, dynamic> request,
) async => Map<String, dynamic>.from(
  jsonDecode(
        await (state.backend as AssignmentProposalBackend).proposalRequest(
          jsonEncode(request),
        ),
      )
      as Map,
);

void _reveal(GlobalKey key) {
  WidgetsBinding.instance.addPostFrameCallback((_) {
    final context = key.currentContext;
    if (context != null && context.mounted) {
      unawaited(Scrollable.ensureVisible(context, alignment: .5));
    }
  });
}

class AssignmentProposals extends StatefulWidget {
  const AssignmentProposals({
    super.key,
    required this.state,
    required this.record,
    required this.onOpenChat,
  });
  final DesktopState state;
  final AssignmentRecord record;
  final VoidCallback onOpenChat;
  @override
  State<AssignmentProposals> createState() => _AssignmentProposalsState();
}

class _AssignmentProposalsState extends State<AssignmentProposals> {
  List<Map<String, dynamic>> reviews = [];
  Map<String, dynamic>? nextCursor;
  int? total;
  String? error, observed;
  bool loading = false, inspecting = false, archiving = false, archived = false;
  bool get locked => loading || inspecting || archiving;
  bool loadedMore = false, updatesAvailable = false;
  int generation = 0;
  @override
  void initState() {
    super.initState();
    widget.state.addListener(changed);
    unawaited(load());
  }

  @override
  void didUpdateWidget(AssignmentProposals old) {
    super.didUpdateWidget(old);
    if (old.record.id != widget.record.id ||
        old.record.instanceId != widget.record.instanceId ||
        old.record.revision != widget.record.revision) {
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
    final revision =
        '${widget.state.snapshot?.historyRevision}:${widget.state.snapshot?.executionRevision}';
    if (observed != revision) {
      observed = revision;
      if (loadedMore || archiving || inspecting) {
        setState(() => updatesAvailable = true);
        return;
      }
      unawaited(load());
    }
  }

  Future<void> load({bool more = false}) async {
    if (more && (loading || nextCursor == null)) return;
    final request = ++generation;
    final cursor = more ? nextCursor : null;
    setState(() {
      loading = true;
      error = null;
    });
    try {
      final result = await _request(widget.state, {
        'action': 'list',
        'assignment_id': int.parse(widget.record.id),
        'archived': archived,
        'before': ?cursor,
      });
      if (!mounted || request != generation) return;
      setState(() {
        final page = (result['reviews'] as List)
            .map((r) => Map<String, dynamic>.from(r as Map))
            .toList();
        reviews = more
            ? [
                ...reviews,
                ...page.where(
                  (r) => !reviews.any((old) => old['id'] == r['id']),
                ),
              ]
            : page;
        nextCursor = result['next_cursor'] == null
            ? null
            : Map<String, dynamic>.from(result['next_cursor'] as Map);
        total = result['total'] as int?;
        loadedMore = more;
        if (!more) updatesAvailable = false;
        loading = false;
      });
    } catch (e) {
      if (mounted && request == generation) {
        setState(() {
          error = 'No se pudieron cargar las propuestas: $e';
          loading = false;
        });
      }
    }
  }

  Future<void> archive(Map<String, dynamic> review) async {
    if (locked) return;
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (_) => AppReviewDialog(
        title: 'Archivar revisión',
        busy: false,
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: const Text('Conservar revisión'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(context, true),
            child: const Text('Archivar'),
          ),
        ],
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Text(
              'Se archivarán todas las versiones de esta revisión. Los encargos y las decisiones ya aplicadas se conservan.',
            ),
            const SizedBox(height: 12),
            const Text(
              'Podrás consultar sus borradores en Revisiones archivadas. Para continuar tendrás que solicitar una revisión nueva; no se repetirá la consulta a la guía.',
            ),
            const SizedBox(height: 12),
            const Text('Encargos incluidos:'),
            for (final a in review['scope']['assignments'] as List)
              Text('• ${a['document']['title']}'),
          ],
        ),
      ),
    );
    if (!mounted || confirmed != true) return;
    final request = ++generation;
    setState(() {
      loading = true;
      archiving = true;
      error = null;
    });
    try {
      await _request(widget.state, {
        'action': 'archive',
        'review_id': review['id'],
      });
      if (mounted && request == generation) await load();
    } catch (e) {
      if (mounted && request == generation) {
        setState(() {
          loading = false;
          error = 'No se pudo archivar la revisión: $e';
        });
      }
    } finally {
      if (mounted) setState(() => archiving = false);
    }
  }

  void changeArchiveView() {
    setState(() {
      archived = !archived;
      reviews = [];
      nextCursor = null;
      total = null;
      loadedMore = false;
      updatesAvailable = false;
    });
    unawaited(load());
  }

  Future<void> begin() async {
    final sent = await showDialog<bool>(
      context: context,
      barrierDismissible: false,
      builder: (_) => _ReviewDialog(state: widget.state, record: widget.record),
    );
    if (!mounted) return;
    if (sent == true) widget.onOpenChat();
    unawaited(load());
  }

  Future<void> cancel(String id) async {
    try {
      await _request(widget.state, {'action': 'cancel', 'review_id': id});
      if (mounted) await load();
    } catch (e) {
      if (mounted) {
        setState(() => error = 'No se pudo cancelar la revisión: $e');
      }
    }
  }

  Future<void> inspect(
    Map<String, dynamic> review,
    Map<String, dynamic> proposal,
  ) async {
    if (inspecting || archiving) return;
    final identity = widget.record.instanceId;
    final returnFocus = FocusManager.instance.primaryFocus;
    setState(() => inspecting = true);
    try {
      var shownReview = review, shownProposal = proposal;
      var failed = false;
      while (mounted) {
        final next = await showDialog<String>(
          context: context,
          barrierDismissible: false,
          builder: (_) => _ProposalDialog(
            state: widget.state,
            review: shownReview,
            proposal: shownProposal,
            onOpenChat: widget.onOpenChat,
          ),
        );
        if (next == null || !mounted) break;
        setState(() => loading = true);
        try {
          final newer = await _request(widget.state, {
            'action': 'get',
            'review_id': next,
          });
          final matching = (newer['proposals'] as List)
              .where(
                (p) =>
                    p['candidate']['assignment_id'].toString() ==
                    widget.record.id,
              )
              .toList();
          if (!mounted || widget.record.instanceId != identity) return;
          setState(() => loading = false);
          if (matching.isEmpty) {
            failed = true;
            setState(
              () => error =
                  'Gennie todavía no ha preparado una propuesta para este encargo en la versión nueva. Puedes continuar en el chat.',
            );
            break;
          }
          shownReview = newer;
          shownProposal = Map<String, dynamic>.from(matching.first as Map);
        } catch (e) {
          failed = true;
          if (mounted) {
            setState(() {
              loading = false;
              error = 'No se pudo consultar la versión nueva: $e';
            });
          }
          break;
        }
      }
      if (mounted && !failed) await load();
    } finally {
      if (mounted) {
        setState(() => inspecting = false);
        WidgetsBinding.instance.addPostFrameCallback((_) {
          if (mounted &&
              widget.record.instanceId == identity &&
              !locked &&
              returnFocus?.context != null &&
              returnFocus!.canRequestFocus) {
            returnFocus.requestFocus();
          }
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: [
            TextButton(
              onPressed: widget.state.snapshot?.busy == true || locked
                  ? null
                  : begin,
              child: const Text('Proponer mejoras'),
            ),
            TextButton(
              onPressed: locked ? null : changeArchiveView,
              child: Text(
                archived ? 'Revisiones actuales' : 'Revisiones archivadas',
              ),
            ),
            if (reviews.isNotEmpty || error != null)
              TextButton(
                onPressed: locked ? null : load,
                child: const Text('Actualizar propuestas'),
              ),
          ],
        ),
        if (loading) const LinearProgressIndicator(),
        if (updatesAvailable)
          const Text(
            'Puede haber propuestas nuevas. Actualiza para consultarlas; las revisiones cargadas se conservan.',
          ),
        if (error != null)
          Text(
            error!,
            style: TextStyle(color: Theme.of(context).colorScheme.error),
          ),
        if (!loading && error == null && reviews.isEmpty && archived)
          const Text('Sin revisiones archivadas.'),
        for (final review in reviews)
          Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Wrap(
                spacing: 8,
                crossAxisAlignment: WrapCrossAlignment.center,
                children: [
                  Text(
                    '${archived ? 'Archivada' : 'Revisión'} · versión ${review['version']?['round'] ?? 1}',
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
                  if (review['chat_request'] != null)
                    const Text('Solicitada desde el chat'),
                  if (!archived && review['version']?['next_review_id'] == null)
                    TextButton(
                      onPressed: locked || review['state'] == 'evaluating'
                          ? null
                          : () => archive(review),
                      child: const Text('Archivar revisión'),
                    ),
                ],
              ),
              if (!archived &&
                  review['state'] == 'waiting_bot' &&
                  review['version']?['next_review_id'] == null) ...[
                const Text(
                  'Revisión preparada. Gennie puede consultar la selección desde el chat.',
                ),
                if ((review['version']?['round'] ?? 1) > 1)
                  Text(
                    'Versión ${review['version']['round']} · continúa la respuesta del usuario sin otra consulta a la guía.',
                  ),
                TextButton(
                  onPressed: locked
                      ? null
                      : () => cancel(review['id'] as String),
                  child: const Text('Cancelar revisión'),
                ),
              ],
              for (final raw in review['proposals'] as List)
                if (raw['candidate']['assignment_id'].toString() ==
                    widget.record.id)
                  ListTile(
                    contentPadding: EdgeInsets.zero,
                    title: Text(raw['candidate']['title'] as String),
                    subtitle: Text(
                      raw['resolution'] == 'applied'
                          ? 'Cambios aplicados'
                          : raw['resolution'] == 'dismissed'
                          ? 'Propuesta descartada'
                          : archived
                          ? 'Borrador archivado · solo consulta'
                          : review['version']?['next_review_id'] != null
                          ? 'Versión anterior · disponible para consulta'
                          : review['current'] != true
                          ? 'Los encargos cambiaron; solicita otra revisión'
                          : 'Propuesta de Gennie · pendiente de revisión',
                    ),
                    onTap: locked
                        ? null
                        : () => inspect(
                            review,
                            Map<String, dynamic>.from(raw as Map),
                          ),
                  ),
            ],
          ),
        if (total != null && reviews.isNotEmpty)
          Text(
            '${reviews.length} de $total revisiones',
            style: Theme.of(context).textTheme.bodySmall,
          ),
        if (nextCursor != null)
          TextButton(
            onPressed: locked ? null : () => load(more: true),
            child: const Text('Cargar revisiones anteriores'),
          ),
      ],
    );
  }
}

class _ReviewDialog extends StatefulWidget {
  const _ReviewDialog({required this.state, required this.record});
  final DesktopState state;
  final AssignmentRecord record;
  @override
  State<_ReviewDialog> createState() => _ReviewDialogState();
}

class _ReviewDialogState extends State<_ReviewDialog> {
  final instructions = TextEditingController(
    text: 'Propón mejoras de proactividad y señala duplicados o conflictos.',
  );
  final form = GlobalKey<FormState>();
  final instructionsKey = GlobalKey(), errorKey = GlobalKey();
  final instructionsFocus = FocusNode();
  late final Set<String> selected = {widget.record.id};
  String requestKey = 'review-${DateTime.now().microsecondsSinceEpoch}';
  String? prepared;
  late final recordsAtOpen = List<AssignmentRecord>.from(
    widget.state.snapshot!.assignments,
  );
  bool busy = false, remote = false, ready = false, guideAvailable = false;
  String? error;
  @override
  void initState() {
    super.initState();
    unawaited(availability());
  }

  @override
  void dispose() {
    instructions.dispose();
    instructionsFocus.dispose();
    super.dispose();
  }

  Future<void> availability() async {
    try {
      final result = await _request(widget.state, {'action': 'availability'});
      if (mounted) {
        setState(() {
          ready = result['bot_available'] == true;
          guideAvailable = result['guide_available'] == true;
          if (!ready) {
            error = 'Activa la conversación con Gennie para pedir propuestas.';
          }
        });
        if (!ready) {
          _reveal(errorKey);
        }
      }
    } catch (e) {
      if (mounted) {
        setState(() => error = '$e');
        _reveal(errorKey);
      }
    }
  }

  Future<void> submit() async {
    if (busy || !ready) return;
    if (!form.currentState!.validate()) {
      instructionsFocus.requestFocus();
      _reveal(instructionsKey);
      return;
    }
    if (selected.isEmpty || selected.length > 20) {
      setState(() => error = 'Selecciona entre uno y veinte encargos.');
      _reveal(errorKey);
      return;
    }
    final records = recordsAtOpen
        .where((r) => selected.contains(r.id))
        .toList();
    if (records.length != selected.length ||
        records.any((r) => r.instanceId == null)) {
      setState(
        () => error =
            'Los encargos cambiaron. Cierra y vuelve a abrir la selección.',
      );
      return;
    }
    setState(() {
      busy = true;
      error = null;
    });
    try {
      final payload = {
        'action': 'begin',
        'assignments': records
            .map(
              (r) => {
                'id': int.parse(r.id),
                'instance_id': r.instanceId,
                'revision': r.revision,
              },
            )
            .toList(),
        'instructions': instructions.text.trim(),
        'use_guide': remote,
      };
      final value = jsonEncode(payload);
      if (prepared != null && prepared != value) {
        requestKey = 'review-${DateTime.now().microsecondsSinceEpoch}';
      }
      payload['request_key'] = requestKey;
      final review = await _request(widget.state, payload);
      if (review['current'] != true) {
        throw StateError(
          'Los encargos cambiaron. Cierra y vuelve a abrir la selección.',
        );
      }
      prepared = value;
      final snapshot = await widget.state.backend.send(
        'Revisa los encargos que seleccioné y prepara propuestas revisables. ${instructions.text.trim()}',
      );
      widget.state.accept(snapshot);
      if (mounted) Navigator.pop(context, true);
    } catch (e) {
      if (mounted) {
        setState(() {
          error = 'No se pudo pedir la revisión: $e';
          busy = false;
        });
        _reveal(errorKey);
      }
    }
  }

  @override
  Widget build(BuildContext context) => AppReviewDialog(
    title: 'Proponer mejoras',
    busy: busy,
    actions: [
      TextButton(
        onPressed: busy ? null : () => Navigator.pop(context),
        child: const Text('Cancelar'),
      ),
      FilledButton(
        onPressed: busy || !ready ? null : submit,
        child: Text(busy ? 'Preparando…' : 'Pedir propuestas'),
      ),
    ],
    child: Form(
      key: form,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Text(
            'Gennie preparará borradores con los encargos seleccionados. Podrás revisar los cambios antes de aplicarlos.',
          ),
          const SizedBox(height: 16),
          TextFormField(
            key: instructionsKey,
            focusNode: instructionsFocus,
            controller: instructions,
            enabled: !busy,
            minLines: 2,
            maxLines: 5,
            decoration: const InputDecoration(labelText: 'Qué quieres mejorar'),
            validator: (v) =>
                v == null || v.trim().isEmpty || v.runes.length > 600
                ? 'Escribe entre 1 y 600 caracteres.'
                : null,
          ),
          const SizedBox(height: 12),
          ExpansionTile(
            tilePadding: EdgeInsets.zero,
            title: Text('Encargos incluidos · ${selected.length}'),
            children: [
              for (final r in recordsAtOpen)
                CheckboxListTile(
                  contentPadding: EdgeInsets.zero,
                  value: selected.contains(r.id),
                  title: Text(jsonDecode(r.document)['title'] as String),
                  onChanged: busy
                      ? null
                      : (v) => setState(() {
                          if (v == true) {
                            selected.add(r.id);
                          } else {
                            selected.remove(r.id);
                          }
                        }),
                ),
            ],
          ),
          CheckboxListTile(
            contentPadding: EdgeInsets.zero,
            value: remote,
            title: const Text('Valorar con la guía remota'),
            subtitle: Text(
              guideAvailable
                  ? 'Enviar esta selección, la orientación y los borradores a OpenRouter para valorar utilidad, duplicados y alcance. La guía no aplica cambios.'
                  : 'Disponible al configurar y habilitar la guía en Arquitectos.',
            ),
            onChanged: busy || !guideAvailable
                ? null
                : (v) => setState(() => remote = v == true),
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

class _ReviewContext extends StatelessWidget {
  const _ReviewContext({
    required this.title,
    required this.text,
    required this.truncated,
    required this.excerptLabel,
  });
  final String title, text, excerptLabel;
  final bool truncated;
  @override
  Widget build(BuildContext context) {
    final style = Theme.of(context).textTheme.labelLarge;
    if (text.runes.length > 400) {
      return ExpansionTile(
        tilePadding: EdgeInsets.zero,
        childrenPadding: const EdgeInsets.symmetric(vertical: 8),
        title: Text(title, style: style),
        subtitle: Text(text, maxLines: 2, overflow: TextOverflow.ellipsis),
        children: [Text(text), if (truncated) Text(excerptLabel)],
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(title, style: style),
        Text(text),
        if (truncated) Text(excerptLabel),
      ],
    );
  }
}

class _ProposalDialog extends StatefulWidget {
  const _ProposalDialog({
    required this.state,
    required this.review,
    required this.proposal,
    required this.onOpenChat,
  });
  final DesktopState state;
  final Map<String, dynamic> review, proposal;
  final VoidCallback onOpenChat;
  @override
  State<_ProposalDialog> createState() => _ProposalDialogState();
}

class _ProposalDialogState extends State<_ProposalDialog> {
  final errorKey = GlobalKey();
  bool busy = false;
  String? error;
  Future<void> resolve(bool apply) async {
    if (busy) return;
    setState(() {
      busy = true;
      error = null;
    });
    try {
      await _request(widget.state, {
        'action': 'resolve',
        'proposal_id': widget.proposal['id'],
        'apply': apply,
      });
      if (mounted) Navigator.pop(context);
    } catch (e) {
      if (mounted) {
        setState(() {
          busy = false;
          error = 'No se pudieron guardar los cambios: $e';
        });
        _reveal(errorKey);
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final p = Map<String, dynamic>.from(widget.proposal['candidate'] as Map);
    final baseline =
        (widget.review['scope']['assignments'] as List).firstWhere(
              (a) => a['id'] == p['assignment_id'],
            )['document']
            as Map;
    final proposed = p['document'] as Map?;
    final questions = p['questions'] as List;
    final pending = widget.proposal['resolution'] == 'pending';
    final next = widget.review['version']?['next_review_id'];
    final active =
        widget.review['state'] != 'evaluating' &&
        next == null &&
        widget.review['archived'] != true;
    final canApply =
        pending &&
        active &&
        widget.review['current'] == true &&
        questions.isEmpty &&
        proposed != null;
    return AppReviewDialog(
      title: p['title'] as String,
      busy: busy,
      actions: [
        TextButton(
          onPressed: busy ? null : () => Navigator.pop(context),
          child: const Text('Cerrar'),
        ),
        if (pending)
          TextButton(
            onPressed: busy || !active ? null : () => resolve(false),
            child: const Text('Descartar'),
          ),
        if (pending)
          FilledButton(
            onPressed: busy || !canApply ? null : () => resolve(true),
            child: Text(busy ? 'Guardando…' : 'Aplicar cambios'),
          ),
      ],
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(p['rationale'] as String),
          if (widget.review['chat_request'] != null) ...[
            const SizedBox(height: 12),
            _ReviewContext(
              title: 'Solicitud del chat',
              text: widget.review['chat_request']['text'] as String,
              truncated: widget.review['chat_request']['truncated'] == true,
              excerptLabel: 'Se muestra un extracto de la solicitud original.',
            ),
          ],
          if (widget.review['version']?['response'] != null) ...[
            const SizedBox(height: 12),
            _ReviewContext(
              title: 'Respuesta para esta versión',
              text: widget.review['version']['response']['text'] as String,
              truncated:
                  widget.review['version']['response']['truncated'] == true,
              excerptLabel: 'Se muestra un extracto de la respuesta original.',
            ),
          ],
          if ((widget.review['version']?['round'] ?? 1) > 1)
            Text(
              'Versión ${widget.review['version']['round']} · nueva propuesta tras tu respuesta. No se repitió la guía.',
            ),
          const SizedBox(height: 12),
          Text('Beneficio esperado: ${p['expected_benefit']}'),
          const SizedBox(height: 16),
          if (widget.review['archived'] == true)
            const Text(
              'Esta revisión está archivada. Puedes consultar sus borradores; para continuar, solicita una revisión nueva.',
            ),
          if (next != null) ...[
            const Text(
              'Esta es una versión anterior. Los cambios se revisan en la versión posterior.',
            ),
            TextButton(
              onPressed: busy
                  ? null
                  : () => Navigator.pop(context, next as String),
              child: const Text('Ver versión posterior'),
            ),
          ] else if (widget.review['current'] != true &&
              widget.review['archived'] != true)
            const Text(
              'Los encargos cambiaron desde esta revisión. Solicita una propuesta nueva.',
            ),
          if (widget.review['state'] == 'guide_failed' ||
              widget.review['state'] == 'interrupted')
            const Text(
              'La valoración de la guía no se completó. Estos borradores no tienen su valoración.',
            ),
          if (widget.review['guide'] != null)
            const Text(
              'Valoración de la guía recibida. Es una orientación, no una garantía del resultado.',
            ),
          for (final question in questions)
            Padding(
              padding: const EdgeInsets.only(top: 8),
              child: Text('Por aclarar con Gennie: $question'),
            ),
          if (questions.isNotEmpty && pending && next == null) ...[
            if (widget.review['origin_current'] == false)
              const Text(
                'Esta revisión pertenece a otra conversación. Solicita una revisión nueva desde el chat actual para continuar.',
              ),
            if (widget.review['version']?['input_verified'] == false)
              const Text(
                'Esta revisión anterior no tiene una intervención de origen verificable. Sus borradores se conservan; solicita una revisión nueva para continuar en el chat.',
              ),
            if (widget.review['bot_available'] == false)
              const Text('Activa la conversación para continuar con Gennie.'),
            TextButton(
              onPressed:
                  busy ||
                      !active ||
                      widget.review['current'] != true ||
                      widget.review['version']?['input_verified'] == false ||
                      widget.review['bot_available'] == false ||
                      widget.review['origin_current'] == false
                  ? null
                  : () {
                      Navigator.pop(context);
                      widget.onOpenChat();
                    },
              child: const Text('Responder en el chat'),
            ),
          ],
          if (proposed != null) ...[
            const SizedBox(height: 16),
            const Text(
              'Cambios propuestos',
              style: TextStyle(fontWeight: FontWeight.w600),
            ),
            for (final key in {...baseline.keys, ...proposed.keys}.where(
              (k) => jsonEncode(proposed[k]) != jsonEncode(baseline[k]),
            )) ...[
              const SizedBox(height: 16),
              Text(
                _labels[key] ?? key.toString(),
                style: const TextStyle(fontWeight: FontWeight.w600),
              ),
              const SizedBox(height: 4),
              Text('Actual: ${_value(baseline[key])}'),
              const SizedBox(height: 4),
              Text('Propuesto: ${_value(proposed[key])}'),
            ],
          ],
          if (error != null)
            Padding(
              key: errorKey,
              padding: const EdgeInsets.only(top: 12),
              child: Text(
                error!,
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            ),
        ],
      ),
    );
  }
}

const _labels = {
  'title': 'Título',
  'description': 'Objetivo',
  'context': 'Contexto',
  'reason': 'Motivo',
  'kind': 'Tipo',
  'priority': 'Importancia',
  'status': 'Estado',
  'timezone': 'Zona horaria',
  'notify_at': 'Entrega puntual',
  'recurrence': 'Entrega recurrente',
  'preparation': 'Preparación',
  'applications': 'Aplicaciones',
  'follow_up': 'Seguimiento',
  'method': 'Método',
  'authorization': 'Autorización',
  'scope': 'Alcance',
  'allowed_routes': 'Vías permitidas',
  'functions': 'Funciones',
  'plugin_id': 'Plugin',
  'frequency': 'Frecuencia',
  'local_time': 'Hora local',
  'weekdays': 'Días (lunes 1–domingo 7)',
  'interval_minutes': 'Intervalo en minutos',
  'rationale': 'Justificación',
  'steps': 'Pasos',
  'signals': 'Aspectos a valorar',
  'decision_guidance': 'Criterio para intervenir',
  'action_scope': 'Acciones delegadas',
  'name': 'Nombre',
  'url': 'Dirección',
  'plugin_available': 'Integración histórica',
  'manual_required': 'Acceso manual histórico',
};
String _value(dynamic value) {
  if (value == null) return 'Sin definir';
  if (value is Map) {
    return value.entries
        .map((e) => '${_labels[e.key] ?? e.key}: ${_value(e.value)}')
        .join('\n');
  }
  if (value is List) {
    return value.isEmpty ? 'Ninguno' : value.map(_value).join(' · ');
  }
  if (value is bool) return value ? 'Sí' : 'No';
  const values = {
    'active': 'Activo',
    'paused': 'Pausado',
    'completed': 'Completado',
    'cancelled': 'Cancelado',
    'recurring': 'Recurrente',
    'one_time': 'Puntual',
    'goal': 'Objetivo',
    'daily': 'Diaria',
    'weekly': 'Semanal',
    'normal': 'Normal',
    'high': 'Alta',
    'low': 'Baja',
    'critical': 'Crítica',
    'http': 'HTTP',
    'web': 'Navegador',
    'computer_use': 'Control del ordenador',
    'desktop': 'Escritorio',
  };
  return values[value] ?? value.toString();
}
