import 'dart:math' as math;
import 'dart:ui' show Offset;

import 'theme/tokens.g.dart';

double wheelAngle(Offset fromCentre) =>
    math.atan2(fromCentre.dy, fromCentre.dx) * 180 / math.pi;

double shortestWheelDelta(double from, double to) {
  final delta = to - from;
  if (delta > 180) return delta - 360;
  if (delta < -180) return delta + 360;
  return delta;
}

/// Holds the fractional detent between samples from one ring drag.
class WheelDetents {
  WheelDetents(this._angle);

  double _angle;
  double _carry = 0;

  ({double delta, int steps}) moveTo(double angle) {
    final delta = shortestWheelDelta(_angle, angle);
    _angle = angle;
    _carry += delta;
    // atan2 can land a fraction of a degree short of an exact detent.
    final whole = (_carry.abs() / MtWheel.detentDegrees + 1e-12).floor();
    final steps = _carry < 0 ? -whole : whole;
    _carry -= steps * MtWheel.detentDegrees;
    return (delta: delta, steps: steps);
  }
}
