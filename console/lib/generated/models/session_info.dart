// GENERATED CODE - DO NOT EDIT BY HAND.
// Regenerate: dart run tool/generate_models.dart

import 'session_state.dart';

final class SessionInfo {
  final String? clientAddr;
  final String? database;
  final int pid;
  final String? query;
  final DateTime? queryStart;
  final SessionState state;
  final String? username;
  final DateTime? xactStart;

  const SessionInfo({
    this.clientAddr,
    this.database,
    required this.pid,
    this.query,
    this.queryStart,
    required this.state,
    this.username,
    this.xactStart,
  });

  static SessionInfo fromJson(dynamic json) {
    final map = json as Map<String, dynamic>;
    return SessionInfo(
      clientAddr: map['client_addr'] == null
          ? null
          : (map['client_addr'] as String),
      database: map['database'] == null ? null : (map['database'] as String),
      pid: (map['pid'] as num).toInt(),
      query: map['query'] == null ? null : (map['query'] as String),
      queryStart: map['query_start'] == null
          ? null
          : (DateTime.parse(map['query_start'] as String)),
      state: SessionState.fromJson(map['state']),
      username: map['username'] == null ? null : (map['username'] as String),
      xactStart: map['xact_start'] == null
          ? null
          : (DateTime.parse(map['xact_start'] as String)),
    );
  }

  Map<String, dynamic> toJson() => {
    'client_addr': clientAddr,
    'database': database,
    'pid': pid,
    'query': query,
    'query_start': queryStart?.toIso8601String(),
    'state': state.toJson(),
    'username': username,
    'xact_start': xactStart?.toIso8601String(),
  };
}
