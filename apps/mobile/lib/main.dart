import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:material_ui/material_ui.dart';
import 'package:path_provider/path_provider.dart';

import 'albums_grid.dart';
import 'click_wheel.dart';
import 'dev_import.dart';
import 'library_menu.dart';
import 'pod_navigation.dart';
import 'pod_shell.dart';
import 'src/library_pages.dart';
import 'src/player_events.dart';
import 'src/rust/api/device.dart';
import 'src/rust/api/playback.dart';
import 'src/rust/frb_generated.dart';
import 'theme/font_licences.dart';
import 'theme/mt_theme.dart';
import 'theme/tokens.g.dart';
import 'track_list.dart';
import 'wheel_list.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await SystemChrome.setPreferredOrientations([DeviceOrientation.portraitUp]);
  await SystemChrome.setEnabledSystemUIMode(SystemUiMode.edgeToEdge);
  registerFontLicences();
  await RustLib.init();
  runApp(MyTunesPod(library: _openLibrary()));
}

Future<DeviceLibrary> _openLibrary() async {
  final directory = await getApplicationSupportDirectory();
  return openDeviceDb(
    path: directory.uri.resolve('device.sqlite').toFilePath(),
  );
}

/// MyTunes Pod follows the platform's light or dark appearance.
class MyTunesPod extends StatelessWidget {
  const MyTunesPod({super.key, this.library});

  final Future<DeviceLibrary>? library;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'MyTunes Pod',
      debugShowCheckedModeBanner: false,
      theme: MtTheme.light.materialTheme,
      darkTheme: MtTheme.dark.materialTheme,
      themeAnimationDuration: Duration.zero,
      builder: (context, child) => AnnotatedRegion<SystemUiOverlayStyle>(
        value: MtTheme.of(context).systemUi,
        child: child!,
      ),
      routes: {
        '/licences': (context) =>
            const LicensePage(applicationName: 'MyTunes Pod'),
      },
      home: _LibraryHome(library: library),
    );
  }
}

class _LibraryHome extends StatefulWidget {
  const _LibraryHome({required this.library});

  final Future<DeviceLibrary>? library;

  @override
  State<_LibraryHome> createState() => _LibraryHomeState();
}

class _LibraryHomeState extends State<_LibraryHome> {
  late Future<LibraryCounts>? _counts = widget.library?.then(
    (library) => library.libraryCounts(),
  );
  late final _songs = LibraryPages<DeviceTrack>(
    (page) async => (await widget.library!).songs(page: page),
  );
  late final _albums = LibraryPages<DeviceAlbum>(
    (page) async => (await widget.library!).albums(page: page),
  );
  bool _importing = false;
  String? _notice;
  List<DeviceTrack> _queue = const [];
  int? _queueIndex;
  StreamSubscription<PlaybackEvent>? _playerEvents;
  bool _hasPlayed = false;
  bool _playing = false;
  bool _startingPlayback = false;
  PodScreen? _playbackError;

  Future<void> _importTracks(Future<DeviceLibrary> database) async {
    if (_importing) return;
    setState(() {
      _importing = true;
      _notice = null;
    });
    try {
      final library = await database;
      final notice = await importPickedTracks(library);
      if (notice != null && mounted) {
        setState(() {
          _songs.clear();
          _albums.clear();
          _counts = library.libraryCounts();
          _notice = notice;
        });
      }
    } catch (_) {
      if (mounted) setState(() => _notice = 'Could not import audio.');
    } finally {
      if (mounted) setState(() => _importing = false);
    }
  }

  Future<void> _playSelection(PodNavigationState navigation) async {
    final database = widget.library;
    if (database == null || _startingPlayback) return;
    _startingPlayback = true;
    final origin = navigation.screen;
    final selection = navigation.selection;
    try {
      final library = await database;
      final tracks = switch (origin) {
        PodScreen.albums => await _albumTracks(library, selection),
        PodScreen.songs => [?await _songs.itemAt(selection)],
        _ => const <DeviceTrack>[],
      };
      if (tracks.isEmpty ||
          !mounted ||
          !navigation.mounted ||
          navigation.screen != origin) {
        return;
      }
      _playerEvents ??= library.playerEvents().listen((event) {
        final playing = event.state == PlaybackState.playing;
        if (mounted &&
            (playing != _playing || event.queueIndex != _queueIndex)) {
          setState(() {
            _playing = playing;
            _queueIndex = event.queueIndex;
          });
        }
      }, onError: (Object _) {});
      await library.loadQueue(
        trackIds: tracks.map((track) => track.id).toList(growable: false),
        startIndex: 0,
      );
      if (!mounted) return;
      setState(() {
        _queue = tracks;
        _queueIndex = 0;
      });
      await library.play();
      if (!mounted) return;
      setState(() {
        _hasPlayed = true;
        _playbackError = null;
      });
      if (navigation.mounted && navigation.screen == origin) {
        navigation.push(PodScreen.nowPlaying);
      }
    } catch (_) {
      if (mounted) setState(() => _playbackError = origin);
    } finally {
      _startingPlayback = false;
    }
  }

