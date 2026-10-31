import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter/semantics.dart';

/// Menú de burbuja con clic secundario, Shift+F10 y acción semántica.
class MessageActions extends StatefulWidget {
  const MessageActions({
    super.key,
    required this.child,
    required this.onReply,
    this.focusNode,
  });
  final Widget child;
  final VoidCallback? onReply;
  final FocusNode? focusNode;
  @override
  State<MessageActions> createState() => _MessageActionsState();
}

class _MessageActionsState extends State<MessageActions> {
  bool focused = false;
  Future<void> menu([Offset? position]) async {
    final overlay =
        Overlay.of(context).context.findRenderObject()! as RenderBox;
    final box = context.findRenderObject()! as RenderBox;
    final origin = position ?? box.localToGlobal(box.size.center(Offset.zero));
    final result = await showMenu<String>(
      context: context,
      position: RelativeRect.fromRect(
        origin & const Size(1, 1),
        Offset.zero & overlay.size,
      ),
      items: const [PopupMenuItem(value: 'reply', child: Text('Responder'))],
    );
    if (mounted && result == 'reply') widget.onReply?.call();
  }

  @override
  Widget build(BuildContext context) {
    if (widget.onReply == null) return widget.child;
    return CallbackShortcuts(
      bindings: {
        const SingleActivator(LogicalKeyboardKey.f10, shift: true): menu,
        const SingleActivator(LogicalKeyboardKey.contextMenu): menu,
      },
      child: FocusableActionDetector(
        focusNode: widget.focusNode,
        onShowFocusHighlight: (value) => setState(() => focused = value),
        child: Semantics(
          customSemanticsActions: {
            const CustomSemanticsAction(label: 'Responder'): widget.onReply!,
          },
          child: GestureDetector(
            onSecondaryTapUp: (details) => menu(details.globalPosition),
            child: Container(
              // Mantener la capa al perder foco evita desmontar enlaces hijos.
              foregroundDecoration: focused
                  ? BoxDecoration(
                      border: Border.all(
                        color: Theme.of(context).colorScheme.primary,
                        width: 2,
                      ),
                      borderRadius: BorderRadius.circular(16),
                    )
                  : const BoxDecoration(),
              child: widget.child,
            ),
          ),
        ),
      ),
    );
  }
}
