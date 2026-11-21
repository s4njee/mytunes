import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mytunes_pod/click_wheel.dart';
import 'package:mytunes_pod/main.dart';
import 'package:mytunes_pod/pod_navigation.dart';
import 'package:mytunes_pod/src/rust/api/device.dart';
import 'package:mytunes_pod/src/rust/api/playback.dart';
import 'package:mytunes_pod/theme/mt_theme.dart';
import 'package:mytunes_pod/theme/tokens.g.dart';

void main() {
  testWidgets('Library uses live counts and the two selection styles', (
    tester,
  ) async {
    _portrait(tester);
    tester.platformDispatcher.platformBrightnessTestValue = Brightness.light;
    addTearDown(tester.platformDispatcher.clearPlatformBrightnessTestValue);
    await tester.pumpWidget(
      MyTunesPod(library: Future.value(_LibraryStub(songCount: 146))),
    );
    await tester.pumpAndSettle();

    expect(find.text('146 songs · 3 albums'), findsOneWidget);
    expect(_wheel(tester).onCentre, isNull);
    expect(tester.widget<Text>(find.text('Now Playing')).style?.color?.a, 0.35);
    expect(find.text('146'), findsOneWidget);
    expect(find.text('3'), findsOneWidget);
    expect(find.text('0'), findsOneWidget);
    _wheel(tester).onStep!(1);
    await tester.pump();

    final albums = find.text('Albums');
    final selected = find.ancestor(
      of: albums,
      matching: find.byWidgetPredicate(
        (widget) => widget is Semantics && widget.properties.selected == true,
      ),
    );
    expect(tester.getSize(selected).height, 54);
    for (final theme in [MtTheme.light, MtTheme.dark]) {
      tester.platformDispatcher.platformBrightnessTestValue = theme.brightness;
      await tester.pumpAndSettle();
      final decoration =
          tester
                  .widget<DecoratedBox>(
                    find
                        .descendant(
                          of: selected,
                          matching: find.byType(DecoratedBox),
                        )
                        .first,
                  )
                  .decoration
              as BoxDecoration;
      expect(decoration.color, theme.colors.selectedRowBg);
      expect(
        decoration.borderRadius,
        BorderRadius.circular(
          theme == MtTheme.light
              ? MtRadius.selectedRowLight
              : MtRadius.selectedRowDark,
        ),
      );
      expect(
        tester.widget<Text>(albums).style?.color,
        theme.colors.selectedRowText,
      );
    }
  });

  testWidgets('wheel clamps at both ends and MENU resets the selection', (
    tester,
  ) async {
    _portrait(tester);
    await tester.pumpWidget(const MyTunesPod());
    _wheel(tester).onStep!(-1);
    await tester.pump();
    expect(_navigation(tester).selection, 0);
    for (var index = 0; index < 7; index++) {
      _wheel(tester).onStep!(1);
    }
    await tester.pumpAndSettle();
    expect(_navigation(tester).selection, 4);
    await tester.tap(find.text('SELECT'));
    await tester.pumpAndSettle();
    expect(find.text('EQUALIZER'), findsOneWidget);
    await tester.tap(find.text('MENU'));
    await tester.pumpAndSettle();
    expect(find.text('LIBRARY'), findsOneWidget);
    expect(_navigation(tester).selection, 0);
  });

  testWidgets('Songs pages through the library and plays the selected track', (
    tester,
  ) async {
    _portrait(tester);
    final library = _LibraryStub(songCount: 146);
    await tester.pumpWidget(MyTunesPod(library: Future.value(library)));
    await tester.pumpAndSettle();

    _wheel(tester).onStep!(1);
    _wheel(tester).onStep!(1);
    await tester.pump();
    await tester.tap(find.text('SELECT'));
    await tester.pumpAndSettle();
    expect(find.text('SONGS'), findsOneWidget);
    expect(find.text('Song 1'), findsOneWidget);
    expect(library.pages.map((page) => page.offset), [0]);

    for (var index = 0; index < 75; index++) {
      _wheel(tester).onStep!(1);
    }
    await tester.pumpAndSettle();
    expect(find.text('Song 76').hitTestable(), findsOneWidget);
    expect(library.pages.map((page) => page.offset), [0, 64]);
    await tester.tap(find.text('PLAY'));
    await tester.pumpAndSettle();
    expect(library.loaded, ['75']);
    expect(library.played, isTrue);
    expect(find.text('NOW PLAYING'), findsOneWidget);

    await tester.tap(find.text('MENU'));
    await tester.pumpAndSettle();
    expect(tester.widget<Text>(find.text('Now Playing')).style?.color?.a, 1);
    await tester.tap(find.text('SELECT'));
    await tester.pumpAndSettle();
    expect(find.text('NOW PLAYING'), findsOneWidget);
  });
}

void _portrait(WidgetTester tester) {
  tester.view.devicePixelRatio = 1;
  tester.view.physicalSize = const Size(402, 874);
  addTearDown(tester.view.reset);
}

ClickWheel _wheel(WidgetTester tester) =>
    tester.widget(find.byType(ClickWheel));

PodNavigationState _navigation(WidgetTester tester) =>
    tester.state(find.byType(PodNavigation));

class _LibraryStub implements DeviceLibrary {
  _LibraryStub({required this.songCount});

  final int songCount;
  final pages = <LibraryPage>[];
  List<String> loaded = [];
  bool played = false;

  @override
  Future<LibraryCounts> libraryCounts() async =>
      LibraryCounts(songs: songCount, albums: 3, artists: 1, playlists: 0);

  @override
  Future<List<DeviceTrack>> songs({required LibraryPage page}) async {
    pages.add(page);
    return [
      for (
        var index = page.offset;
        index < songCount && index < page.offset + page.limit;
        index++
      )
        DeviceTrack(
          id: '$index',
          title: 'Song ${index + 1}',
          artist: 'Test Artist',
          durationMs: 65000,
        ),
    ];
  }

  @override
  Future<PlayerEventSubscription> subscribePlayer() async =>
      _PlayerSubscription();

  @override
  Future<void> loadQueue({
    required List<String> trackIds,
    required int startIndex,
  }) async {
    expect(startIndex, 0);
    loaded = trackIds;
  }

  @override
  Future<void> play() async => played = true;

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _PlayerSubscription implements PlayerEventSubscription {
  @override
  Future<Stream<PlaybackEvent>> stream() async => const Stream.empty();

  @override
  void dispose() {}

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}
