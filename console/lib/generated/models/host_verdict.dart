// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

import 'domain_signal.dart';
import 'evidence.dart';
import 'host_bottleneck.dart';

final class HostVerdict {
  final HostBottleneck bottleneck;
  final List<DomainSignal> domainSignals;
  final List<Evidence> evidence;
  final int score;
  final String scoreVersion;

  const HostVerdict({
    required this.bottleneck,
    required this.domainSignals,
    required this.evidence,
    required this.score,
    required this.scoreVersion,
  });

  static HostVerdict fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return HostVerdict(
      bottleneck: HostBottleneck.fromJson(map['bottleneck']),
      domainSignals: (map['domain_signals'] as List<dynamic>)
          .map((e) => DomainSignal.fromJson(e))
          .toList(),
      evidence: (map['evidence'] as List<dynamic>)
          .map((e) => Evidence.fromJson(e))
          .toList(),
      score: (map['score'] as num).toInt(),
      scoreVersion: map['score_version'] as String,
    );
  }

  Map<String, dynamic> toJson() => {
    'bottleneck': bottleneck.toJson(),
    'domain_signals': domainSignals.map((e) => e.toJson()).toList(),
    'evidence': evidence.map((e) => e.toJson()).toList(),
    'score': score,
    'score_version': scoreVersion,
  };
}
