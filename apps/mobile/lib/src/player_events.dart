import 'rust/api/device.dart';
import 'rust/api/playback.dart';

extension MobilePlayerEvents on DeviceLibrary {
  /// Subscribe to transport snapshots, releasing the Rust listener on cancel.
  Stream<PlaybackEvent> playerEvents() async* {
    final subscription = await subscribePlayer();
    try {
      yield* await subscription.stream();
    } finally {
      subscription.dispose();
    }
  }
}
