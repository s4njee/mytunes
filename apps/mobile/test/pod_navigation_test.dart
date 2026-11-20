import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart' show MaterialApp;
import 'package:mytunes_pod/pod_navigation.dart';

const _duration = Duration(milliseconds: 220);
const _menuButton = ValueKey('menu button');

void main() {
  testWidgets('forward and back slide the display for 220 ms', (tester) async {
    await tester.pumpWidget(_app());
    final navigation = tester.state<PodNavigationState>(
      find.byType(PodNavigation),
    );
    final menu = find.text('menu: 0');
    final albums = find.text('albums: 2');
    final button = find.byKey(_menuButton);
    final width = tester.getSize(menu).width;
    final deckPosition = tester.getCenter(button);

    navigation.push(PodScreen.albums, selection: 2);
    await tester.pump();
    expect(tester.getTopLeft(albums).dx, width);
    await tester.pump(const Duration(milliseconds: 110));
    final progress = Curves.easeOut.transform(0.5);
    expect(tester.getTopLeft(albums).dx, closeTo(width * (1 - progress), 0.01));
    expect(tester.getTopLeft(menu).dx, closeTo(-width * progress, 0.01));
    await tester.pump(const Duration(milliseconds: 109));
    expect(menu, findsOneWidget);
    await tester.pump(const Duration(milliseconds: 1));
    expect(tester.getTopLeft(albums).dx, 0);
    await tester.pumpAndSettle();
    expect(menu, findsNothing);
    expect(tester.getCenter(button), deckPosition);

    await tester.tap(button);
    await tester.pump();
    expect(tester.getTopLeft(menu).dx, -width);
    await tester.pump(const Duration(milliseconds: 110));
    expect(tester.getTopLeft(menu).dx, closeTo(-width * (1 - progress), 0.01));
    expect(tester.getTopLeft(albums).dx, closeTo(width * progress, 0.01));
    await tester.pump(const Duration(milliseconds: 110));
    expect(tester.getTopLeft(menu).dx, 0);
    await tester.pumpAndSettle();
    expect(albums, findsNothing);
    expect(tester.getCenter(button), deckPosition);
  });

  testWidgets('MENU resets the root after multiple pushes', (tester) async {
    await tester.pumpWidget(_app());
    final navigation = tester.state<PodNavigationState>(
      find.byType(PodNavigation),
    );
    final button = find.byKey(_menuButton);

    navigation.select(3);
    await tester.tap(button);
    await tester.pump();
    expect(find.text('menu: 3'), findsOneWidget);

    navigation.push(PodScreen.queue, selection: 4);
    await _finishSlide(tester);
    navigation.push(PodScreen.nowPlaying);
    await _finishSlide(tester);
    expect(find.text('nowPlaying: 0'), findsOneWidget);

    await tester.tap(button);
    await _finishSlide(tester);
    expect(find.text('menu: 0'), findsOneWidget);
    expect(find.text('queue: 4'), findsNothing);
    expect(find.text('nowPlaying: 0'), findsNothing);
  });

  testWidgets('MENU can reverse an in-flight push', (tester) async {
    await tester.pumpWidget(_app());
    final navigation = tester.state<PodNavigationState>(
      find.byType(PodNavigation),
    );
    navigation.push(PodScreen.albums);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 80));
    final menu = find.text('menu: 0');
    final albums = find.text('albums: 0');
    final menuBefore = tester.getTopLeft(menu);
    final albumsBefore = tester.getTopLeft(albums);

    await tester.tap(find.byKey(_menuButton));
    await tester.pump();
    expect(tester.getTopLeft(menu), menuBefore);
    expect(tester.getTopLeft(albums), albumsBefore);
    await _finishSlide(tester);
    expect(tester.getTopLeft(menu).dx, 0);
    expect(albums, findsNothing);
  });

  testWidgets('Android system back behaves like MENU', (tester) async {
    await tester.pumpWidget(_app());
    final navigation = tester.state<PodNavigationState>(
      find.byType(PodNavigation),
    );
    navigation.select(4);
    navigation.push(PodScreen.equalizer);
    await _finishSlide(tester);

    await tester.binding.handlePopRoute();
    await _finishSlide(tester);
    expect(find.text('menu: 0'), findsOneWidget);
    navigation.select(2);
    await tester.pump();
    await tester.binding.handlePopRoute();
    await tester.pump();
    expect(find.text('menu: 2'), findsOneWidget);
  }, variant: const TargetPlatformVariant({TargetPlatform.android}));

  testWidgets('iOS edge swipe leaves the wheel screen alone', (tester) async {
    await tester.pumpWidget(
      MaterialApp(
        home: const Text('Previous route'),
        routes: {'/wheel': (_) => _wheel()},
      ),
    );
    tester.state<NavigatorState>(find.byType(Navigator)).pushNamed('/wheel');
    await tester.pumpAndSettle();
    final navigation = tester.state<PodNavigationState>(
      find.byType(PodNavigation),
    );
    expect(ModalRoute.of(navigation.context)?.popGestureEnabled, isFalse);
    navigation.push(PodScreen.albums);
    await _finishSlide(tester);

    await tester.dragFrom(const Offset(1, 200), const Offset(390, 0));
    await tester.pumpAndSettle();
    expect(find.text('albums: 0'), findsOneWidget);
  }, variant: const TargetPlatformVariant({TargetPlatform.iOS}));
}

Widget _app() => MaterialApp(home: _wheel());

Widget _wheel() => PodNavigation(
  display: (_, screen, selection) => Text('${screen.name}: $selection'),
  deck: (_, navigation) => GestureDetector(
    key: _menuButton,
    onTap: navigation.menu,
    child: const Text('MENU'),
  ),
);

Future<void> _finishSlide(WidgetTester tester) =>
    tester.pumpAndSettle(_duration);
