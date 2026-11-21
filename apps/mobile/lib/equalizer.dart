import 'package:flutter/widgets.dart';

import 'src/equalizer_settings.dart';
import 'theme/mt_theme.dart';
import 'theme/tokens.g.dart';

class Equalizer extends StatelessWidget {
  const Equalizer({super.key, required this.settings, required this.selection});

  final EqualizerSettings settings;
  final int selection;

  @override
  Widget build(BuildContext context) {
    final colors = MtTheme.of(context).colors;
    final frequencies = EqualizerSettings.frequencies;
    return ListView(
      padding: const EdgeInsets.only(top: 20),
      children: [
        SizedBox(
          height: MtSize.eqBandRow,
          child: Row(
            spacing: MtSize.eqBandGap,
            children: [
              for (var band = 0; band < frequencies.length; band++)
                Expanded(
                  child: _Band(
                    frequency: frequencies[band],
                    gain: settings.gains[band],
                    selected: selection == band,
                  ),
                ),
            ],
          ),
        ),
        const SizedBox(height: 18),
        SizedBox(height: 1, child: ColoredBox(color: colors.rule)),
        for (final option in EqualizerOption.values)
          _Option(
            option: option,
            enabled: settings.enabled(option),
            selected: selection == frequencies.length + option.index,
          ),
      ],
    );
  }
}

class _Band extends StatelessWidget {
  const _Band({
    required this.frequency,
    required this.gain,
    required this.selected,
  });

  final String frequency;
  final int gain;
  final bool selected;

  @override
  Widget build(BuildContext context) {
    final colors = MtTheme.of(context).colors;
    final signedGain = gain > 0 ? '+$gain' : '$gain';
    return Semantics(
      container: true,
      selected: selected,
      label: '$frequency hertz',
      value: '$signedGain decibels',
      excludeSemantics: true,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        spacing: 9,
        children: [
          Expanded(
            child: ClipRRect(
              borderRadius: BorderRadius.circular(MtRadius.eqWell),
              child: LayoutBuilder(
                builder: (context, constraints) {
                  final centre = constraints.maxHeight / 2;
                  final height = centre * gain.abs() / 6;
                  return ColoredBox(
                    color: colors.well,
                    child: Stack(
                      fit: StackFit.expand,
                      children: [
                        Positioned(
                          top: centre,
                          left: 0,
                          right: 0,
                          height: 1,
                          child: ColoredBox(color: colors.eqZeroLine),
                        ),
                        Positioned(
                          top: gain > 0 ? centre - height : centre,
                          left: 0,
                          right: 0,
                          height: height,
                          child: ColoredBox(
                            color: selected
                                ? colors.accent
                                : colors.accentMuted,
                          ),
                        ),
                      ],
                    ),
                  );
                },
              ),
            ),
          ),
          Text(
            frequency,
            textAlign: TextAlign.center,
            maxLines: 1,
            style: MtType.eqHz.copyWith(
              color: selected ? colors.accent : colors.textTimecode,
            ),
          ),
          Text(
            signedGain,
            textAlign: TextAlign.center,
            maxLines: 1,
            style: MtType.eqGain.copyWith(color: colors.textCount),
          ),
        ],
      ),
    );
  }
}

class _Option extends StatelessWidget {
  const _Option({
    required this.option,
    required this.enabled,
    required this.selected,
  });

  final EqualizerOption option;
  final bool enabled;
  final bool selected;

  @override
  Widget build(BuildContext context) {
    final colors = MtTheme.of(context).colors;
    return Semantics(
      container: true,
      selected: selected,
      toggled: enabled,
      child: SizedBox(
        height: MtSize.statusRow,
        child: Row(
          spacing: MtSpace.rowGap,
          children: [
            Expanded(
              child: Text(
                option.label,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: MtType.statusLabel.copyWith(
                  color: selected ? colors.accent : colors.textLabel,
                ),
              ),
            ),
            Text(
              enabled ? 'ON' : 'OFF',
              style: MtType.meta.copyWith(
                color: enabled ? colors.accent : colors.textQuaternary,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
