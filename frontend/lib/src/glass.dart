import 'dart:ui';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_svg/flutter_svg.dart';

// macOS aporta el fondo desde el runner; los demás destinos usan fondo propio.
class GlassCanvas extends StatelessWidget {
  const GlassCanvas({super.key, required this.child});
  final Widget child;

  @override
  Widget build(BuildContext context) {
    if (defaultTargetPlatform == TargetPlatform.macOS &&
        !MediaQuery.highContrastOf(context)) {
      return child;
    }
    final dark = Theme.of(context).brightness == Brightness.dark;
    return DecoratedBox(
      decoration: BoxDecoration(
        gradient: LinearGradient(
          begin: Alignment.topLeft,
          end: Alignment.bottomRight,
          colors: dark
              ? const [Color(0xff454b55), Color(0xff24262c), Color(0xff353b40)]
              : const [Color(0xffe0e7ed), Color(0xfff5f5f4), Color(0xffd8e2e2)],
        ),
      ),
      child: DecoratedBox(
        decoration: BoxDecoration(
          gradient: RadialGradient(
            center: const Alignment(-.65, -.8),
            radius: 1.4,
            colors: [
              Colors.white.withValues(alpha: dark ? .10 : .45),
              Colors.white.withValues(alpha: 0),
            ],
          ),
        ),
        child: child,
      ),
    );
  }
}

// Material reutilizable; el filtro afecta al fondo, nunca al texto ni controles.
class Glass extends StatelessWidget {
  const Glass({
    super.key,
    required this.child,
    this.radius = 12,
    this.padding = EdgeInsets.zero,
    this.canvas = false,
    this.showBorder = true,
    this.tintOpacity = .78,
    this.blurSigma = 16,
  });
  final Widget child;
  final double radius;
  final EdgeInsetsGeometry padding;
  final bool canvas;
  final bool showBorder;
  final double tintOpacity;
  final double blurSigma;
  @override
  Widget build(BuildContext context) {
    final dark = Theme.of(context).brightness == Brightness.dark;
    final highContrast = MediaQuery.highContrastOf(context);
    final nativeBackdrop = defaultTargetPlatform == TargetPlatform.macOS;
    final tint = dark ? const Color(0xff26282c) : const Color(0xfff5f5f4);
    return ClipRRect(
      borderRadius: BorderRadius.circular(radius),
      child: BackdropFilter(
        enabled: !highContrast,
        filter: ImageFilter.blur(sigmaX: blurSigma, sigmaY: blurSigma),
        child: DecoratedBox(
          decoration: BoxDecoration(
            color: tint.withValues(
              alpha: highContrast
                  ? 1
                  : (canvas ? (nativeBackdrop ? .08 : .50) : tintOpacity),
            ),
            gradient: highContrast
                ? null
                : LinearGradient(
                    begin: Alignment.topLeft,
                    end: Alignment.bottomRight,
                    colors: [
                      Colors.white.withValues(
                        alpha: dark
                            ? .07
                            : (canvas && nativeBackdrop ? .06 : .28),
                      ),
                      Colors.white.withValues(alpha: 0),
                      Colors.white.withValues(
                        alpha: dark
                            ? .02
                            : (canvas && nativeBackdrop ? .02 : .10),
                      ),
                    ],
                    stops: const [0, .48, 1],
                  ),
          ),
          // El brillo se pinta encima del material sin interceptar entrada.
          child: DecoratedBox(
            position: DecorationPosition.foreground,
            decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(radius),
              border: canvas || highContrast || !showBorder
                  ? null
                  : Border.all(
                      color: Colors.white.withValues(alpha: dark ? .16 : .55),
                      width: .7,
                    ),
            ),
            child: Material(
              type: MaterialType.transparency,
              child: Padding(padding: padding, child: child),
            ),
          ),
        ),
      ),
    );
  }
}

// Único punto de presentación de tooltips en toda la aplicación.
class AppTooltip extends StatefulWidget {
  const AppTooltip({
    super.key,
    required this.label,
    required this.child,
    this.preferRight = false,
  });
  final String label;
  final Widget child;
  final bool preferRight;
  @override
  State<AppTooltip> createState() => _AppTooltipState();
}

class _AppTooltipState extends State<AppTooltip> {
  static _AppTooltipState? _active;
  static const _nativeTooltip = MethodChannel('gennie/tooltip');
  OverlayEntry? _tooltip;
  bool _nativeVisible = false;
  int _tooltipGeneration = 0;
  void hide() {
    if (_active == this) _active = null;
    _tooltipGeneration++;
    if (_nativeVisible) {
      _nativeVisible = false;
      _nativeTooltip.invokeMethod<void>('hide').catchError((Object error) {
        debugPrint('No se pudo ocultar el tooltip nativo: $error');
      });
    }
    _tooltip?.remove();
    _tooltip = null;
  }

