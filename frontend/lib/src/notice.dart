import 'dart:math' as math;
import 'package:flutter/material.dart';
import 'glass.dart';
import 'sidebar.dart';

/// Confirmación breve y compartida; reemplaza avisos previos para no acumularlos.
void showAppNotice(BuildContext context, String message) {
  final messenger = ScaffoldMessenger.of(context);
  messenger.removeCurrentSnackBar();
  messenger.showSnackBar(
    SnackBar(
      behavior: SnackBarBehavior.floating,
      width: math.min(360, MediaQuery.sizeOf(context).width - 32),
      backgroundColor: Colors.transparent,
      elevation: 0,
      padding: EdgeInsets.zero,
      duration: const Duration(seconds: 4),
      content: AppNotice(
        message: message,
        onClose: messenger.hideCurrentSnackBar,
      ),
    ),
  );
}

class AppNotice extends StatelessWidget {
  const AppNotice({super.key, required this.message, required this.onClose});
  final String message;
  final VoidCallback onClose;

  @override
  Widget build(BuildContext context) {
    final ink = sidebarInk(context);
    return Glass(
      radius: 12,
      tintOpacity: .92,
      padding: const EdgeInsets.fromLTRB(14, 10, 8, 10),
      child: Row(
        children: [
          Icon(Icons.check_circle_outline, size: 18, color: ink),
          const SizedBox(width: 10),
          Expanded(
            child: Text(
              message,
              style: TextStyle(
                fontFamily: 'Inter',
                fontSize: 13,
                fontWeight: FontWeight.w500,
                height: 1.4,
                color: ink,
              ),
            ),
          ),
          const SizedBox(width: 8),
          AppTooltip(
            label: 'Cerrar aviso',
            child: Semantics(
              label: 'Cerrar aviso',
              child: IconButton(
                onPressed: onClose,
                style: IconButton.styleFrom(
                  padding: EdgeInsets.zero,
                  minimumSize: const Size(28, 28),
                  maximumSize: const Size(28, 28),
                  tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                ),
                constraints: const BoxConstraints.tightFor(
                  width: 28,
                  height: 28,
                ),
                icon: Icon(
                  Icons.close,
                  size: 14,
                  color: ink.withValues(alpha: .7),
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}
