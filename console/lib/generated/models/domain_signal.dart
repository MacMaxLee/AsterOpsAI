// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

import 'host_domain.dart';
import 'tier.dart';

final class DomainSignal {
  final int crossedCount;
  final HostDomain domain;
  final int sampleCount;
  final Tier tier;

  const DomainSignal({
    required this.crossedCount,
    required this.domain,
    required this.sampleCount,
    required this.tier,
  });

  static DomainSignal fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return DomainSignal(
      crossedCount: (map['crossed_count'] as num).toInt(),
      domain: HostDomain.fromJson(map['domain']),
      sampleCount: (map['sample_count'] as num).toInt(),
      tier: Tier.fromJson(map['tier']),
    );
  }

  Map<String, dynamic> toJson() => {
    'crossed_count': crossedCount,
    'domain': domain.toJson(),
    'sample_count': sampleCount,
    'tier': tier.toJson(),
  };
}
