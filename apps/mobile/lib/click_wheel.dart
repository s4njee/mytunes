import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

import 'pod_navigation.dart';
import 'theme/mt_theme.dart';
import 'theme/tokens.g.dart';
import 'wheel_detents.dart';

/// The fixed labels and buttons surround a freely rotating ring.
class ClickWheel extends StatefulWidget {
  const ClickWheel({
    super.key,
    required this.screen,
    this.playing = false,
    this.eqBand = '60',
    this.onStep,
    this.onMenu,
    this.onPrevious,
    this.onNext,
    this.onPlayPause,
    this.onCentre,
  });

  final PodScreen screen;
  final bool playing;
  final String eqBand;

  /// Called once for each detent with +1 clockwise or -1 counterclockwise.
  final ValueChanged<int>? onStep;
  final VoidCallback? onMenu;
  final VoidCallback? onPrevious;
  final VoidCallback? onNext;
  final VoidCallback? onPlayPause;
  final VoidCallback? onCentre;

  @override
  State<ClickWheel> createState() => _ClickWheelState();
}

const _wheelRadius = MtWheel.diameter / 2;
const _centreRadius = MtWheel.centreButton / 2;
const _wheelCentre = Offset(_wheelRadius, _wheelRadius);
const _buttonDragDegrees = 6.0;

class _ClickWheelState extends State<ClickWheel> {
  int? _pointer;
  _Button? _pressed;
  Offset _down = Offset.zero;
  bool _tapCancelled = false;
  WheelDetents? _drag;
  double _rotation = 0;

  void _pointerDown(PointerDownEvent event) {
    if (_pointer != null || event.buttons != kPrimaryButton) return;
    final position = event.localPosition - _wheelCentre;
    final button = _Button.at(position);
    if (button == null && position.distance > _wheelRadius) return;
    _pointer = event.pointer;
    _down = position;
    _pressed = button;
    _tapCancelled = false;
    if (button == null) {
      setState(() => _drag = WheelDetents(wheelAngle(position)));
    }
  }

  void _pointerMove(PointerMoveEvent event) {
    if (event.pointer != _pointer) return;
    final position = event.localPosition - _wheelCentre;
    final drag = _drag;
    if (drag == null) {
      _checkTapMotion(position);
      return;
    }
    // The angle at the exact centre is undefined.
    if (position == Offset.zero) return;
    final motion = drag.moveTo(wheelAngle(position));
    if (motion.delta != 0) setState(() => _rotation += motion.delta);
    final direction = motion.steps.sign;
    for (var step = 0; step < motion.steps.abs(); step++) {
      unawaited(HapticFeedback.selectionClick());
      widget.onStep?.call(direction);
    }
  }

  void _checkTapMotion(Offset position) {
    if ((position - _down).distance > kTouchSlop ||
        (_pressed != _Button.centre &&
            shortestWheelDelta(wheelAngle(_down), wheelAngle(position)).abs() >
                _buttonDragDegrees)) {
      _tapCancelled = true;
    }
  }

  void _pointerUp(PointerUpEvent event) {
    if (event.pointer != _pointer) return;
    final button = _pressed;
    final position = event.localPosition - _wheelCentre;
    if (button != null) _checkTapMotion(position);
    _finishPointer();
    if (button != null && !_tapCancelled && _Button.at(position) == button) {
      _press(button);
    }
  }

  void _finishPointer() {
    _pointer = null;
    _pressed = null;
    if (_drag != null) setState(() => _drag = null);
  }

  VoidCallback? _action(_Button button) => switch (button) {
    _Button.menu => widget.onMenu,
    _Button.previous => widget.onPrevious,
    _Button.next => widget.onNext,
    _Button.playPause => widget.onPlayPause,
    _Button.centre => widget.onCentre,
  };

  void _press(_Button button) {
    final action = _action(button);
    if (action == null) return;
    unawaited(HapticFeedback.lightImpact());
    action();
  }

  String get _centreLabel => switch (widget.screen) {
    PodScreen.menu => 'SELECT',
    PodScreen.nowPlaying => widget.playing ? 'PAUSE' : 'PLAY',
    PodScreen.equalizer => 'BAND ${widget.eqBand}',
    _ => 'PLAY',
  };

  String get _hint => switch (widget.screen) {
    PodScreen.nowPlaying => 'DRAG RING TO SCRUB',
    PodScreen.equalizer => 'DRAG RING TO SET GAIN',
    _ => 'DRAG RING TO BROWSE',
  };

