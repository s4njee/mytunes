import 'dart:math' as math;
import 'dart:ui' show PointerDeviceKind;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart' show MaterialApp;
import 'package:mytunes_pod/click_wheel.dart';
import 'package:mytunes_pod/main.dart';
import 'package:mytunes_pod/pod_navigation.dart';
import 'package:mytunes_pod/theme/mt_theme.dart';
import 'package:mytunes_pod/theme/tokens.g.dart';

void main() {
  testWidgets('wheel hit areas use the handoff geometry', (tester) async {
    final taps = <String>[];
    final steps = <int>[];
    final haptics = _captureHaptics(tester);
    await tester.pumpWidget(
      _app(
        ClickWheel(
          screen: PodScreen.menu,
          onMenu: () => taps.add('menu'),
          onPrevious: () => taps.add('previous'),
          onNext: () => taps.add('next'),
          onPlayPause: () => taps.add('playPause'),
          onCentre: () => taps.add('centre'),
          onStep: steps.add,
        ),
      ),
    );
    final surface = _surface();
    final topLeft = tester.getTopLeft(surface);
    expect(tester.getSize(surface), const Size(252, 252));
    expect(tester.getTopLeft(find.text('MENU')).dy - topLeft.dy, 16);
    expect(
      tester.getSize(
        find.ancestor(
          of: find.text('SELECT'),
          matching: find.byWidgetPredicate(
            (widget) =>
                widget is Positioned && widget.width == MtWheel.centreButton,
          ),
        ),
      ),
      const Size(104, 104),
    );
    for (final point in [
      const Offset(77, 1),
      const Offset(1, 99),
      const Offset(251, 153),
      const Offset(175, 251),
      const Offset(126, 75),
    ]) {
      await tester.tapAt(topLeft + point);
    }
    expect(taps, ['menu', 'previous', 'next', 'playPause', 'centre']);
    expect(haptics, List.filled(5, 'HapticFeedbackType.lightImpact'));
    expect(steps, isEmpty);
  });

  testWidgets('a captured ring drag emits steps and rotates the face', (
    tester,
  ) async {
    final steps = <int>[];
    final haptics = _captureHaptics(tester);
    var presses = 0;
    await tester.pumpWidget(
      _app(
        ClickWheel(
          screen: PodScreen.menu,
          onStep: steps.add,
          onNext: () => presses++,
        ),
      ),
    );
    final centre = tester.getCenter(_surface());
    final hint = find.text('DRAG RING TO BROWSE');
    final labelPosition = tester.getCenter(find.text('MENU'));
    final face = find.descendant(
      of: find.byType(ClickWheel),
      matching: find.byType(Transform),
    );
    expect(
      tester.widget<Text>(hint).style?.color,
      MtTheme.light.colors.textHint,
    );
    final drag = await tester.startGesture(_point(centre, -48));
    await tester.pump();
    expect(tester.widget<Text>(hint).style?.color, MtTheme.light.colors.accent);

    await drag.moveTo(_point(centre, -18));
    await tester.pump();
    expect(steps, [1, 1]);
    expect(
      tester.widget<Transform>(face).transform.entry(1, 0),
      closeTo(0.5, 1e-6),
    );
    expect(tester.getCenter(find.text('MENU')), labelPosition);

    // The pointer stays captured across a button and beyond the outside radius.
    await drag.moveTo(_point(centre, 2, radius: 142));
    await drag.up();
    await tester.pumpAndSettle(const Duration(milliseconds: 250));
    expect(steps, [1, 1, 1, 1]);
    expect(presses, 0);
    expect(haptics, List.filled(4, 'HapticFeedbackType.selectionClick'));
    expect(
      tester.widget<Text>(hint).style?.color,
      MtTheme.light.colors.textHint,
    );

    final nextDrag = await tester.startGesture(_point(centre, 46));
    await nextDrag.moveTo(_point(centre, 53));
    await nextDrag.cancel();
    expect(steps, [1, 1, 1, 1]);
  });

  testWidgets('button drags and starts outside the ring emit no steps', (
    tester,
  ) async {
    final steps = <int>[];
    var presses = 0;
    await tester.pumpWidget(
      _app(
        ClickWheel(
          screen: PodScreen.menu,
          onStep: steps.add,
          onNext: () => presses++,
          onCentre: () => presses++,
        ),
      ),
    );
    final centre = tester.getCenter(_surface());
    final next = await tester.startGesture(_point(centre, 0));
    await next.moveTo(_point(centre, 8));
    await next.moveTo(_point(centre, 0));
    await next.up();
    final centreDrag = await tester.startGesture(centre);
    await centreDrag.moveTo(_point(centre, 45));
    await centreDrag.up();
    final corner = await tester.startGesture(
      tester.getTopLeft(_surface()) + const Offset(1, 1),
    );
    await corner.moveTo(_point(centre, -45));
    await corner.moveTo(_point(centre, -20));
    await corner.up();
    expect(steps, isEmpty);
    expect(presses, 0);
  });

  testWidgets('labels follow screen and playback context', (tester) async {
    for (final (screen, playing, centre, hint) in [
      (PodScreen.menu, false, 'SELECT', 'BROWSE'),
      (PodScreen.albums, false, 'PLAY', 'BROWSE'),
      (PodScreen.songs, false, 'PLAY', 'BROWSE'),
      (PodScreen.queue, false, 'PLAY', 'BROWSE'),
      (PodScreen.nowPlaying, false, 'PLAY', 'SCRUB'),
      (PodScreen.nowPlaying, true, 'PAUSE', 'SCRUB'),
      (PodScreen.equalizer, false, 'BAND 1k', 'SET GAIN'),
    ]) {
      await tester.pumpWidget(
        _app(ClickWheel(screen: screen, playing: playing, eqBand: '1k')),
      );
      expect(find.text(centre), findsOneWidget);
      expect(find.text('DRAG RING TO $hint'), findsOneWidget);
      expect(find.text('▶ ❙❙'), findsOneWidget);
    }
  });

  testWidgets('mouse and stylus drags use the same detents', (tester) async {
    final steps = <int>[];
    await tester.pumpWidget(
      _app(ClickWheel(screen: PodScreen.menu, onStep: steps.add)),
    );
    final centre = tester.getCenter(_surface());
    for (final kind in [PointerDeviceKind.mouse, PointerDeviceKind.stylus]) {
      final drag = await tester.startGesture(_point(centre, -48), kind: kind);
      await drag.moveTo(_point(centre, -35));
      await drag.up();
    }
    expect(steps, [1, 1]);
  });

  testWidgets('the app wheel MENU returns the navigation stack to root', (
    tester,
  ) async {
    await tester.pumpWidget(const MyTunesPod());
    final navigation = tester.state<PodNavigationState>(
      find.byType(PodNavigation),
    );
    navigation.select(3);
    navigation.push(PodScreen.albums);
    await tester.pumpAndSettle();
    expect(find.text('PLAY'), findsOneWidget);
    await tester.tap(find.text('MENU'));
    await tester.pumpAndSettle();
    expect(navigation.screen, PodScreen.menu);
    expect(navigation.selection, 0);
    expect(find.text('SELECT'), findsOneWidget);
  });
}

Widget _app(ClickWheel wheel) => MaterialApp(
  theme: MtTheme.light.materialTheme,
  home: Center(child: wheel),
);

Finder _surface() => find.descendant(
  of: find.byType(ClickWheel),
  matching: find.byType(Listener),
);

Offset _point(Offset centre, double degrees, {double radius = 112}) {
  final angle = degrees * math.pi / 180;
  return centre + Offset(math.cos(angle), math.sin(angle)) * radius;
}

List<String> _captureHaptics(WidgetTester tester) {
  final events = <String>[];
  final messenger = tester.binding.defaultBinaryMessenger;
  messenger.setMockMethodCallHandler(SystemChannels.platform, (call) async {
    if (call.method == 'HapticFeedback.vibrate') {
      events.add(call.arguments as String);
    }
    return null;
  });
  addTearDown(
    () => messenger.setMockMethodCallHandler(SystemChannels.platform, null),
  );
  return events;
}
