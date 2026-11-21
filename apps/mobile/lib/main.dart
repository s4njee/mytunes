import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:material_ui/material_ui.dart';
import 'package:path_provider/path_provider.dart';

import 'click_wheel.dart';
import 'dev_import.dart';
import 'library_menu.dart';
import 'pod_navigation.dart';
import 'pod_shell.dart';
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
  bool _importing = false;
  String? _notice;
  List<DeviceTrack> _queue = const [];
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

  Future<void> _playSong(PodNavigationState navigation) async {
    final database = widget.library;
    if (database == null || _startingPlayback) return;
    _startingPlayback = true;
    final selection = navigation.selection;
    try {
      final library = await database;
      final tracks = await library.songs(
        page: LibraryPage(offset: selection, limit: 1),
      );
      if (tracks.isEmpty ||
          !mounted ||
          !navigation.mounted ||
          navigation.screen != PodScreen.songs) {
        return;
      }
      _playerEvents ??= library.playerEvents().listen((event) {
        final playing = event.state == PlaybackState.playing;
        if (mounted && playing != _playing) {
          setState(() => _playing = playing);
        }
      }, onError: (Object _) {});
      await library.loadQueue(trackIds: [tracks.single.id], startIndex: 0);
      if (!mounted) return;
      setState(() => _queue = tracks);
      await library.play();
      if (!mounted) return;
      setState(() {
        _hasPlayed = true;
        _playbackError = null;
      });
      if (navigation.mounted && navigation.screen == PodScreen.songs) {
        navigation.push(PodScreen.nowPlaying);
      }
    } catch (_) {
      if (mounted) setState(() => _playbackError = PodScreen.songs);
    } finally {
      _startingPlayback = false;
    }
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
        return () => unawaited(_playSong(navigation));
      default:
        return null;
    }
  }

  int _selectionCount(PodScreen screen, LibraryCounts? counts) =>
      switch (screen) {
        PodScreen.menu => LibraryMenuItem.values.length,
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
        metadata: _countLabel(counts?.albums ?? 0, 'album'),
        body: const SizedBox.shrink(),
      ),
      PodScreen.songs => PodDisplay(
        eyebrow: 'Songs',
        metadata: _playbackError == screen
            ? 'Could not play song'
            : _countLabel(counts?.songs ?? 0, 'song'),
        body: SongsList(
          library: library,
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
        eyebrow: _queue.firstOrNull?.album ?? 'Now Playing',
        metadata: _queue.firstOrNull?.title ?? '',
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