  Widget _button(_Button button, String label, Widget child) {
    final enabled = _action(button) != null;
    return Positioned.fromRect(
      rect: button.bounds,
      child: Semantics(
        button: true,
        enabled: enabled,
        excludeSemantics: true,
        label: label,
        onTap: enabled ? () => _press(button) : null,
        child: child,
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final theme = MtTheme.of(context);
    final colors = theme.colors;
    final dragging = _drag != null;
    final glyph = MtType.wheelGlyph.copyWith(color: colors.textTimecode);
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        SizedBox.square(
          dimension: MtWheel.diameter,
          child: MouseRegion(
            cursor: dragging
                ? SystemMouseCursors.grabbing
                : SystemMouseCursors.grab,
            child: Listener(
              behavior: HitTestBehavior.opaque,
              onPointerDown: _pointerDown,
              onPointerMove: _pointerMove,
              onPointerUp: _pointerUp,
              onPointerCancel: (event) {
                if (event.pointer == _pointer) _finishPointer();
              },
              child: Stack(
                clipBehavior: Clip.none,
                children: [
                  Positioned.fill(
                    child: _WheelFace(theme: theme, rotation: _rotation),
                  ),
                  _button(
                    _Button.menu,
                    'MENU',
                    Align(
                      alignment: Alignment.topCenter,
                      child: Padding(
                        padding: const EdgeInsets.only(
                          top: MtWheel.menuLabelTop,
                        ),
                        child: Text(
                          'MENU',
                          style: MtType.wheelMenu.copyWith(
                            color: colors.textTimecode,
                          ),
                        ),
                      ),
                    ),
                  ),
                  _button(
                    _Button.previous,
                    'Previous track',
                    Center(child: Text('◀◀', style: glyph)),
                  ),
                  _button(
                    _Button.next,
                    'Next track',
                    Center(child: Text('▶▶', style: glyph)),
                  ),
                  _button(
                    _Button.playPause,
                    'Play or pause',
                    Center(
                      child: Text('▶ ❙❙', style: glyph.copyWith(fontSize: 14)),
                    ),
                  ),
                  _button(
                    _Button.centre,
                    _centreLabel,
                    DecoratedBox(
                      decoration: BoxDecoration(
                        shape: BoxShape.circle,
                        color: colors.centreFace,
                        border: Border.all(color: colors.accentHairline),
                        boxShadow: [
                          theme.brightness == Brightness.dark
                              ? const BoxShadow(
                                  color: Color(0x99000000),
                                  offset: Offset(0, 4),
                                  blurRadius: 14,
                                )
                              : const BoxShadow(
                                  color: Color(0x33141E2D),
                                  offset: Offset(0, 2),
                                  blurRadius: 9,
                                ),
                        ],
                      ),
                      child: Center(
                        child: Text(
                          _centreLabel,
                          style: MtType.wheelCentre.copyWith(
                            color: colors.accent,
                          ),
                        ),
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
        const SizedBox(height: MtSpace.deckGap),
        Text(
          _hint,
          style: MtType.wheelHint.copyWith(
            color: dragging ? colors.accent : colors.textHint,
          ),
        ),
      ],
    );
  }
}

enum _Button {
  menu(
    Rect.fromLTWH(
      MtWheel.menuHitInsetX,
      MtWheel.menuHitTop,
      MtWheel.diameter - 2 * MtWheel.menuHitInsetX,
      MtWheel.menuHitHeight,
    ),
  ),
  previous(
    Rect.fromLTWH(
      0,
      MtWheel.sideHitTop,
      MtWheel.sideHitSize,
      MtWheel.sideHitSize,
    ),
  ),
  next(
    Rect.fromLTWH(
      MtWheel.diameter - MtWheel.sideHitSize,
      MtWheel.sideHitTop,
      MtWheel.sideHitSize,
      MtWheel.sideHitSize,
    ),
  ),
  playPause(
    Rect.fromLTWH(
      MtWheel.bottomHitInsetX,
      MtWheel.diameter - MtWheel.bottomHitHeight,
      MtWheel.diameter - 2 * MtWheel.bottomHitInsetX,
      MtWheel.bottomHitHeight,
    ),
  ),
  centre(
    Rect.fromLTWH(
      _wheelRadius - _centreRadius,
      _wheelRadius - _centreRadius,
      MtWheel.centreButton,
      MtWheel.centreButton,
    ),
  );

  const _Button(this.bounds);

  final Rect bounds;

  static _Button? at(Offset fromCentre) {
    if (fromCentre.distance < _centreRadius) return centre;
    final point = fromCentre + _wheelCentre;
    for (final button in values) {
      if (button != centre && button.bounds.contains(point)) return button;
    }
    return null;
  }
}

class _WheelFace extends StatelessWidget {
  const _WheelFace({required this.theme, required this.rotation});

  final MtTheme theme;
  final double rotation;

  @override
  Widget build(BuildContext context) {
    final colors = theme.colors;
    final dark = theme.brightness == Brightness.dark;
    // CSS measures the angle clockwise from up. Project the corners of the
    // square onto that direction to match the handoff's gradient stops.
    const angle = MtWheel.gradientAngleDeg * math.pi / 180;
    final x = math.sin(angle);
    final y = -math.cos(angle);
    final span = x.abs() + y.abs();
    return DecoratedBox(
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        boxShadow: [
          dark
              ? const BoxShadow(
                  color: Color(0x73000000),
                  offset: Offset(0, 10),
                  blurRadius: 26,
                )
              : const BoxShadow(
                  color: Color(0x29141E2D),
                  offset: Offset(0, 8),
                  blurRadius: 18,
                ),
        ],
      ),
      child: ClipOval(
        child: CustomPaint(
          foregroundPainter: _InsetShading(dark),
          child: Transform.rotate(
            angle: rotation * math.pi / 180,
            child: DecoratedBox(
              decoration: BoxDecoration(
                gradient: LinearGradient(
                  begin: Alignment(-x * span, -y * span),
                  end: Alignment(x * span, y * span),
                  colors: [colors.wheelStart, colors.wheelMid, colors.wheelEnd],
                  stops: const [0, MtWheel.gradientMidStop, 1],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _InsetShading extends CustomPainter {
  const _InsetShading(this.dark);

  final bool dark;

  @override
  void paint(Canvas canvas, Size size) {
    final circle = Path()..addOval(Offset.zero & size);
    final top = Path.combine(
      PathOperation.difference,
      circle,
      circle.shift(const Offset(0, 1)),
    );
    final bottom = Path.combine(
      PathOperation.difference,
      circle,
      circle.shift(const Offset(0, -1)),
    );
    canvas.drawPath(
      top,
      Paint()..color = dark ? const Color(0x1CFFFFFF) : const Color(0xFFFFFFFF),
    );
    canvas.drawPath(
      bottom,
      Paint()..color = dark ? const Color(0x80000000) : const Color(0x2414171A),
    );
  }

  @override
  bool shouldRepaint(_InsetShading oldDelegate) => dark != oldDelegate.dark;
}
