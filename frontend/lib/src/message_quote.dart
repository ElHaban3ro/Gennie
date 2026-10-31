import 'package:flutter/material.dart';
import 'backend.dart';
import 'glass.dart';

class MessageQuote extends StatelessWidget {
  const MessageQuote({
    super.key,
    required this.quote,
    this.assistantName = 'Gennie',
    this.onOpen,
  });
  final ReplyQuote quote;
  final String assistantName;
  final VoidCallback? onOpen;
  @override
  Widget build(BuildContext context) {
    final ink = Theme.of(context).colorScheme.onSurface;
    final author = switch (quote.speaker) {
      'user' => 'Tú',
      'assistant' => assistantName,
      _ => 'Sistema',
    };
    final content = Container(
      width: double.infinity,
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
      decoration: BoxDecoration(
        color: Colors.grey.withValues(alpha: .10),
        border: Border(
          left: BorderSide(color: ink.withValues(alpha: .4), width: 3),
        ),
        borderRadius: BorderRadius.circular(6),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(
            author,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: const TextStyle(fontSize: 11, fontWeight: FontWeight.w600),
          ),
          const SizedBox(height: 3),
          Text(
            '${quote.text}${quote.truncated ? '…' : ''}',
            maxLines: 2,
            overflow: TextOverflow.ellipsis,
            style: const TextStyle(fontSize: 12, height: 1.4),
          ),
          if (quote.sourceMessageId == null)
            const Text(
              'Mensaje original no disponible',
              style: TextStyle(fontSize: 10),
            ),
        ],
      ),
    );
    if (onOpen == null) return content;
    return AppTooltip(
      label: 'Ver mensaje original',
      child: Semantics(
        button: true,
        label: quote.speaker == 'user'
            ? 'Ver tu mensaje original'
            : 'Ver mensaje original de $author',
        child: InkWell(
          onTap: onOpen,
          borderRadius: BorderRadius.circular(6),
          child: content,
        ),
      ),
    );
  }
}