  Future<List<DeviceTrack>> _albumTracks(
    DeviceLibrary library,
    int selection,
  ) async {
    final album = await _albums.itemAt(selection);
    return album == null ? const [] : library.albumTracks(id: album.id);
  }

  DeviceTrack? get _currentTrack {
    final index = _queueIndex ?? 0;
    return index < _queue.length ? _queue[index] : null;
  }

  VoidCallback? _centreAction(
    PodNavigationState navigation,
    LibraryCounts? counts,
  ) {
    switch (navigation.screen) {
      case PodScreen.menu:
        final item = LibraryMenuItem.values[navigation.selection];
        if (item == LibraryMenuItem.nowPlaying && !_hasPlayed) return null;
        return () => navigation.push(item.destination);
      case PodScreen.songs when (counts?.songs ?? 0) > 0:
      case PodScreen.albums when (counts?.albums ?? 0) > 0:
        return () => unawaited(_playSelection(navigation));
      default:
        return null;
    }
  }

  int _selectionCount(PodScreen screen, LibraryCounts? counts) =>
      switch (screen) {
        PodScreen.menu => LibraryMenuItem.values.length,
        PodScreen.albums => counts?.albums ?? 0,
        PodScreen.songs => counts?.songs ?? 0,
        PodScreen.queue => _queue.length,
        _ => 0,
      };

  Widget _display(
    PodScreen screen,
    int selection,
    LibraryCounts? counts,
    String libraryMetadata,
  ) {
    final library = widget.library;
    return switch (screen) {
      PodScreen.menu => PodDisplay(
        eyebrow: 'Library',
        metadata: libraryMetadata,
        onEyebrowHold: kDebugMode && library != null && !_importing
            ? () => unawaited(_importTracks(library))
            : null,
        body: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Expanded(
              child: LibraryMenu(
                selection: selection,
                albums: counts?.albums ?? (library == null ? 0 : null),
                songs: counts?.songs ?? (library == null ? 0 : null),
                queueLength: _queue.length,
                hasPlayed: _hasPlayed,
              ),
            ),
            if (_notice case final notice?)
              Semantics(
                liveRegion: true,
                child: Text(
                  notice,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
          ],
        ),
      ),
      PodScreen.albums => PodDisplay(
        eyebrow: 'Albums',
        metadata: _playbackError == screen
            ? 'Could not play album'
            : _countLabel(counts?.albums ?? 0, 'album'),
        body: library == null
            ? const SizedBox.shrink()
            : AlbumsGrid(
                library: library,
                albums: _albums,
                count: counts?.albums ?? 0,
                selection: selection,
              ),
      ),
      PodScreen.songs => PodDisplay(
        eyebrow: 'Songs',
        metadata: _playbackError == screen
            ? 'Could not play song'
            : _countLabel(counts?.songs ?? 0, 'song'),
        body: SongsList(
          songs: _songs,
          count: counts?.songs ?? 0,
          selection: selection,
        ),
      ),
      PodScreen.queue => PodDisplay(
        eyebrow: 'Queue',
        metadata: _countLabel(_queue.length, 'track'),
        body: WheelList(
          count: _queue.length,
          selection: selection,
          rowHeight: MtSize.queueRow,
          topPadding: 8,
          itemBuilder: (context, index) => TrackRow(
            track: _queue[index],
            number: index + 1,
            selected: index == selection,
          ),
        ),
      ),
      PodScreen.nowPlaying => PodDisplay(
        eyebrow: _currentTrack?.album ?? 'Now Playing',
        metadata: _currentTrack?.title ?? '',
        body: const SizedBox.shrink(),
      ),
      PodScreen.equalizer => const PodDisplay(
        eyebrow: 'Equalizer',
        metadata: 'Custom',
        body: SizedBox.shrink(),
      ),
    };
  }

  @override
  void dispose() {
    unawaited(_playerEvents?.cancel());
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<LibraryCounts>(
      future: _counts,
      builder: (context, snapshot) {
        final library = widget.library;
        final counts = snapshot.data;
        final metadata = switch (snapshot) {
          _ when _importing => 'Importing…',
          _ when snapshot.hasError => 'Could not open library',
          _ when counts != null =>
            '${_countLabel(counts.songs, 'song')} · ${_countLabel(counts.albums, 'album')}',
          _ when library != null => 'Opening library…',
          _ => '0 songs · 0 albums',
        };
        return PodNavigation(
          display: (context, screen, selection) =>
              _display(screen, selection, counts, metadata),
          deck: (context, navigation) {
            final count = _selectionCount(navigation.screen, counts);
            return ClickWheel(
              screen: navigation.screen,
              playing: _playing,
              onMenu: navigation.menu,
              onCentre: _centreAction(navigation, counts),
              onStep: count == 0
                  ? null
                  : (direction) => navigation.select(
                      (navigation.selection + direction).clamp(0, count - 1),
                    ),
            );
          },
        );
      },
    );
  }

  String _countLabel(int count, String noun) =>
      '$count $noun${count == 1 ? '' : 's'}';
}
