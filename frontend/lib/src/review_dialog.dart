import 'package:flutter/material.dart';

class AppReviewDialog extends StatelessWidget {
  const AppReviewDialog({
    super.key,
    required this.title,
    required this.busy,
    required this.actions,
    required this.child,
  });
  final String title;
  final bool busy;
  final List<Widget> actions;
  final Widget child;
  @override
  Widget build(BuildContext context) => PopScope(
    canPop: !busy,
    child: Actions(
      actions: <Type, Action<Intent>>{
        DismissIntent: CallbackAction<DismissIntent>(
          onInvoke: (_) {
            if (!busy) Navigator.of(context).pop();
            return null;
          },
        ),
      },
      child: FocusScope(
        autofocus: true,
        child: Dialog(
          backgroundColor: Theme.of(context).colorScheme.surfaceContainerHigh,
          insetPadding: const EdgeInsets.all(16),
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 660, maxHeight: 680),
            child: Padding(
              padding: const EdgeInsets.all(20),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Text(title, style: Theme.of(context).textTheme.titleLarge),
                  const SizedBox(height: 16),
                  Flexible(
                    child: Scrollbar(
                      thumbVisibility: true,
                      child: SingleChildScrollView(primary: true, child: child),
                    ),
                  ),
                  const SizedBox(height: 16),
                  Wrap(
                    alignment: WrapAlignment.end,
                    spacing: 8,
                    runSpacing: 8,
                    children: actions,
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
