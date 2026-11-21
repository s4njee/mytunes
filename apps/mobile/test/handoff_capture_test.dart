import 'dart:async';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mytunes_pod/main.dart';
import 'package:mytunes_pod/pod_navigation.dart';
import 'package:mytunes_pod/src/pod_session.dart';
import 'package:mytunes_pod/src/rust/api/device.dart';
import 'package:mytunes_pod/src/rust/api/playback.dart';
import 'package:mytunes_pod/theme/tokens.g.dart';

void main() {
  _HandoffBinding();

  setUpAll(() async {
    await Future.wait([
      (FontLoader(MtFonts.sans)
            ..addFont(rootBundle.load('assets/fonts/IBMPlexSans-Regular.ttf'))
            ..addFont(rootBundle.load('assets/fonts/IBMPlexSans-Medium.ttf'))
            ..addFont(rootBundle.load('assets/fonts/IBMPlexSans-SemiBold.ttf')))
          .load(),
      (FontLoader(MtFonts.mono)
            ..addFont(rootBundle.load('assets/fonts/JetBrainsMono-Regular.ttf'))
            ..addFont(rootBundle.load('assets/fonts/JetBrainsMono-Medium.ttf')))
          .load(),
    ]);
  });

  const screens = {
    PodScreen.menu: ('menu', 'LIBRARY'),
    PodScreen.albums: ('albums', 'ALBUMS'),
    PodScreen.nowPlaying: ('now-playing', 'Long Way Around'),
    PodScreen.queue: ('queue', 'QUEUE'),
    PodScreen.equalizer: ('equalizer', 'EQUALIZER'),
  };
  final directory = Platform.environment['MYTUNES_RENDER_HANDOFF'];

  for (final theme in Brightness.values) {
    testWidgets('renders the five ${theme.name} handoff states', (
      tester,
    ) async {
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = const Size(402, 874);
      tester.view.viewPadding = const FakeViewPadding(top: 59, bottom: 34);
      tester.platformDispatcher.platformBrightnessTestValue = theme;
      addTearDown(tester.view.reset);
      addTearDown(tester.platformDispatcher.clearPlatformBrightnessTestValue);

      final library = _HandoffLibrary();
      final app = MyTunesPod(library: Future.value(library));
      const boundaryKey = ValueKey('handoff viewport');
      final viewport = find.byKey(boundaryKey);
      try {
        await tester.pumpWidget(RepaintBoundary(key: boundaryKey, child: app));
        await tester.pumpAndSettle();
        library.publishPlayingSample();
        await tester.pumpAndSettle();
        final navigation = tester.state<PodNavigationState>(
          find.byType(PodNavigation),
        );

        for (final entry in screens.entries) {
          final (name, expected) = entry.value;
          if (entry.key != PodScreen.menu) navigation.push(entry.key);
          await tester.pumpAndSettle();
          expect(find.text(expected), findsOneWidget);
          if (entry.key == PodScreen.nowPlaying) {
            expect(find.text('1:26'), findsOneWidget);
          }

          if (directory != null && directory.isNotEmpty) {
            final boundary = tester.renderObject<RenderRepaintBoundary>(
              viewport,
            );
            final image = boundary.toImageSync();
            try {
              await tester.runAsync(() async {
                await Directory(directory).create(recursive: true);
                final file = File('$directory/${theme.name}-$name.png');
                final data = await image.toByteData(
                  format: ui.ImageByteFormat.png,
                );
                await file.writeAsBytes(
                  data!.buffer.asUint8List(
                    data.offsetInBytes,
                    data.lengthInBytes,
                  ),
                );
              });
            } finally {
              image.dispose();
            }
          }
          if (entry.key != PodScreen.menu) navigation.menu();
          await tester.pumpAndSettle();
        }
      } finally {
        await tester.pumpWidget(const SizedBox.shrink());
        unawaited(library.events.close());
      }
    });
  }
}

class _HandoffBinding extends AutomatedTestWidgetsFlutterBinding {
  @override
  bool get disableShadows => false;
}

class _HandoffLibrary implements DeviceLibrary {
  // Placeholder library and 1:26 playing snapshot from the 1a and 2a prototypes.
  static const _albums = [
    ('Second Winter', 'Halvard Nyk', 1998, 9),
    ('Low Tide Radio', 'Mona Ellery', 2004, 5),
    ('Copper Lines', 'The Ovens', 1991, 11),
    ('Held Note', 'Ines Brahm', 2011, 8),
    ('Night Shift Tapes', 'Various', 1986, 14),
    ('Fieldwork', 'Aud Sørby', 2016, 7),
  ];
  static const _catalog = [
    ('Long Way Around', 254, 0),
    ('Second Winter', 198, 0),
    ('Harbour Light', 221, 1),
    ('Copper Lines', 302, 2),
    ('Held Note', 176, 3),
    ('Third Shift', 243, 4),
    ('Fieldwork II', 289, 5),
  ];

  final events = StreamController<PlaybackEvent>.broadcast();
  final _tracks = {
    for (final (title, seconds, album) in _catalog)
      title: DeviceTrack(
        id: title,
        title: title,
        artist: _albums[album].$2,
        album: _albums[album].$1,
        durationMs: seconds * 1000,
        codec: 'flac',
        bitDepth: 24,
        sampleRate: 96000,
      ),
  };

  void publishPlayingSample() {
    events.add(
      PlaybackEvent(
        state: PlaybackState.playing,
        positionMs: BigInt.from(86000),
        trackId: _tracks.keys.first,
        queueIndex: 0,
        pass: BigInt.one,
        playedMs: BigInt.from(86000),
        volumePercent: 62,
        ended: false,
        skipped: false,
      ),
    );
  }

  @override
  Future<String?> settingsGet({required String key}) async => PodSession(
    trackIds: _tracks.keys.toList(),
    albumYear: 1998,
    lastForegroundMs: DateTime.now().millisecondsSinceEpoch,
  ).encode();

  @override
  Future<LibraryCounts> libraryCounts() async =>
      const LibraryCounts(songs: 7, albums: 6, artists: 6, playlists: 0);

  @override
  Future<List<DeviceAlbum>> albums({required LibraryPage page}) async => [
    for (final (title, artist, year, trackCount)
        in _albums.skip(page.offset).take(page.limit))
      DeviceAlbum(
        id: AlbumId(
          title: title,
          artist: artist,
          year: year,
          compilation: artist == 'Various',
        ),
        trackCount: trackCount,
      ),
  ];

  @override
  Future<List<DeviceTrack>> tracksByIds({required List<String> ids}) async => [
    for (final id in ids) ?_tracks[id],
  ];

  @override
  Future<PlayerEventSubscription> subscribePlayer() async =>
      _HandoffSubscription(events.stream);

  @override
  Future<void> loadQueue({
    required List<String> trackIds,
    required int startIndex,
  }) async {}

  @override
  Future<void> setVolume({required double volume}) async {}

  @override
  Future<void> setEq({required List<int> gains}) async {}

  @override
  Future<void> setFlags({
    required bool gapless,
    required PlaybackSoundCheck soundCheck,
    required double volumeLimit,
  }) async {}

  @override
  Future<void> setShuffle({required PlaybackShuffle mode}) async {}

  @override
  Future<void> settingsSet({
    required String key,
    required String valueJson,
  }) async {}

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _HandoffSubscription implements PlayerEventSubscription {
  const _HandoffSubscription(this.events);

  final Stream<PlaybackEvent> events;

  @override
  Future<Stream<PlaybackEvent>> stream() async => events;

  @override
  void dispose() {}

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}
