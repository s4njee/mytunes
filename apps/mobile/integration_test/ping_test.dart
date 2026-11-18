import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:mytunes_pod/src/rust/api/simple.dart' as core;
import 'package:mytunes_pod/src/rust/frb_generated.dart';

/// Runs on a device or emulator with the real Rust library (plan E0.S1 AC2):
///   flutter test integration_test
void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async => RustLib.init());

  test('Rust ping returns pong', () {
    expect(core.ping(), 'pong');
    expect(core.coreVersion(), matches(RegExp(r'^\d+\.\d+\.\d+$')));
  });
}
