import 'package:flutter_test/flutter_test.dart';
import 'package:mytunes_pod/wheel_detents.dart';

void main() {
  test('clockwise drag crosses the angle seam without a jump', () {
    final detents = WheelDetents(170);

    expect(detents.moveTo(-175), (delta: 15.0, steps: 1));
    expect(detents.moveTo(-166), (delta: 9.0, steps: 1));
  });

  test('counterclockwise drag crosses the angle seam without a jump', () {
    final detents = WheelDetents(-170);

    expect(detents.moveTo(175), (delta: -15.0, steps: -1));
    expect(detents.moveTo(166), (delta: -9.0, steps: -1));
  });

  test('30 degrees emits two detents and carries six degrees', () {
    final detents = WheelDetents(0);

    expect(detents.moveTo(30), (delta: 30.0, steps: 2));
    expect(detents.moveTo(35), (delta: 5.0, steps: 0));
    expect(detents.moveTo(36), (delta: 1.0, steps: 1));
  });

  test('reversing direction unwinds the partial detent first', () {
    final detents = WheelDetents(0);

    expect(detents.moveTo(18), (delta: 18.0, steps: 1));
    expect(detents.moveTo(12), (delta: -6.0, steps: 0));
    expect(detents.moveTo(1), (delta: -11.0, steps: 0));
    expect(detents.moveTo(0), (delta: -1.0, steps: -1));
  });

  test('angle round-off does not swallow a complete detent', () {
    expect(WheelDetents(0).moveTo(11.999999999999998).steps, 1);
    expect(WheelDetents(0).moveTo(-11.999999999999998).steps, -1);
  });
}
