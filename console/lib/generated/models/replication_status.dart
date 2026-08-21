// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

import 'standby_info.dart';

final class ReplicationStatus {
  final bool inRecovery;
  final bool isPrimary;
  final List<StandbyInfo> standbys;

  const ReplicationStatus({
    required this.inRecovery,
    required this.isPrimary,
    required this.standbys,
  });

  static ReplicationStatus fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return ReplicationStatus(
      inRecovery: map['in_recovery'] as bool,
      isPrimary: map['is_primary'] as bool,
      standbys: (map['standbys'] as List<dynamic>)
          .map((e) => StandbyInfo.fromJson(e))
          .toList(),
    );
  }

  Map<String, dynamic> toJson() => {
    'in_recovery': inRecovery,
    'is_primary': isPrimary,
    'standbys': standbys.map((e) => e.toJson()).toList(),
  };
}
