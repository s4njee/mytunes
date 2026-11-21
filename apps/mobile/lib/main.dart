import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:material_ui/material_ui.dart';
import 'package:path_provider/path_provider.dart';

import 'albums_grid.dart';
import 'click_wheel.dart';
import 'dev_import.dart';
import 'equalizer.dart';
import 'library_menu.dart';
import 'now_playing.dart';
import 'pod_navigation.dart';
import 'pod_shell.dart';
import 'src/equalizer_settings.dart';
import 'src/library_pages.dart';
import 'src/player_events.dart';
import 'src/rust/api/device.dart';
import 'src/rust/api/playback.dart';
import 'src/rust/frb_generated.dart';
import 'src/wheel_scrub.dart';
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
  DeviceAlbum? _playingAlbum;
  int _artPaletteIndex = 0;
  final _artwork = <String, Future<String?>>{};
  final _playback = ValueNotifier<PlaybackEvent?>(null);
  late final _scrub = WheelScrub(_seekTo);
  StreamSubscription<PlaybackEvent>? _playerEvents;
  Future<void> _commands = Future.value();
  bool _volumeConfigured = false;
  EqualizerSettings _equalizer = const EqualizerSettings();
  EqualizerSettings? _appliedEqualizer;
  late final _settingsReady = _loadEqualizer();
  bool _equalizerReady = false;
  bool _equalizerError = false;
  bool _hasPlayed = false;
  bool _playing = false;
  bool _startingPlayback = false;
  PodScreen? _playbackError;

  @override
  void initState() {
    super.initState();
    unawaited(_settingsReady);
  }

  Future<void> _loadEqualizer() async {
    try {
      final library = await widget.library;
      final json = await library?.settingsGet(
        key: EqualizerSettings.storageKey,
      );
      final saved = EqualizerSettings.decode(json);
      if (mounted) _equalizer = saved;
    } catch (_) {
      // Use the defaults when the saved setting is unreadable.
    }
    if (mounted) setState(() => _equalizerReady = true);
  }

  void _equalizerStep(PodNavigationState navigation, int direction) {
    final index = navigation.selection;
    final bands = EqualizerSettings.frequencies.length;
    if (index < bands) {
      _changeEqualizer(_equalizer.adjust(index, direction));
    } else {
      navigation.select(
        (index + direction).clamp(
          bands - 1,
          bands + EqualizerOption.values.length - 1,
        ),
      );
    }
  }

  void _equalizerCentre(PodNavigationState navigation) {
    final index = navigation.selection;
    final bands = EqualizerSettings.frequencies.length;
    if (index < bands) {
      navigation.select(index + 1);
    } else {
      _changeEqualizer(
        _equalizer.toggle(EqualizerOption.values[index - bands]),
      );
    }
  }

  void _changeEqualizer(EqualizerSettings settings) {
    if (identical(settings, _equalizer)) return;
    setState(() => _equalizer = settings);
    unawaited(_saveEqualizer(settings));
  }

  Future<void> _saveEqualizer(EqualizerSettings settings) async {
    try {
      await _withTransport((library) async {
        if (!identical(settings, _equalizer)) return;
        if (_volumeConfigured) {
          await settings.apply(library, previous: _appliedEqualizer);
          _appliedEqualizer = settings;
        }
        await library.settingsSet(
          key: EqualizerSettings.storageKey,
          valueJson: settings.encode(),
        );
      });
      if (mounted && _equalizerError) setState(() => _equalizerError = false);
    } catch (_) {
      if (mounted) setState(() => _equalizerError = true);
    }
  }

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
      final album = origin == PodScreen.albums
          ? await _albums.itemAt(selection)
          : null;
      final tracks = switch (origin) {
        PodScreen.albums when album != null => await library.albumTracks(
          id: album.id,
        ),
        PodScreen.songs => [?await _songs.itemAt(selection)],
        PodScreen.queue => _queue,
        _ => const <DeviceTrack>[],
      };
      if (tracks.isEmpty || !mounted) return;
      await _settingsReady;
      final startIndex = origin == PodScreen.queue ? selection : 0;
      await _withTransport((library) async {
        if (!navigation.mounted || navigation.screen != origin) return;
        _scrub.cancel();
        _playerEvents ??= library.playerEvents().listen(
          _receivePlayback,
          onError: (Object _) {},
        );
        if (!_volumeConfigured) {
          final equalizer = _equalizer;
          await equalizer.apply(library);
          _appliedEqualizer = equalizer;
          await library.setVolume(
            volume: NowPlaying.defaultVolumePercent / 100,
          );
          _volumeConfigured = true;
        }
        await library.loadQueue(
          trackIds: tracks.map((track) => track.id).toList(growable: false),
          startIndex: startIndex,
        );
        if (!mounted) return;
        setState(() {
          _queue = tracks;
          _queueIndex = startIndex;
          if (origin != PodScreen.queue) {
            _playingAlbum = album;
            _artPaletteIndex = album == null ? 0 : selection;
          }
        });
        await library.play();
        if (mounted) {
          setState(() {
            _hasPlayed = true;
            _playbackError = null;
          });
          if (navigation.mounted && navigation.screen == origin) {
            navigation.push(PodScreen.nowPlaying);
          }
        }
      });
    } catch (_) {
      if (mounted) setState(() => _playbackError = origin);
    } finally {
      _startingPlayback = false;
    }
  }

  Future<void> _withTransport(Future<void> Function(DeviceLibrary) action) {
    final next = _commands.then((_) async {
      final library = await widget.library;
      if (mounted && library != null) await action(library);
    });
    _commands = next.then<void>((_) {}, onError: (Object _, StackTrace _) {});
    return next;
  }

  (String?, int?, BigInt?) _trackPass(PlaybackEvent? event) =>
      (event?.trackId, event?.queueIndex, event?.pass);

  void _receivePlayback(PlaybackEvent event) {
    if (!mounted) return;
    if (_trackPass(_playback.value) != _trackPass(event)) _scrub.cancel();
    _playback.value = event;
    final playing = event.state == PlaybackState.playing;
    if (playing != _playing || event.queueIndex != _queueIndex) {
      setState(() {
        _playing = playing;
        _queueIndex = event.queueIndex;
      });
    }
  }

  Future<void> _control(Future<void> Function(DeviceLibrary) action) async {
    try {
      await _withTransport(action);
      if (mounted && _playbackError == PodScreen.nowPlaying) {
        setState(() => _playbackError = null);
      }
    } catch (_) {
      if (mounted) setState(() => _playbackError = PodScreen.nowPlaying);
    }
  }

  void _toggle() => unawaited(_control((library) => library.toggle()));

  void _skip({required bool forward}) {
    _scrub.cancel();
    unawaited(_control((library) => forward ? library.next() : library.prev()));
  }

  void _scrubStep(int direction) {
    final event = _playback.value;
    final track = _currentTrack;
    if (event == null || track == null || event.trackId != track.id) return;
    _scrub.step(
      direction,
      position: event.positionMs.toInt(),
      duration: track.durationMs?.toInt() ?? 0,
    );
  }

  Future<void> _seekTo(int position) {
    final track = _trackPass(_playback.value);
    return _control((library) async {
      if (track == _trackPass(_playback.value)) {
        await library.seek(positionMs: BigInt.from(position));
      }
    });
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
        final initialIndex = item == LibraryMenuItem.queue && _queue.isNotEmpty
            ? (_queueIndex ?? 0).clamp(0, _queue.length - 1)
            : 0;
        return () => navigation.push(item.destination, selection: initialIndex);
      case PodScreen.songs when (counts?.songs ?? 0) > 0:
      case PodScreen.albums when (counts?.albums ?? 0) > 0:
      case PodScreen.queue when _queue.isNotEmpty:
        return () => unawaited(_playSelection(navigation));
      case PodScreen.nowPlaying when _queue.isNotEmpty:
        return _toggle;
      case PodScreen.equalizer when _equalizerReady:
        return () => _equalizerCentre(navigation);
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
            current: index == _queueIndex,
          ),
        ),
      ),
      PodScreen.nowPlaying => _nowPlaying(),
      PodScreen.equalizer => PodDisplay(
        eyebrow: 'Equalizer',
        metadata: _equalizerError ? 'Could not apply settings' : 'Custom',
        body: Equalizer(settings: _equalizer, selection: selection),
      ),
    };
  }

  Widget _nowPlaying() {
    final track = _currentTrack;
    final artworkId = track?.artworkId ?? _playingAlbum?.artworkId;
    final year = _playingAlbum?.id.year;
    final library = widget.library;
    return PodDisplay(
      eyebrow: track?.album ?? 'Now Playing',
      metadata: _playbackError == PodScreen.nowPlaying
          ? 'Could not control playback'
          : (year?.toString() ?? ''),
      body: track == null
          ? const SizedBox.shrink()
          : NowPlaying(
              track: track,
              artworkPath: artworkId == null || library == null
                  ? null
                  : _artwork.putIfAbsent(
                      artworkId,
                      () => library.then(
                        (library) => library.artworkPath(
                          id: artworkId,
                          size: ThumbnailSize.small,
                        ),
                      ),
                    ),
              paletteIndex: _artPaletteIndex,
              events: _playback,
              scrubPreview: _scrub.preview,
            ),
    );
  }

  @override
  void dispose() {
    unawaited(_playerEvents?.cancel());
    _scrub.dispose();
    _playback.dispose();
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
            final now = navigation.screen == PodScreen.nowPlaying;
            final equalizer = navigation.screen == PodScreen.equalizer;
            final hasQueue = _queue.isNotEmpty;
            final bands = EqualizerSettings.frequencies;
            return ClickWheel(
              screen: navigation.screen,
              playing: _playing,
              eqBand: equalizer && navigation.selection < bands.length
                  ? bands[navigation.selection]
                  : null,
              onMenu: navigation.menu,
              onCentre: _centreAction(navigation, counts),
              onPrevious: hasQueue ? () => _skip(forward: false) : null,
              onNext: hasQueue ? () => _skip(forward: true) : null,
              onPlayPause: hasQueue ? _toggle : null,
              onDragEnd: now ? _scrub.release : null,
              onStep: equalizer && _equalizerReady
                  ? (direction) => _equalizerStep(navigation, direction)
                  : now
                  ? _scrubStep
                  : count == 0
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
