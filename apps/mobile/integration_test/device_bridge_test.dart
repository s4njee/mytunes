import 'dart:async';
import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:mytunes_pod/src/player_events.dart';
import 'package:mytunes_pod/src/rust/api/device.dart';
import 'package:mytunes_pod/src/rust/api/playback.dart';
import 'package:mytunes_pod/src/rust/frb_generated.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async => RustLib.init());

  test('reads the device library and persists settings and events', () async {
    final folder = await Directory.systemTemp.createTemp('mytunes-library');
    addTearDown(() => folder.delete(recursive: true));
    final path = '${folder.path}/device.sqlite';
    final library = await openDeviceDb(path: path);
    const page = LibraryPage(offset: 0, limit: 20);
    try {
      final counts = await library.libraryCounts();
      expect(
        [counts.songs, counts.albums, counts.artists, counts.playlists],
        [0, 0, 0, 0],
      );
      expect(await library.albums(page: page), isEmpty);
      expect(
        await library.albumTracks(
          id: const AlbumId(title: 'Missing', compilation: false),
        ),
        isEmpty,
      );
      expect(await library.artists(), isEmpty);
      expect(await library.songs(page: page), isEmpty);
      expect(await library.playlists(), isEmpty);
      expect(await library.playlistTracks(id: 'missing'), isEmpty);
      expect(
        await library.artworkPath(id: 'missing', size: ThumbnailSize.large),
        isNull,
      );
      await expectLater(
        library.songs(page: const LibraryPage(offset: 0, limit: 201)),
        throwsA(_bridgeError('at most 200 items')),
      );

      expect(await library.settingsGet(key: 'theme'), isNull);
      await library.settingsSet(key: 'theme', valueJson: '"dark"');
      final event = LibraryEvent(
        id: 'ac4916e8-1350-4eba-8f79-083eaad51c4d',
        timestampMs: 1000,
        kind: LibraryEventKind.play,
        payloadJson: '{"track_id":"track"}',
      );
      await library.recordEvent(event: event);
      await expectLater(
        library.recordEvent(event: event),
        throwsA(isA<AnyhowException>()),
      );
    } finally {
      await library.close();
    }
    final reopened = await openDeviceDb(path: path);
    try {
      expect(await reopened.settingsGet(key: 'theme'), '"dark"');
    } finally {
      await reopened.close();
    }
  });

  test('dispatches player commands and delivers transport events', () async {
    final folder = await Directory.systemTemp.createTemp('mytunes-player');
    addTearDown(() => folder.delete(recursive: true));
    final library = await openDeviceDb(path: '${folder.path}/device.sqlite');
    final events = StreamIterator(library.playerEvents());
    try {
      expect(await events.moveNext(), isTrue);
      expect(events.current.state, PlaybackState.empty);
      await library.setVolume(volume: 0.65);
      expect(await events.moveNext(), isTrue);
      expect(events.current.volumePercent, 65);
      await events.cancel();

      await library.loadQueue(trackIds: [], startIndex: 0);
      await expectLater(
        library.loadQueue(trackIds: ['missing'], startIndex: 0),
        throwsA(_bridgeError('no longer in the device library')),
      );
      await library.pause();
      for (final command in [
        library.play,
        library.toggle,
        () => library.seek(positionMs: BigInt.zero),
        library.next,
        library.prev,
      ]) {
        await expectLater(command(), throwsA(_bridgeError('queue is empty')));
      }
      await library.setEq(gains: [3, 1, -2, 4, 2]);
      await library.setFlags(
        gapless: true,
        soundCheck: PlaybackSoundCheck.off,
        volumeLimit: 0.8,
      );
      await library.setShuffle(mode: PlaybackShuffle.albums);
      await library.setRepeat(mode: PlaybackRepeat.all);
      await library.playClick();

      final latest = await library.playerEvents().first;
      expect(latest.state, PlaybackState.empty);
      expect(latest.volumePercent, 65);
    } finally {
      await events.cancel();
      await library.close();
    }
  });
}

Matcher _bridgeError(String text) => isA<AnyhowException>().having(
  (error) => error.message,
  'message',
  contains(text),
);
