import 'package:flutter_test/flutter_test.dart';
import 'package:mytunes_pod/main.dart';
import 'package:mytunes_pod/theme/tokens.g.dart';

void main() {
  testWidgets('shows the core reply', (tester) async {
    await tester.pumpWidget(const MyTunesPod(coreReply: 'pong (core 0.0.1)'));
    expect(find.text('MyTunes Pod'), findsOneWidget);
    expect(find.text('pong (core 0.0.1)'), findsOneWidget);
  });

  test('generated tokens match the design handoff', () {
    expect(MtColors.dark.accent.toARGB32(), 0xFFF0A23C);
    expect(MtColors.light.accent.toARGB32(), 0xFF2F6DB5);
    expect(MtWheel.diameter, 252);
    expect(MtWheel.detentDegrees, 12);
  });
}
