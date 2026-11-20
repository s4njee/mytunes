import 'package:file_selector/file_selector.dart';

import 'src/rust/api/device.dart';

/// Opens the system picker and imports the selected audio in a debug build.
Future<String?> importPickedTracks(DeviceLibrary library) async {
  final files = await openFiles(
    acceptedTypeGroups: const [
      XTypeGroup(
        label: 'Audio',
        mimeTypes: ['audio/*'],
        uniformTypeIdentifiers: ['public.audio'],
      ),
    ],
  );
  if (files.isEmpty) return null;

  final report = await library.importDevFiles(
    paths: files.map((file) => file.path).toList(),
  );
  final summary = [
    '${report.imported} imported',
    if (report.unchanged > 0) '${report.unchanged} already present',
    if (report.errors.isNotEmpty) '${report.errors.length} failed',
  ].join(' · ');
  return report.errors.isEmpty ? summary : '$summary\n${report.errors.first}';
}
