import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:mytunes_pod/src/wheel_scrub.dart';

void main() {
  testWidgets('coalesces during a slow seek and flushes the final target', (
    tester,
  ) async {
    final requests = <int>[];
    final complete = Completer<void>();
    final scrub = WheelScrub((target) async {
      requests.add(target);
      if (requests.length == 1) await complete.future;
    });
    addTearDown(scrub.dispose);

    scrub.step(1, position: 0, duration: 10000);
    await tester.pump(const Duration(milliseconds: 150));
    expect(requests, [4000]);
    scrub.step(1, position: 0, duration: 10000);
    scrub.step(1, position: 0, duration: 10000);
    scrub.release();
    expect(scrub.preview.value, 9999);
    expect(requests, [4000]);
    complete.complete();
    await tester.pump();
    expect(requests, [4000, 9999]);
    expect(scrub.preview.value, isNull);
  });

  testWidgets('cancelling discards pending seeks and resets the preview', (
    tester,
  ) async {
    final requests = <int>[];
    final scrub = WheelScrub((target) async => requests.add(target));
    addTearDown(scrub.dispose);
    scrub.step(-1, position: 1500, duration: 10000);
    expect(scrub.preview.value, 0);
    scrub.cancel();
    await tester.pump(const Duration(milliseconds: 150));
    expect(requests, isEmpty);
    expect(scrub.preview.value, isNull);
  });
}
