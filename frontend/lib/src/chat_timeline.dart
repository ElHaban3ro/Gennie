import 'dart:async';
import 'package:flutter/material.dart';
import 'backend.dart';
import 'chat_message.dart';
import 'sidebar.dart';

/// Historial visual del asistente. Leerlo nunca activa una sesión del proveedor.
class ChatTimeline extends StatefulWidget {
  const ChatTimeline({
    super.key,
    required this.state,
    required this.controller,
    required this.onOpenPlugin,
    required this.onReply,
  });
  final DesktopState state;
  final ScrollController controller;
  final ValueChanged<String> onOpenPlugin;
  final ValueChanged<ChatEntry> onReply;
  @override
  State<ChatTimeline> createState() => _ChatTimelineState();
}

class _ChatTimelineState extends State<ChatTimeline> {
  List<ChatEntry> entries = [];
  ChatCursor? before;
  bool loading = false, loaded = false, refreshPending = false;
  String? error, revision;
  final keys = <String, GlobalKey>{};
  final sourceFocus = <String, FocusNode>{};
  int sourceNavigation = 0;
  DesktopState get state => widget.state;
  ScrollController get scroll => widget.controller;

  @override
  void initState() {
    super.initState();
    state.addListener(changed);
    scroll.addListener(onScroll);
    // La primera consulta comienza después de montar la vista.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) changed();
    });
  }

  @override
  void dispose() {
    state.removeListener(changed);
    scroll.removeListener(onScroll);
    for (final node in sourceFocus.values) {
      node.dispose();
    }
    super.dispose();
  }

  void changed() {
    final snapshot = state.snapshot;
    if (snapshot == null) return;
    if (revision != snapshot.historyRevision) {
      revision = snapshot.historyRevision;
      if (loading) {
        refreshPending = true;
      } else {
        unawaited(load(refresh: true));
      }
    }
  }

  void onScroll() {
    if (loaded &&
        !loading &&
        error == null &&
        before != null &&
        scroll.position.extentAfter < 100) {
      unawaited(load());
    }
  }

  Future<void> load({bool refresh = false}) async {
    if (loading || !mounted) return;
    final backend = state.backend;
    if (backend is! AssistantChatBackend) {
      setState(() => error = 'El backend no permite cargar el chat');
      return;
    }
    final atEnd = !scroll.hasClients || scroll.offset < 70;
    // Guardar un mensaje visible como ancla cuando llegan entradas nuevas.
    String? anchor;
    double? anchorY;
    if (refresh && !atEnd) {
      final viewport = context.findRenderObject() as RenderBox?;
      if (viewport != null && viewport.hasSize) {
        final top = viewport.localToGlobal(Offset.zero).dy;
        for (final entry in entries) {
          final box = keys[entry.id]?.currentContext?.findRenderObject();
          if (box is RenderBox && box.hasSize) {
            final y = box.localToGlobal(Offset.zero).dy;
            if (y >= top && y < top + viewport.size.height) {
              anchor = entry.id;
              anchorY = y;
              break;
            }
          }
        }
      }
    }
    final oldOffset = scroll.hasClients ? scroll.offset : 0.0;
    final oldMax = scroll.hasClients ? scroll.position.maxScrollExtent : 0.0;
    setState(() {
      loading = true;
      error = null;
    });
    try {
      final oldest = refresh ? entries.firstOrNull : null;
      var page = await (backend as AssistantChatBackend).loadChatPage(
        refresh ? null : before,
      );
      if (!mounted) return;
      final incoming = [...page.messages];
      // Reconstruir solo el tramo ya solicitado: también retira mensajes eliminados.
      bool covered() {
        if (oldest == null ||
            page.before == null ||
            incoming.any((entry) => entry.id == oldest.id)) {
          return true;
        }
        final date = page.before!.createdAt.compareTo(oldest.message.createdAt);
        return date < 0 ||
            date == 0 &&
                BigInt.parse(page.before!.messageId) <= BigInt.parse(oldest.id);
      }

      while (refresh && !covered()) {
        final cursor = page.before!;
        page = await (backend as AssistantChatBackend).loadChatPage(cursor);
        if (!mounted) return;
        if (page.before?.messageId == cursor.messageId) {
          throw StateError('El historial no avanzó');
        }
        incoming.insertAll(0, page.messages);
      }
      var nextCursor = page.before;
      if (refresh && oldest != null) {
        final boundary = incoming.indexWhere((entry) => entry.id == oldest.id);
        if (boundary > 0) {
          incoming.removeRange(0, boundary);
          nextCursor = ChatCursor(
            messageId: incoming.first.id,
            createdAt: incoming.first.message.createdAt,
          );
        }
      }
      final unique = <String, ChatEntry>{};
      for (final entry in [...incoming, if (!refresh) ...entries]) {
        unique[entry.id] = entry;
      }
      final next = unique.values.toList();
      keys.removeWhere((id, _) => !unique.containsKey(id));
      final retiredFocus = <FocusNode>[];
      sourceFocus.removeWhere((id, node) {
        if (unique.containsKey(id)) return false;
        retiredFocus.add(node);
        return true;
      });
      // Los hijos aún pueden estar montados con esos nodos hasta el nuevo frame.
      if (retiredFocus.isNotEmpty) {
        WidgetsBinding.instance.addPostFrameCallback((_) {
          for (final node in retiredFocus) {
            node.dispose();
          }
        });
      }
      setState(() {
        entries = next;
        before = nextCursor;
        loaded = true;
      });
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!mounted || !scroll.hasClients) return;
        if (refresh && atEnd) {
          scroll.jumpTo(0);
        } else if (refresh) {
          final box = keys[anchor]?.currentContext?.findRenderObject();
          final delta = box is RenderBox && anchorY != null
              ? anchorY - box.localToGlobal(Offset.zero).dy
              : scroll.position.maxScrollExtent - oldMax;
          scroll.jumpTo(
            (oldOffset + delta).clamp(0, scroll.position.maxScrollExtent),
          );
        }
      });
    } catch (failure) {
      if (mounted) setState(() => error = '$failure');
    } finally {
      if (mounted) {
        setState(() => loading = false);
        if (refreshPending) {
          refreshPending = false;
          unawaited(load(refresh: true));
        }
      }
    }
  }

  Future<void> openSource(String id) async {
    if (loading) return;
    final navigation = ++sourceNavigation;
    while (mounted &&
        !entries.any((entry) => entry.id == id) &&
        before != null &&
        error == null) {
      final previous = before!.messageId;
      await load();
      if (!mounted || before?.messageId == previous) break;
    }
    if (!mounted || !scroll.hasClients) return;
    final target = entries.indexWhere((entry) => entry.id == id);
    if (target < 0) {
      setState(() => error = 'El mensaje original ya no está disponible');
      return;
    }
    for (var attempt = 0; attempt < 12; attempt++) {
      if (!mounted || navigation != sourceNavigation) return;
      final location = keys[id]?.currentContext;
      if (location != null && location.mounted) {
        final node = sourceFocus[id];
        if (node == null) {
          setState(
            () => sourceFocus[id] = FocusNode(debugLabel: 'Mensaje citado'),
          );
          await WidgetsBinding.instance.endOfFrame;
        }
        if (!mounted || navigation != sourceNavigation) return;
        final current = keys[id]?.currentContext;
        if (current == null || !current.mounted) return;
        await Scrollable.ensureVisible(
          current,
          alignment: .25,
          duration: MediaQuery.disableAnimationsOf(context)
              ? Duration.zero
              : const Duration(milliseconds: 180),
        );
        if (mounted && navigation == sourceNavigation) {
          sourceFocus[id]?.requestFocus();
        }
        return;
      }
      final visible = <int>[];
      for (var index = 0; index < entries.length; index++) {
        final box = keys[entries[index].id]?.currentContext?.findRenderObject();
        if (box is RenderBox && box.attached && box.hasSize) visible.add(index);
      }
      double destination;
      if (visible.isEmpty) {
        destination =
            scroll.position.maxScrollExtent *
            (1 - target / (entries.length - 1).clamp(1, entries.length));
      } else {
        final center = visible[visible.length ~/ 2];
        destination =
            scroll.offset +
            (center - target) *
                scroll.position.viewportDimension /
                visible.length;
      }
      scroll.jumpTo(destination.clamp(0, scroll.position.maxScrollExtent));
      WidgetsBinding.instance.scheduleFrame();
      await WidgetsBinding.instance.endOfFrame;
    }
    if (mounted) {
      setState(
        () => error =
            'No se pudo ubicar el mensaje original; vuelve a intentarlo',
      );
    }
  }

  String sessionLabel(String timestamp) =>
      'Nueva conversación · ${chatDayLabel(timestamp, now: DateTime.now())} ${messageTimeLabel(timestamp)}'
          .trim();

  @override
  Widget build(BuildContext context) {
    final snapshot = state.snapshot;
    final transient = <Message>[
      if (snapshot?.reply.isNotEmpty == true)
        Message(
          speaker: 'assistant',
          text: snapshot!.reply,
          createdAt: '',
          interruption: snapshot.replyInterruption,
        ),
      if (state.failure != null &&
          !(snapshot?.errorRecorded == true &&
              state.failure == snapshot?.error))
        Message(
          speaker: 'system',
          text: 'Error: ${state.failure}',
          createdAt: '',
        ),
    ];
    final active = snapshot?.conversations
        .where((session) => session.id == snapshot.activeId)
        .firstOrNull;
    final emptySession =
        active != null &&
        active.lastMessage == null &&
        snapshot?.activeSessionStartedAt != null;
    final sessionSlots = emptySession ? 1 : 0;
    final count = entries.length + sessionSlots + transient.length;
    if (loaded && count == 0 && error == null && !loading) {
      return const ContentEmpty(
        title: '¿En qué te ayudo?',
        message: 'Un espacio para pensar, conversar y hacer.',
      );
    }
    return Align(
      alignment: Alignment.topCenter,
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 840),
        child: SelectionArea(
          child: ListView.builder(
            key: const ValueKey('chat-timeline'),
            controller: scroll,
            reverse: true,
            padding: const EdgeInsets.fromLTRB(32, 24, 32, 24),
            itemCount: count + 1,
            findChildIndexCallback: (key) {
              if (key is ValueKey<String>) {
                final index = entries.indexWhere(
                  (entry) => entry.id == key.value,
                );
                if (index >= 0) return count - 1 - index;
              }
              return null;
            },
            itemBuilder: (context, index) {
              if (index == count) {
                return Center(
                  child: error != null
                      ? Column(
                          children: [
                            Text(error!),
                            TextButton(
                              onPressed: () => load(refresh: true),
                              child: const Text('Reintentar historial'),
                            ),
                          ],
                        )
                      : loading
                      ? const Padding(
                          padding: EdgeInsets.all(8),
                          child: SizedBox(
                            width: 18,
                            height: 18,
                            child: CircularProgressIndicator(strokeWidth: 2),
                          ),
                        )
                      : before != null
                      ? TextButton(
                          onPressed: () => load(),
                          child: const Text('Cargar mensajes anteriores'),
                        )
                      : Padding(
                          padding: const EdgeInsets.all(12),
                          child: Text(
                            count == 0
                                ? '¿En qué te ayudo?'
                                : 'Inicio del historial',
                            style: const TextStyle(fontSize: 11),
                          ),
                        ),
                );
              }
              final chronological = count - 1 - index;
              if (emptySession && chronological == entries.length) {
                return ChatDayBadge(
                  label: sessionLabel(snapshot!.activeSessionStartedAt!),
                );
              }
              if (chronological >= entries.length + sessionSlots) {
                return ChatMessage(
                  message:
                      transient[chronological - entries.length - sessionSlots],
                  state: state,
                  conversationId: snapshot?.activeId,
                  onOpenPlugin: widget.onOpenPlugin,
                );
              }
              final entry = entries[chronological];
              final previous = chronological == 0
                  ? null
                  : entries[chronological - 1];
              final day = messageLocalDay(entry.message.createdAt);
              return KeyedSubtree(
                key: ValueKey(entry.id),
                child: Column(
                  key: keys.putIfAbsent(entry.id, () => GlobalKey()),
                  children: [
                    if (day != null &&
                        day !=
                            messageLocalDay(previous?.message.createdAt ?? ''))
                      ChatDayBadge(
                        label: chatDayLabel(
                          entry.message.createdAt,
                          now: DateTime.now(),
                        ),
                      ),
                    if (entry.startsSession)
                      ChatDayBadge(label: sessionLabel(entry.sessionStartedAt)),
                    ChatMessage(
                      focusNode: sourceFocus[entry.id],
                      message: entry.message,
                      delegation: entry.delegation,
                      execution: entry.execution,
                      tools: entry.tools,
                      quote: entry.quote,
                      onReply: () => widget.onReply(entry),
                      onOpenQuote:
                          entry.quote?.sourceMessageId == null || loading
                          ? null
                          : () => openSource(entry.quote!.sourceMessageId!),
                      state: state,
                      conversationId: entry.conversationId,
                      onOpenPlugin: widget.onOpenPlugin,
                    ),
                  ],
                ),
              );
            },
          ),
        ),
      ),
    );
  }
}
