import 'package:flutter/services.dart';
import 'package:material_ui/material_ui.dart';

import 'pod_shell.dart';
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
  runApp(const MyTunesPod());
}

/// MyTunes Pod follows the platform's light or dark appearance.
class MyTunesPod extends StatelessWidget {
  const MyTunesPod({super.key});

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
      home: const PodShell(
        eyebrow: 'Library',
        metadata: '0 songs · 0 albums',
        display: SizedBox.shrink(),
        deck: SizedBox(height: MtWheel.diameter),
      ),
    );
  }
}
