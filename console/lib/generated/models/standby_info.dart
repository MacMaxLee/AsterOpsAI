// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

final class StandbyInfo {
  final String? clientAddr;
  final String? flushLsn;
  final double? replayLagSeconds;
  final String? replayLsn;
  final String? sentLsn;
  final String state;
  final String? writeLsn;

  const StandbyInfo({
    this.clientAddr,
    this.flushLsn,
    this.replayLagSeconds,
    this.replayLsn,
    this.sentLsn,
    required this.state,
    this.writeLsn,
  });

  static StandbyInfo fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return StandbyInfo(
      clientAddr: map['client_addr'] == null
          ? null
          : (map['client_addr'] as String),
      flushLsn: map['flush_lsn'] == null ? null : (map['flush_lsn'] as String),
      replayLagSeconds: map['replay_lag_seconds'] == null
          ? null
          : ((map['replay_lag_seconds'] as num).toDouble()),
      replayLsn: map['replay_lsn'] == null
          ? null
          : (map['replay_lsn'] as String),
      sentLsn: map['sent_lsn'] == null ? null : (map['sent_lsn'] as String),
      state: map['state'] as String,
      writeLsn: map['write_lsn'] == null ? null : (map['write_lsn'] as String),
    );
  }

  Map<String, dynamic> toJson() => {
    'client_addr': clientAddr,
    'flush_lsn': flushLsn,
    'replay_lag_seconds': replayLagSeconds,
    'replay_lsn': replayLsn,
    'sent_lsn': sentLsn,
    'state': state,
    'write_lsn': writeLsn,
  };
}
