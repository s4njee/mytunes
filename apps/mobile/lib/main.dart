import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:material_ui/material_ui.dart';
import 'package:path_provider/path_provider.dart';

import 'dev_import.dart';
import 'pod_shell.dart';
import 'src/rust/api/device.dart';
import 'src/rust/frb_generated.dart';
import 'theme/font_licences.dart';
import 'theme/mt_theme.dart';
import 'theme/tokens.g.dart';

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
        return PodShell(
          eyebrow: 'Library',
          metadata: metadata,
          onEyebrowHold: kDebugMode && library != null && !_importing
              ? () => unawaited(_importTracks(library))
              : null,
          display: _notice == null
              ? const SizedBox.shrink()
              : Padding(
                  padding: const EdgeInsets.only(top: MtSpace.rowGap),
                  child: Semantics(liveRegion: true, child: Text(_notice!)),
                ),
          deck: const SizedBox(height: MtWheel.diameter),
        );
      },
    );
  }

  String _countLabel(int count, String noun) =>
      '$count $noun${count == 1 ? '' : 's'}';
}