  Future<void> show() async {
    _active?.hide();
    hide();
    _active = this;
    final generation = _tooltipGeneration;
    if (defaultTargetPlatform == TargetPlatform.macOS) {
      final box = context.findRenderObject()! as RenderBox;
      final origin = box.localToGlobal(Offset.zero);
      _nativeVisible = true;
      try {
        await _nativeTooltip.invokeMethod<void>('show', {
          'label': widget.label,
          'preferRight': widget.preferRight,
          'x': origin.dx,
          'y': origin.dy,
          'width': box.size.width,
          'height': box.size.height,
          'dark': Theme.of(context).brightness == Brightness.dark,
          'highContrast': MediaQuery.highContrastOf(context),
          'textScale': MediaQuery.textScalerOf(context).scale(12) / 12,
        });
        return;
      } on MissingPluginException {
        // Tests y runners anteriores conservan la alternativa Flutter explícita.
        debugPrint('Tooltip nativo no disponible; se usa el filtro Flutter.');
        _nativeVisible = false;
      } on PlatformException catch (error) {
        debugPrint('No se pudo mostrar el tooltip nativo: $error');
        _nativeVisible = false;
      }
      if (!mounted || generation != _tooltipGeneration) return;
    }
    final overlay = Overlay.of(context);
    final overlayBox = overlay.context.findRenderObject()! as RenderBox;
    final box = context.findRenderObject()! as RenderBox;
    final anchor =
        box.localToGlobal(Offset.zero, ancestor: overlayBox) & box.size;
    _tooltip = OverlayEntry(
      builder: (context) => Positioned.fill(
        child: CustomSingleChildLayout(
          delegate: _TooltipLayout(anchor, widget.preferRight),
          child: IgnorePointer(
            child: Material(
              type: MaterialType.transparency,
              child: Glass(
                radius: 8,
                showBorder: false,
                tintOpacity: .28,
                blurSigma: 24,
                padding: const EdgeInsets.symmetric(
                  horizontal: 14,
                  vertical: 7,
                ),
                child: Text(
                  widget.label,
                  style: const TextStyle(
                    fontFamily: 'Inter',
                    fontSize: 12,
                    fontWeight: FontWeight.w500,
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
    overlay.insert(_tooltip!);
  }

  @override
  void dispose() {
    hide();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => MouseRegion(
    onEnter: (_) => show(),
    onExit: (_) => hide(),
    child: Focus(
      canRequestFocus: false,
      onFocusChange: (focused) => focused ? show() : hide(),
      onKeyEvent: (_, event) {
        if (event.logicalKey == LogicalKeyboardKey.escape ||
            event.logicalKey == LogicalKeyboardKey.enter ||
            event.logicalKey == LogicalKeyboardKey.space) {
          hide();
        }
        return KeyEventResult.ignored;
      },
      child: Listener(
        onPointerDown: (_) => hide(),
        child: Semantics(tooltip: widget.label, child: widget.child),
      ),
    ),
  );
}

class RailButton extends StatelessWidget {
  const RailButton({
    super.key,
    required this.label,
    required this.icon,
    required this.onPressed,
    this.selected = false,
    this.asset,
  });
  final String? asset;
  final String label;
  final IconData icon;
  final VoidCallback onPressed;
  final bool selected;
  @override
  Widget build(BuildContext context) => AppTooltip(
    label: label,
    preferRight: true,
    child: Semantics(
      button: true,
      label: label,
      selected: selected,
      child: SizedBox(
        width: 36,
        height: 36,
        child: IconButton(
          onPressed: onPressed,
          padding: EdgeInsets.zero,
          style: ButtonStyle(
            shape: WidgetStatePropertyAll(
              RoundedRectangleBorder(borderRadius: BorderRadius.circular(8)),
            ),
            backgroundColor: WidgetStateProperty.resolveWith(
              (states) => selected
                  ? const Color(0x24808080)
                  : states.contains(WidgetState.hovered)
                  ? const Color(0x14808080)
                  : Colors.transparent,
            ),
          ),
          icon: asset == null
              ? Icon(
                  icon,
                  size: 19,
                  color: Theme.of(context).brightness == Brightness.dark
                      ? const Color(0xffd0d0d0)
                      : const Color(0xff333333),
                )
              : SvgPicture.asset(
                  asset!,
                  width: 18,
                  height: 18,
                  colorFilter: ColorFilter.mode(
                    Theme.of(context).brightness == Brightness.dark
                        ? const Color(0xffd0d0d0)
                        : const Color(0xff333333),
                    BlendMode.srcIn,
                  ),
                ),
        ),
      ),
    ),
  );
}

// Mantiene la etiqueta dentro del overlay, también en el botón Enviar al pie.
class _TooltipLayout extends SingleChildLayoutDelegate {
  _TooltipLayout(this.anchor, this.preferRight);
  final Rect anchor;
  final bool preferRight;
  @override
  BoxConstraints getConstraintsForChild(BoxConstraints constraints) =>
      BoxConstraints.loose(
        Size(
          (constraints.maxWidth - 16).clamp(0, double.infinity),
          (constraints.maxHeight - 16).clamp(0, double.infinity),
        ),
      );
  @override
  Offset getPositionForChild(Size size, Size childSize) {
    double x, y;
    if (preferRight) {
      x = anchor.right + 4;
      if (x + childSize.width > size.width - 8) {
        x = anchor.left - childSize.width - 4;
      }
      y = anchor.center.dy - childSize.height / 2;
    } else {
      x = anchor.center.dx - childSize.width / 2;
      y = anchor.bottom + 6;
      if (y + childSize.height > size.height - 8) {
        y = anchor.top - childSize.height - 6;
      }
    }
    return Offset(
      x.clamp(8, (size.width - childSize.width - 8).clamp(8, double.infinity)),
      y.clamp(
        8,
        (size.height - childSize.height - 8).clamp(8, double.infinity),
      ),
    );
  }

  @override
  bool shouldRelayout(_TooltipLayout oldDelegate) =>
      anchor != oldDelegate.anchor || preferRight != oldDelegate.preferRight;
}
