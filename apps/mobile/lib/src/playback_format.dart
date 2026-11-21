import 'rust/api/device.dart';

String trackTime(int milliseconds) {
  final seconds = Duration(milliseconds: milliseconds).inSeconds;
  return '${seconds ~/ 60}:${(seconds % 60).toString().padLeft(2, '0')}';
}

String? trackFormat(DeviceTrack track) {
  final codec = track.codec?.trim().toUpperCase();
  if (codec == null || codec.isEmpty) return null;
  final lossless = switch (codec) {
    'FLAC' || 'ALAC' || 'WAV' || 'AIFF' => true,
    _ => false,
  };
  final bits = track.bitDepth ?? 0;
  final rate = track.sampleRate ?? 0;
  final bitrate = track.bitrate ?? 0;
  if (lossless && bits > 0 && rate >= 1000) {
    return '$codec · $bits/${rate ~/ 1000}';
  }
  if (!lossless && bitrate > 0) return '$codec · $bitrate';
  return codec;
}
