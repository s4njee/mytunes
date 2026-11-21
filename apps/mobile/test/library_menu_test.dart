import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mytunes_pod/albums_grid.dart';
import 'package:mytunes_pod/click_wheel.dart';
import 'package:mytunes_pod/main.dart';
import 'package:mytunes_pod/pod_navigation.dart';
import 'package:mytunes_pod/src/equalizer_settings.dart';
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
  testWidgets('Albums browses 500 tiles and reveals the wheel selection', (
    tester,
  ) async {
    _portrait(tester);
    final library = _LibraryStub(songCount: 500, albumCount: 500);
    await tester.pumpWidget(MyTunesPod(library: Future.value(library)));
    await tester.pumpAndSettle();
    _wheel(tester).onStep!(1);
    await tester.pump();
    await tester.tap(find.text('SELECT'));
    await tester.pumpAndSettle();

    expect(find.byType(AlbumsGrid), findsOneWidget);
    expect(find.text('500 albums'), findsOneWidget);
    expect(library.albumPages.map((page) => page.offset), [0]);
    expect(library.artworkRequests, [('cover-0', ThumbnailSize.large)]);
    final first = tester.getTopLeft(find.text('Album 1'));
    final second = tester.getTopLeft(find.text('Album 2'));
    expect(second.dx - first.dx, 172 + MtSpace.gridGap);
    expect(second.dy, first.dy);

    for (var index = 0; index < 501; index++) {
      _wheel(tester).onStep!(1);
    }
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 120));
    await tester.pumpAndSettle();
    expect(_navigation(tester).selection, 499);
    expect(find.text('Album 500').hitTestable(), findsOneWidget);
    expect(library.albumPages.last.offset, 448);
    expect(library.albumPages.length, lessThan(8));
  });

  testWidgets('SELECT plays an album starting with its first ordered track', (
    tester,
  ) async {
    _portrait(tester);
    final library = _LibraryStub(songCount: 3);
    await tester.pumpWidget(MyTunesPod(library: Future.value(library)));
    await tester.pumpAndSettle();
    _wheel(tester).onStep!(1);
    await tester.pump();
    await tester.tap(find.text('SELECT'));
    await tester.pumpAndSettle();
    _wheel(tester).onStep!(1);
    await tester.pump();
    await tester.tap(find.text('PLAY'));
    await tester.pumpAndSettle();
    expect(library.playedAlbum?.title, 'Album 2');
    expect(library.loaded, ['disc-1-track-2', 'disc-2-track-1']);
    expect(library.startIndex, 0);
    expect(library.played, isTrue);
    expect(find.text('ALBUM 2'), findsWidgets);
    expect(find.text('First track'), findsOneWidget);
    expect(_navigation(tester).screen, PodScreen.nowPlaying);
  });

  testWidgets('Now Playing renders snapshots and controls the Rust bridge', (
    tester,
  ) async {
    _portrait(tester);
    final library = _LibraryStub(songCount: 3);
    await tester.pumpWidget(MyTunesPod(library: Future.value(library)));
    await tester.pumpAndSettle();
    _wheel(tester).onStep!(1);
    await tester.pump();
    await tester.tap(find.text('SELECT'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('PLAY'));
    await tester.pumpAndSettle();
    expect(library.volume, 0.62);
    library.emit(index: 0, position: 15000);
    await tester.pumpAndSettle();

    expect(find.text('First track'), findsOneWidget);
    expect(find.text('FLAC · 24/96'), findsOneWidget);
    expect(find.text('0:15'), findsOneWidget);
    expect(find.text('-1:45'), findsOneWidget);
    expect(find.text('VOL'), findsOneWidget);
    expect(find.text('PAUSE'), findsOneWidget);

    _wheel(tester).onStep!(1);
    _wheel(tester).onStep!(1);
    await tester.pump(const Duration(milliseconds: 149));
    expect(find.text('0:23'), findsOneWidget);
    expect(library.seeks, isEmpty);
    await tester.pump(const Duration(milliseconds: 1));
    expect(library.seeks, [23000]);

    _wheel(tester).onStep!(-1);
    _wheel(tester).onDragEnd!();
    await tester.pump();
    expect(library.seeks, [23000, 19000]);

    await tester.tap(find.text('PAUSE'));
    await tester.tap(find.text('◀◀'));
    await tester.tap(find.text('▶▶'));
    await tester.tap(find.text('▶ ❙❙'));
    await tester.pumpAndSettle();
    expect(library.transport, ['toggle', 'previous', 'next', 'toggle']);

    _wheel(tester).onStep!(1);
    library.emit(index: 1, position: 0);
    await tester.pumpAndSettle();
    await tester.pump(const Duration(milliseconds: 151));
    expect(find.text('Second track'), findsOneWidget);
    expect(find.text('AAC · 256'), findsOneWidget);
    expect(library.seeks, [23000, 19000]);
  });

  testWidgets('Queue opens on the current track and plays another entry', (
    tester,
  ) async {
    _portrait(tester);
    tester.platformDispatcher.platformBrightnessTestValue = Brightness.light;
    addTearDown(tester.platformDispatcher.clearPlatformBrightnessTestValue);
    final library = _LibraryStub(songCount: 3);
    await tester.pumpWidget(MyTunesPod(library: Future.value(library)));
    await tester.pumpAndSettle();
    _wheel(tester).onStep!(1);
    await tester.pump();
    await tester.tap(find.text('SELECT'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('PLAY'));
    await tester.pumpAndSettle();
    library.emit(index: 1, position: 5000);
    await tester.pumpAndSettle();

    await tester.tap(find.text('MENU'));
    await tester.pumpAndSettle();
    for (var index = 0; index < 3; index++) {
      _wheel(tester).onStep!(1);
    }
    await tester.pump();
    await tester.tap(find.text('SELECT'));
    await tester.pumpAndSettle();
    expect(find.text('QUEUE'), findsOneWidget);
    expect(find.text('2 tracks'), findsOneWidget);
    expect(_navigation(tester).selection, 1);
    Color? colorOf(String text) =>
        tester.widget<Text>(find.text(text)).style?.color;
    expect(colorOf('Second track'), MtTheme.light.colors.selectedRowText);
    expect(colorOf('02'), MtTheme.light.colors.selectedRowText);

    _wheel(tester).onStep!(-1);
    await tester.pumpAndSettle();
    expect(colorOf('Second track'), MtTheme.light.colors.accent);
    expect(colorOf('02'), MtTheme.light.colors.accent);
    tester.platformDispatcher.platformBrightnessTestValue = Brightness.dark;
    await tester.pumpAndSettle();
    expect(colorOf('Second track'), MtTheme.dark.colors.accent);
    expect(colorOf('02'), MtTheme.dark.colors.accent);

    await tester.tap(find.text('PLAY'));
    await tester.pumpAndSettle();
    expect(library.startIndex, 0);
    expect(library.loaded, ['disc-1-track-2', 'disc-2-track-1']);
    expect(find.text('First track'), findsOneWidget);
    expect(_navigation(tester).screen, PodScreen.nowPlaying);
  });

  testWidgets('Equalizer changes live DSP and persists the wheel settings', (
    tester,
  ) async {
    _portrait(tester);
    final library = _LibraryStub(songCount: 3);
    await tester.pumpWidget(MyTunesPod(library: Future.value(library)));
    await tester.pumpAndSettle();
    _wheel(tester).onStep!(1);
    await tester.pump();
    await tester.tap(find.text('SELECT'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('PLAY'));
    await tester.pumpAndSettle();
    expect(library.gains, [3, 1, -2, 4, 2]);
    expect(library.shuffle, PlaybackShuffle.albums);
    await tester.tap(find.text('MENU'));
    await tester.pumpAndSettle();
    for (var i = 0; i < 4; i++) {
      _wheel(tester).onStep!(1);
    }
    await tester.pump();
    await tester.tap(find.text('SELECT'));
    await tester.pumpAndSettle();
    expect(find.text('BAND 60'), findsOneWidget);
    for (var i = 0; i < 5; i++) {
      _wheel(tester).onStep!(1);
    }
    await tester.pumpAndSettle();
    expect(library.gains, [6, 1, -2, 4, 2]);
    for (var i = 0; i < 5; i++) {
      _wheel(tester).onCentre!();
    }
    await tester.pumpAndSettle();
    expect(find.text('TOGGLE'), findsOneWidget);
    _wheel(tester).onCentre!();
    _wheel(tester).onStep!(1);
    _wheel(tester).onCentre!();
    _wheel(tester).onStep!(1);
    _wheel(tester).onCentre!();
    await tester.pumpAndSettle();
    expect(library.gapless, isFalse);
    expect(library.volumeCap, 0.5);
    expect(library.shuffle, PlaybackShuffle.off);
    final saved = EqualizerSettings.decode(
      library.settings[EqualizerSettings.storageKey],
    );
    expect(saved.gains, [6, 1, -2, 4, 2]);
    expect(saved.volumeLimited, isTrue);
    expect(saved.gapless, isFalse);
    expect(saved.shuffleAlbums, isFalse);
    for (var i = 0; i < 3; i++) {
      _wheel(tester).onStep!(-1);
    }
    await tester.pumpAndSettle();
    expect(find.text('BAND 14k'), findsOneWidget);
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
  _LibraryStub({required this.songCount, this.albumCount = 3});

  final int songCount;
  final int albumCount;
  final pages = <LibraryPage>[];
  final albumPages = <LibraryPage>[];
  final artworkRequests = <(String, ThumbnailSize)>[];
  final _events = StreamController<PlaybackEvent>.broadcast();
  final seeks = <int>[];
  final transport = <String>[];
  final settings = <String, String>{};
  List<int>? gains;
  bool? gapless;
  double? volumeCap;
  PlaybackShuffle? shuffle;
  double? volume;
  int? startIndex;
  AlbumId? playedAlbum;
  List<String> loaded = [];
  bool played = false;

  @override
  Future<LibraryCounts> libraryCounts() async => LibraryCounts(
    songs: songCount,
    albums: albumCount,
    artists: 1,
    playlists: 0,
  );

  @override
  Future<List<DeviceAlbum>> albums({required LibraryPage page}) async {
    albumPages.add(page);
    return [
      for (
        var index = page.offset;
        index < albumCount && index < page.offset + page.limit;
        index++
      )
        DeviceAlbum(
          id: AlbumId(
            title: 'Album ${index + 1}',
            artist: 'Test Artist',
            compilation: false,
          ),
          trackCount: 2,
          artworkId: index == 0 ? 'cover-0' : null,
        ),
    ];
  }

  @override
  Future<List<DeviceTrack>> albumTracks({required AlbumId id}) async {
    playedAlbum = id;
    return [
      DeviceTrack(
        id: 'disc-1-track-2',
        title: 'First track',
        album: id.title,
        codec: 'flac',
        bitDepth: 24,
        sampleRate: 96000,
        durationMs: 120000,
        discNumber: 1,
        trackNumber: 2,
      ),
      DeviceTrack(
        id: 'disc-2-track-1',
        title: 'Second track',
        album: id.title,
        codec: 'aac',
        bitrate: 256,
        durationMs: 60000,
        discNumber: 2,
        trackNumber: 1,
      ),
    ];
  }

  @override
  Future<String?> artworkPath({
    required String id,
    required ThumbnailSize size,
  }) async {
    artworkRequests.add((id, size));
    return null;
  }

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
      _PlayerSubscription(_events.stream);

  @override
  Future<void> loadQueue({
    required List<String> trackIds,
    required int startIndex,
  }) async {
    this.startIndex = startIndex;
    loaded = trackIds;
  }

  @override
  Future<void> play() async => played = true;

  @override
  Future<void> setVolume({required double volume}) async =>
      this.volume = volume;

  @override
  Future<void> setEq({required List<int> gains}) async => this.gains = gains;

  @override
  Future<void> setFlags({
    required bool gapless,
    required PlaybackSoundCheck soundCheck,
    required double volumeLimit,
  }) async {
    this.gapless = gapless;
    volumeCap = volumeLimit;
  }

  @override
  Future<void> setShuffle({required PlaybackShuffle mode}) async =>
      shuffle = mode;

  @override
  Future<String?> settingsGet({required String key}) async => settings[key];

  @override
  Future<void> settingsSet({
    required String key,
    required String valueJson,
  }) async => settings[key] = valueJson;

  @override
  Future<void> seek({required BigInt positionMs}) async =>
      seeks.add(positionMs.toInt());

  @override
  Future<void> toggle() async => transport.add('toggle');

  @override
  Future<void> prev() async => transport.add('previous');

  @override
  Future<void> next() async => transport.add('next');

  void emit({required int index, required int position}) => _events.add(
    PlaybackEvent(
      state: PlaybackState.playing,
      positionMs: BigInt.from(position),
      trackId: loaded[index],
      queueIndex: index,
      pass: BigInt.from(index + 1),
      playedMs: BigInt.from(position),
      volumePercent: 62,
      ended: false,
      skipped: false,
    ),
  );

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _PlayerSubscription implements PlayerEventSubscription {
  _PlayerSubscription(this._events);

  final Stream<PlaybackEvent> _events;

  @override
  Future<Stream<PlaybackEvent>> stream() async => _events;

  @override
  void dispose() {}

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}
