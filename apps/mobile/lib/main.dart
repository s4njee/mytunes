import 'package:flutter/widgets.dart';

import 'src/rust/api/simple.dart' as core;
import 'src/rust/frb_generated.dart';
import 'theme/tokens.g.dart';

Future<void> main() async {
  await RustLib.init();
  runApp(MyTunesPod(coreReply: '${core.ping()} (core ${core.coreVersion()})'));
}

/// M0 shell: proves Flutter can call the Rust core (plan E0.S1 AC2).
///
/// The real wheel UI starts in milestone M3.
class MyTunesPod extends StatelessWidget {
  const MyTunesPod({super.key, required this.coreReply});

  /// What the Rust core answered to `ping`.
  final String coreReply;

  @override
  Widget build(BuildContext context) {
    const colors = MtColors.dark;
    return WidgetsApp(
      title: 'MyTunes Pod',
      color: colors.accent,
      debugShowCheckedModeBanner: false,
      builder: (context, _) => ColoredBox(
        color: colors.bg,
        child: Center(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(
                'MyTunes Pod',
                style: MtType.menuRow.copyWith(color: colors.textPrimary),
              ),
              const SizedBox(height: MtSpace.rowGap),
              Text(
                coreReply,
                key: const Key('core-reply'),
                style: MtType.meta.copyWith(color: colors.accent),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
